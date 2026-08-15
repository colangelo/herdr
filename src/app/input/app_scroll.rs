//! Alt-screen scroll passthrough — copy-mode scroll gestures forwarded to the
//! application when the focused pane owns the alternate screen and therefore
//! has no scrollback for copy mode to enter.

use bytes::Bytes;
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

use crate::{
    app::{state::AppScrollState, App, AppState, Mode},
    input::TerminalKey,
    terminal::TerminalRuntimeRegistry,
};

use super::copy_mode::CopyModeEntryScroll;

/// The key forwarded to the application for a scroll intent, or `None` for
/// keys the mode swallows. The vocabulary is deliberately the pager one:
/// arrow keys mean prompt history in shell-like TUIs, so no line granularity.
fn passthrough_key(key: &TerminalKey) -> Option<KeyCode> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    if alt {
        return None;
    }
    match key.code {
        KeyCode::Char('u' | 'U') if ctrl => Some(KeyCode::PageUp),
        KeyCode::Char('d' | 'D') if ctrl => Some(KeyCode::PageDown),
        KeyCode::PageUp if !ctrl => Some(KeyCode::PageUp),
        KeyCode::PageDown if !ctrl => Some(KeyCode::PageDown),
        KeyCode::Char('g') if !ctrl => Some(KeyCode::Home),
        KeyCode::Char('G') if !ctrl => Some(KeyCode::End),
        KeyCode::Home if !ctrl => Some(KeyCode::Home),
        KeyCode::End if !ctrl => Some(KeyCode::End),
        _ => None,
    }
}

fn is_exit_key(key: &TerminalKey) -> bool {
    matches!(key.code, KeyCode::Esc | KeyCode::Enter)
        || (matches!(key.code, KeyCode::Char('q')) && key.modifiers.is_empty())
}

impl AppState {
    /// Divert a copy-mode scroll gesture into the passthrough mode when the
    /// focused pane's application owns the alternate screen. Returns whether
    /// the gesture was diverted; `false` (pane unresolvable, primary screen)
    /// leaves entry to the copy-mode path.
    pub(crate) fn try_enter_app_scroll_mode(
        &mut self,
        terminal_runtimes: &TerminalRuntimeRegistry,
        entry: CopyModeEntryScroll,
    ) -> bool {
        let Some(ws_idx) = self.active else {
            return false;
        };
        let Some(pane_id) = self
            .workspaces
            .get(ws_idx)
            .and_then(|ws| ws.focused_pane_id())
        else {
            return false;
        };
        let alt_screen = self
            .runtime_for_pane_in_workspace(terminal_runtimes, ws_idx, pane_id)
            .is_some_and(|rt| rt.alternate_screen_active());
        if !alt_screen {
            return false;
        }
        self.clear_selection();
        self.app_scroll = Some(AppScrollState { pane_id });
        self.mode = Mode::AppScroll;
        // "Half page" has no distinct terminal key; alt-screen applications
        // page at their own granularity, so both page gestures send PageUp.
        // The line gesture enters the mode without a key: no line-scroll key
        // exists that is safe to send blind.
        if matches!(
            entry,
            CopyModeEntryScroll::Page | CopyModeEntryScroll::HalfPage
        ) {
            self.pending_app_scroll_keys
                .push(TerminalKey::new(KeyCode::PageUp, KeyModifiers::empty()));
        }
        true
    }

    pub(crate) fn leave_app_scroll_mode(&mut self) {
        self.app_scroll = None;
        if self.mode == Mode::AppScroll {
            self.mode = Mode::Terminal;
        }
    }

    pub(crate) fn handle_app_scroll_key(&mut self, key: TerminalKey) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        let focused = self
            .active
            .and_then(|ws_idx| self.workspaces.get(ws_idx))
            .and_then(|ws| ws.focused_pane_id());
        let pinned = self.app_scroll.as_ref().map(|state| state.pane_id);
        if pinned.is_none() || focused != pinned {
            // Focus moved under the mode (API, another client, a click): the
            // scroll workflow is over, and forwarding to a different pane than
            // the one the user was reading would be worse than exiting.
            self.leave_app_scroll_mode();
            return;
        }
        if self.is_prefix_key(&key) {
            // Unlike copy mode there is no anchor to preserve, so the mode
            // exits rather than lingering behind prefix mode; the gesture
            // re-enters it in one step.
            self.leave_app_scroll_mode();
            self.mode = Mode::Prefix;
            return;
        }
        if is_exit_key(&key) {
            self.leave_app_scroll_mode();
            return;
        }
        if let Some(code) = passthrough_key(&key) {
            self.pending_app_scroll_keys
                .push(TerminalKey::new(code, KeyModifiers::empty()));
        }
    }
}

impl App {
    pub(crate) fn handle_app_scroll_key(&mut self, key: TerminalKey) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        self.state.update_dismissed = true;
        self.state.handle_app_scroll_key(key);
        self.dispatch_pending_app_scroll_keys();
    }

    /// Encode and send the keys the passthrough mode queued. Losing the pane
    /// or its runtime exits the mode instead of erroring: the application the
    /// user was scrolling is gone.
    pub(crate) fn dispatch_pending_app_scroll_keys(&mut self) {
        if self.state.pending_app_scroll_keys.is_empty() {
            return;
        }
        let keys = std::mem::take(&mut self.state.pending_app_scroll_keys);
        let target = self
            .state
            .app_scroll
            .as_ref()
            .map(|state| state.pane_id)
            .zip(self.state.active);
        let sent = target.is_some_and(|(pane_id, ws_idx)| {
            let Some(runtime) = self.lookup_runtime_sender(ws_idx, pane_id) else {
                return false;
            };
            for key in keys {
                let bytes = runtime.encode_terminal_key(key);
                if bytes.is_empty() {
                    continue;
                }
                if runtime.try_send_bytes(Bytes::from(bytes)).is_err() {
                    return false;
                }
            }
            true
        });
        if !sent {
            self.state.leave_app_scroll_mode();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::app_for_mouse_test;
    use super::*;
    use crate::workspace::Workspace;
    use ratatui::layout::Rect;
    use tokio::sync::mpsc;

    const ALT_SCREEN_BYTES: &[u8] = b"\x1b[?1049halt screen content";

    fn app_with_channel_pane(
        screen_bytes: &[u8],
    ) -> (App, crate::layout::PaneId, mpsc::Receiver<Bytes>) {
        let mut app = app_for_mouse_test();
        let mut ws = Workspace::test_new("test");
        let pane_id = ws.tabs[0].root_pane;
        let pane_infos = ws.tabs[0].layout.panes(Rect::new(0, 0, 20, 5));
        let info = pane_infos[0].clone();
        let (runtime, rx) =
            crate::terminal::TerminalRuntime::test_with_channel_and_scrollback_bytes(
                info.inner_rect.width,
                info.inner_rect.height,
                16 * 1024,
                screen_bytes,
                16,
            );
        ws.tabs[0].runtimes.insert(pane_id, runtime);
        app.state.workspaces = vec![ws];
        app.state.active = Some(0);
        app.state.selected = 0;
        app.state.mode = Mode::Terminal;
        app.state.view.pane_infos = pane_infos;
        app.state.prefix_code = KeyCode::Char('a');
        app.state.prefix_mods = KeyModifiers::CONTROL;
        (app, pane_id, rx)
    }

    fn app_with_alt_screen_pane() -> (App, crate::layout::PaneId, mpsc::Receiver<Bytes>) {
        app_with_channel_pane(ALT_SCREEN_BYTES)
    }

    fn drain(rx: &mut mpsc::Receiver<Bytes>) -> Vec<u8> {
        let mut out = Vec::new();
        while let Ok(bytes) = rx.try_recv() {
            out.extend_from_slice(&bytes);
        }
        out
    }

    fn encoded(app: &App, pane_id: crate::layout::PaneId, code: KeyCode) -> Vec<u8> {
        app.state
            .runtime_for_pane_in_workspace(&app.terminal_runtimes, 0, pane_id)
            .expect("runtime")
            .encode_terminal_key(TerminalKey::new(code, KeyModifiers::empty()))
    }

    async fn press(app: &mut App, code: KeyCode, mods: KeyModifiers) {
        app.handle_key(TerminalKey::new(code, mods)).await;
    }

    async fn prefix_gesture(app: &mut App, code: KeyCode, mods: KeyModifiers) {
        let (prefix_code, prefix_mods) = (app.state.prefix_code, app.state.prefix_mods);
        press(app, prefix_code, prefix_mods).await;
        press(app, code, mods).await;
    }

    #[tokio::test]
    async fn prefix_ctrl_u_on_alt_screen_enters_passthrough_and_sends_page_up() {
        let (mut app, pane_id, mut rx) = app_with_alt_screen_pane();

        prefix_gesture(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL).await;

        assert_eq!(app.state.mode, Mode::AppScroll);
        assert!(app.state.copy_mode.is_none());
        assert_eq!(app.state.app_scroll, Some(AppScrollState { pane_id }));
        assert_eq!(drain(&mut rx), encoded(&app, pane_id, KeyCode::PageUp));
    }

    #[tokio::test]
    async fn passthrough_repeats_and_pages_both_ways() {
        let (mut app, pane_id, mut rx) = app_with_alt_screen_pane();
        prefix_gesture(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL).await;
        drain(&mut rx);

        press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL).await;
        press(&mut app, KeyCode::Char('d'), KeyModifiers::CONTROL).await;
        press(&mut app, KeyCode::PageUp, KeyModifiers::empty()).await;

        let mut expected = encoded(&app, pane_id, KeyCode::PageUp);
        expected.extend(encoded(&app, pane_id, KeyCode::PageDown));
        expected.extend(encoded(&app, pane_id, KeyCode::PageUp));
        assert_eq!(drain(&mut rx), expected);
        assert_eq!(app.state.mode, Mode::AppScroll);
    }

    #[tokio::test]
    async fn passthrough_top_and_bottom_keys() {
        let (mut app, pane_id, mut rx) = app_with_alt_screen_pane();
        prefix_gesture(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL).await;
        drain(&mut rx);

        press(&mut app, KeyCode::Char('g'), KeyModifiers::empty()).await;
        press(&mut app, KeyCode::Char('G'), KeyModifiers::SHIFT).await;

        let mut expected = encoded(&app, pane_id, KeyCode::Home);
        expected.extend(encoded(&app, pane_id, KeyCode::End));
        assert_eq!(drain(&mut rx), expected);
    }

    #[tokio::test]
    async fn passthrough_swallows_unmapped_keys() {
        let (mut app, _pane_id, mut rx) = app_with_alt_screen_pane();
        prefix_gesture(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL).await;
        drain(&mut rx);

        press(&mut app, KeyCode::Char('x'), KeyModifiers::empty()).await;
        press(&mut app, KeyCode::Up, KeyModifiers::empty()).await;

        assert!(drain(&mut rx).is_empty());
        assert_eq!(app.state.mode, Mode::AppScroll);
    }

    #[tokio::test]
    async fn esc_exits_and_returns_keys_to_the_application() {
        let (mut app, pane_id, mut rx) = app_with_alt_screen_pane();
        prefix_gesture(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL).await;
        drain(&mut rx);

        press(&mut app, KeyCode::Esc, KeyModifiers::empty()).await;
        assert_eq!(app.state.mode, Mode::Terminal);
        assert!(app.state.app_scroll.is_none());
        assert!(drain(&mut rx).is_empty());

        press(&mut app, KeyCode::Char('x'), KeyModifiers::empty()).await;
        assert_eq!(
            drain(&mut rx),
            app.state
                .runtime_for_pane_in_workspace(&app.terminal_runtimes, 0, pane_id)
                .expect("runtime")
                .encode_terminal_key(TerminalKey::new(KeyCode::Char('x'), KeyModifiers::empty()))
        );
    }

    #[tokio::test]
    async fn q_exits_the_mode() {
        let (mut app, _pane_id, mut rx) = app_with_alt_screen_pane();
        prefix_gesture(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL).await;
        drain(&mut rx);

        press(&mut app, KeyCode::Char('q'), KeyModifiers::empty()).await;

        assert_eq!(app.state.mode, Mode::Terminal);
        assert!(drain(&mut rx).is_empty());
    }

    #[tokio::test]
    async fn line_gesture_enters_without_sending() {
        let (mut app, _pane_id, mut rx) = app_with_alt_screen_pane();

        prefix_gesture(&mut app, KeyCode::Char('k'), KeyModifiers::CONTROL).await;

        assert_eq!(app.state.mode, Mode::AppScroll);
        assert!(drain(&mut rx).is_empty());
    }

    #[tokio::test]
    async fn page_gesture_on_alt_screen_diverts_too() {
        let (mut app, pane_id, mut rx) = app_with_alt_screen_pane();

        prefix_gesture(&mut app, KeyCode::PageUp, KeyModifiers::empty()).await;

        assert_eq!(app.state.mode, Mode::AppScroll);
        assert_eq!(drain(&mut rx), encoded(&app, pane_id, KeyCode::PageUp));
    }

    #[tokio::test]
    async fn primary_screen_still_enters_copy_mode() {
        let (mut app, _pane_id, mut rx) =
            app_with_channel_pane(b"line one\r\nline two\r\nline three");

        prefix_gesture(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL).await;

        assert_eq!(app.state.mode, Mode::Copy);
        assert!(app.state.copy_mode.is_some());
        assert!(app.state.app_scroll.is_none());
        assert!(drain(&mut rx).is_empty());
    }

    #[tokio::test]
    async fn prefix_from_passthrough_reenters_prefix_and_gesture_reenters_mode() {
        let (mut app, pane_id, mut rx) = app_with_alt_screen_pane();
        prefix_gesture(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL).await;
        drain(&mut rx);

        let (prefix_code, prefix_mods) = (app.state.prefix_code, app.state.prefix_mods);
        press(&mut app, prefix_code, prefix_mods).await;
        assert_eq!(app.state.mode, Mode::Prefix);
        assert!(app.state.app_scroll.is_none());

        press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL).await;
        assert_eq!(app.state.mode, Mode::AppScroll);
        assert_eq!(drain(&mut rx), encoded(&app, pane_id, KeyCode::PageUp));
    }

    #[tokio::test]
    async fn focus_change_under_the_mode_exits_without_forwarding() {
        let (mut app, pane_id, mut rx) = app_with_alt_screen_pane();
        prefix_gesture(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL).await;
        drain(&mut rx);
        assert_eq!(app.state.app_scroll, Some(AppScrollState { pane_id }));

        // A split focuses the new pane, so the pinned pane is no longer focused.
        app.state.workspaces[0].test_split(ratatui::layout::Direction::Horizontal);

        press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL).await;

        assert_eq!(app.state.mode, Mode::Terminal);
        assert!(app.state.app_scroll.is_none());
        assert!(drain(&mut rx).is_empty());
    }
}
