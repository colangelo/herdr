use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
#[cfg(test)]
use ratatui::layout::Direction;
use ratatui::layout::Rect;

use crate::{
    app::{
        state::{
            AppState, ContextMenuKind, ContextMenuState, MenuListState, Mode, NavigatorStateFilter,
        },
        App,
    },
    input::TerminalKey,
    layout::NavDirection,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ModalAction {
    Continue,
    Save,
    Clear,
    Cancel,
    Confirm,
    Apply,
    Close,
}

/// What the todo panel is being asked to do. Keys and clicks both funnel
/// through `App::apply_pane_todo_action`, so a shortcut and its button can
/// never diverge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PaneTodoAction {
    Add,
    Edit,
    ToggleDone,
    Remove,
    ClearDone,
    FollowLink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ModalKeyBinding {
    Enter,
    Esc,
    CtrlC,
}

impl ModalKeyBinding {
    fn matches(self, key: &KeyEvent) -> bool {
        match self {
            Self::Enter => key.code == KeyCode::Enter,
            Self::Esc => key.code == KeyCode::Esc,
            Self::CtrlC => {
                key.code == KeyCode::Char('c')
                    && key.modifiers == crossterm::event::KeyModifiers::CONTROL
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ModalActionSpec<A> {
    pub action: A,
    pub bindings: &'static [ModalKeyBinding],
}

pub(super) fn modal_action_from_key<A: Copy>(
    key: &KeyEvent,
    specs: &[ModalActionSpec<A>],
) -> Option<A> {
    specs
        .iter()
        .find(|spec| spec.bindings.iter().any(|binding| binding.matches(key)))
        .map(|spec| spec.action)
}

pub(super) fn modal_action_from_buttons<A: Copy>(
    col: u16,
    row: u16,
    buttons: &[(Rect, A)],
) -> Option<A> {
    buttons.iter().find_map(|(rect, action)| {
        (col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height)
            .then_some(*action)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GlobalMenuAction {
    Detach,
    WhatsNew,
    Keybinds,
    ReloadConfig,
    Settings,
}

pub(super) fn global_menu_actions(state: &AppState) -> Vec<GlobalMenuAction> {
    let mut actions = vec![
        GlobalMenuAction::Settings,
        GlobalMenuAction::Keybinds,
        GlobalMenuAction::ReloadConfig,
    ];
    if state.update_available.is_some() || state.latest_release_notes_available {
        actions.push(GlobalMenuAction::WhatsNew);
    }
    actions.push(GlobalMenuAction::Detach);
    actions
}

pub(super) fn open_global_menu(state: &mut AppState) {
    state.global_menu = MenuListState::new(0);
    state.mode = Mode::GlobalMenu;
}

pub(super) fn open_keybind_help(state: &mut AppState) {
    state.keybind_help.scroll = 0;
    state.keybind_help.query.clear();
    state.keybind_help.search_focused = false;
    state.mode = Mode::KeybindHelp;
}

fn open_update_release_notes(state: &mut AppState) {
    let Some(notes) = crate::release_notes::load_latest() else {
        return;
    };

    state.release_notes = Some(crate::app::state::ReleaseNotesState {
        version: notes.version,
        body: notes.body,
        scroll: 0,
        preview: notes.preview,
    });
    state.mode = Mode::ReleaseNotes;
}

pub(super) fn request_detach(state: &mut AppState) {
    if state.detach_exits {
        state.should_quit = true;
    } else {
        state.detach_requested = true;
    }
}

pub(super) fn apply_global_menu_action(state: &mut AppState, action: GlobalMenuAction) {
    match action {
        GlobalMenuAction::Detach => {
            leave_modal(state);
            request_detach(state);
        }
        GlobalMenuAction::WhatsNew => open_update_release_notes(state),
        GlobalMenuAction::Keybinds => open_keybind_help(state),
        GlobalMenuAction::ReloadConfig => {
            state.request_reload_config = true;
            leave_modal(state);
        }
        GlobalMenuAction::Settings => super::settings::open_settings(state),
    }
}

pub(crate) fn handle_global_menu_key(state: &mut AppState, key: KeyEvent) {
    let actions = global_menu_actions(state);
    match key.code {
        KeyCode::Esc => leave_modal(state),
        KeyCode::Up | KeyCode::Char('k') => state.global_menu.move_prev(),
        KeyCode::Down | KeyCode::Char('j') => state.global_menu.move_next(actions.len()),
        KeyCode::Enter => {
            if let Some(action) = actions.get(state.global_menu.highlighted).copied() {
                apply_global_menu_action(state, action);
            }
        }
        _ => {}
    }
}

pub(crate) fn handle_navigator_key(
    state: &mut AppState,
    terminal_runtimes: &crate::terminal::TerminalRuntimeRegistry,
    key: KeyEvent,
) {
    if state.navigator.search_focused {
        match key.code {
            KeyCode::Esc => {
                state.navigator.search_focused = false;
            }
            KeyCode::Enter => {
                state.accept_navigator_selection_from(terminal_runtimes);
            }
            KeyCode::Backspace => {
                state.navigator.state_filter = None;
                state.navigator.query.pop();
                state.select_first_navigator_match_from(terminal_runtimes);
            }
            KeyCode::Up => state.move_navigator_selection_from(terminal_runtimes, -1),
            KeyCode::Down => state.move_navigator_selection_from(terminal_runtimes, 1),
            KeyCode::Char('n') if key.modifiers == KeyModifiers::CONTROL => {
                state.move_navigator_selection_from(terminal_runtimes, 1)
            }
            KeyCode::Char('p') if key.modifiers == KeyModifiers::CONTROL => {
                state.move_navigator_selection_from(terminal_runtimes, -1)
            }
            KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
                state.navigator.query.clear();
                state.navigator.state_filter = None;
                state.clamp_navigator_selection_from(terminal_runtimes);
            }
            KeyCode::Char(c)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                insert_navigator_search_text(state, terminal_runtimes, &c.to_string());
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Esc => {
            // Dismissing a link selection goes back to the modal that opened
            // it, leaving the staged link exactly as it was.
            if state.navigator.purpose == crate::app::state::NavigatorPurpose::PaneTodoLink {
                state.close_pane_todo_link_picker();
            } else {
                leave_modal(state);
            }
        }
        KeyCode::Enter => {
            state.accept_navigator_selection_from(terminal_runtimes);
        }
        KeyCode::Char('/') => {
            state.navigator.state_filter = None;
            state.navigator.search_focused = true;
            state.clamp_navigator_selection_from(terminal_runtimes);
        }
        KeyCode::Backspace if state.navigator.state_filter.is_some() => {
            state.navigator.state_filter = None;
            state.clamp_navigator_selection_from(terminal_runtimes);
        }
        KeyCode::Char('a') if key.modifiers.is_empty() => {
            state.navigator.query.clear();
            state.navigator.state_filter = None;
            state.clamp_navigator_selection_from(terminal_runtimes);
        }
        KeyCode::Char('b') if key.modifiers.is_empty() => {
            state.navigator.query.clear();
            state.navigator.state_filter = Some(NavigatorStateFilter::Blocked);
            state.select_first_navigator_match_from(terminal_runtimes);
        }
        KeyCode::Char('w') if key.modifiers.is_empty() => {
            state.navigator.query.clear();
            state.navigator.state_filter = Some(NavigatorStateFilter::Working);
            state.select_first_navigator_match_from(terminal_runtimes);
        }
        KeyCode::Char('i') if key.modifiers.is_empty() => {
            state.navigator.query.clear();
            state.navigator.state_filter = Some(NavigatorStateFilter::Idle);
            state.select_first_navigator_match_from(terminal_runtimes);
        }
        KeyCode::Char('d') if key.modifiers.is_empty() => {
            state.navigator.query.clear();
            state.navigator.state_filter = Some(NavigatorStateFilter::Done);
            state.select_first_navigator_match_from(terminal_runtimes);
        }
        KeyCode::Char('j') | KeyCode::Down if key.modifiers.is_empty() => {
            state.move_navigator_selection_from(terminal_runtimes, 1)
        }
        KeyCode::Char('k') | KeyCode::Up if key.modifiers.is_empty() => {
            state.move_navigator_selection_from(terminal_runtimes, -1)
        }
        KeyCode::Char('d') if key.modifiers == KeyModifiers::CONTROL => state
            .move_navigator_selection_by_lines_from(
                terminal_runtimes,
                (state.navigator_body_rect().height / 2).max(1) as isize,
            ),
        KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => state
            .move_navigator_selection_by_lines_from(
                terminal_runtimes,
                -((state.navigator_body_rect().height / 2).max(1) as isize),
            ),
        KeyCode::Char(' ') => state.toggle_selected_navigator_workspace_from(terminal_runtimes),
        KeyCode::Home => {
            state.navigator.selected = 0;
            state.ensure_navigator_selection_visible_from(terminal_runtimes);
        }
        KeyCode::End | KeyCode::Char('G') => {
            state.navigator.selected = state
                .navigator_rows_from(terminal_runtimes)
                .len()
                .saturating_sub(1);
            state.ensure_navigator_selection_visible_from(terminal_runtimes);
        }
        _ => {}
    }
}

pub(crate) fn insert_navigator_search_text(
    state: &mut AppState,
    terminal_runtimes: &crate::terminal::TerminalRuntimeRegistry,
    text: &str,
) {
    if !state.navigator.search_focused {
        return;
    }
    state.navigator.state_filter = None;
    state.navigator.query.push_str(text);
    state.select_first_navigator_match_from(terminal_runtimes);
}

pub(crate) fn insert_keybind_help_query_text(state: &mut AppState, text: &str) {
    if !state.keybind_help.search_focused {
        return;
    }
    state
        .keybind_help
        .query
        .extend(text.chars().filter(|ch| !ch.is_control()));
    state.keybind_help.scroll = 0;
}

pub(super) fn keybind_help_back(state: &mut AppState) {
    if state.keybind_help.search_focused {
        state.keybind_help.query.clear();
        state.keybind_help.search_focused = false;
        state.keybind_help.scroll = 0;
    } else {
        leave_modal(state);
    }
}

pub(crate) fn handle_keybind_help_key(state: &mut AppState, key: TerminalKey) {
    if state.keybind_help.search_focused {
        let text_char = keybind_help_text_char(key.clone());
        match key.code {
            KeyCode::Up => state.scroll_keybind_help(-1),
            KeyCode::Down => state.scroll_keybind_help(1),
            KeyCode::PageUp => state.scroll_keybind_help(-8),
            KeyCode::PageDown => state.scroll_keybind_help(8),
            KeyCode::Home => state.keybind_help.scroll = 0,
            KeyCode::End => state.keybind_help.scroll = state.keybind_help_max_scroll(),
            KeyCode::Backspace => {
                state.keybind_help.query.pop();
                state.keybind_help.scroll = 0;
            }
            KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => {
                state.keybind_help.query.clear();
                state.keybind_help.scroll = 0;
            }
            KeyCode::Esc => keybind_help_back(state),
            KeyCode::Enter => leave_modal(state),
            _ => {
                if let Some(character) = text_char {
                    insert_keybind_help_query_text(state, &character.to_string());
                }
            }
        }
        return;
    }

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => state.scroll_keybind_help(-1),
        KeyCode::Down | KeyCode::Char('j') => state.scroll_keybind_help(1),
        KeyCode::PageUp => state.scroll_keybind_help(-8),
        KeyCode::PageDown => state.scroll_keybind_help(8),
        KeyCode::Home => state.keybind_help.scroll = 0,
        KeyCode::End => state.keybind_help.scroll = state.keybind_help_max_scroll(),
        _ if keybind_help_text_char(key.clone()) == Some('/') => {
            state.keybind_help.search_focused = true;
            state.keybind_help.scroll = 0;
        }
        KeyCode::Esc => keybind_help_back(state),
        KeyCode::Enter => leave_modal(state),
        _ if keybind_help_text_char(key.clone()) == Some('?') => leave_modal(state),
        _ => {}
    }
}

fn keybind_help_text_char(key: TerminalKey) -> Option<char> {
    if !key.modifiers.difference(KeyModifiers::SHIFT).is_empty() {
        return None;
    }
    if let Some(character) = key.shifted_codepoint.and_then(char::from_u32) {
        return Some(character);
    }
    let KeyCode::Char(character) = key.code else {
        return None;
    };
    Some(character)
}

pub(super) fn open_rename_workspace(
    state: &mut AppState,
    terminal_runtimes: &crate::terminal::TerminalRuntimeRegistry,
    ws_idx: usize,
) {
    state.pending_workspace_create_cwd = None;
    state.selected = ws_idx;
    state.rename_pane_target = None;
    state.name_input =
        state.workspaces[ws_idx].display_name_from(&state.terminals, terminal_runtimes);
    state.name_input_replace_on_type = false;
    state.mode = Mode::RenameWorkspace;
}

pub(crate) fn open_new_workspace_dialog(state: &mut AppState, cwd: std::path::PathBuf) {
    let suggested_name = crate::workspace::derive_label_from_cwd(&cwd);
    state.creating_new_tab = false;
    state.requested_new_tab_name = None;
    state.pending_workspace_create_cwd = Some(cwd);
    state.rename_pane_target = None;
    state.name_input = suggested_name;
    state.name_input_replace_on_type = true;
    state.mode = Mode::RenameWorkspace;
}

pub(super) fn open_rename_active_tab(state: &mut AppState, replace_on_type: bool) {
    state.creating_new_tab = false;
    state.requested_new_tab_name = None;
    state.pending_workspace_create_cwd = None;
    state.rename_pane_target = None;
    if let Some(ws) = state.active.and_then(|i| state.workspaces.get(i)) {
        if let Some(name) = ws.active_tab_display_name() {
            state.name_input = name;
            state.name_input_replace_on_type = replace_on_type;
            state.mode = Mode::RenameTab;
        }
    }
}

pub(super) fn open_rename_pane(state: &mut AppState, pane_id: crate::layout::PaneId) {
    let Some(ws) = state.active.and_then(|i| state.workspaces.get(i)) else {
        return;
    };
    let Some(pane) = ws.pane_state(pane_id) else {
        return;
    };
    let terminal = state.terminals.get(&pane.attached_terminal_id);
    state.creating_new_tab = false;
    state.requested_new_tab_name = None;
    state.pending_workspace_create_cwd = None;
    state.rename_pane_target = Some(pane_id);
    state.name_input = terminal
        .and_then(|t| t.manual_label.clone())
        .unwrap_or_default();
    state.name_input_replace_on_type = terminal.and_then(|t| t.manual_label.as_ref()).is_none();
    state.mode = Mode::RenamePane;
}

fn workspace_create_label(input: &str, suggested_name: &str) -> Option<String> {
    let name = input.trim();
    (!name.is_empty() && name != suggested_name).then(|| name.to_string())
}

fn next_new_tab_default_name(state: &AppState) -> String {
    state
        .active
        .and_then(|i| state.workspaces.get(i))
        .map(|ws| (ws.tabs.len() + 1).to_string())
        .unwrap_or_else(|| "1".to_string())
}

pub(super) fn open_new_tab_dialog(state: &mut AppState) {
    state.creating_new_tab = true;
    state.requested_new_tab_name = None;
    state.pending_workspace_create_cwd = None;
    state.rename_pane_target = None;
    state.name_input = next_new_tab_default_name(state);
    state.name_input_replace_on_type = true;
    state.mode = Mode::RenameTab;
}

pub(super) fn leave_modal(state: &mut AppState) {
    if state.active.is_some() {
        state.mode = Mode::Terminal;
    } else {
        state.mode = Mode::Navigate;
    }
}

pub(super) const ONBOARDING_WELCOME_ACTIONS: &[ModalActionSpec<ModalAction>] = &[ModalActionSpec {
    action: ModalAction::Continue,
    bindings: &[ModalKeyBinding::Enter],
}];

pub(super) const RELEASE_NOTES_ACTIONS: &[ModalActionSpec<ModalAction>] = &[ModalActionSpec {
    action: ModalAction::Close,
    bindings: &[ModalKeyBinding::Enter, ModalKeyBinding::Esc],
}];

pub(super) const RENAME_ACTIONS: &[ModalActionSpec<ModalAction>] = &[
    ModalActionSpec {
        action: ModalAction::Save,
        bindings: &[ModalKeyBinding::Enter],
    },
    ModalActionSpec {
        action: ModalAction::Clear,
        bindings: &[ModalKeyBinding::CtrlC],
    },
    ModalActionSpec {
        action: ModalAction::Cancel,
        bindings: &[ModalKeyBinding::Esc],
    },
];

pub(super) const CONFIRM_CLOSE_ACTIONS: &[ModalActionSpec<ModalAction>] = &[
    ModalActionSpec {
        action: ModalAction::Confirm,
        bindings: &[ModalKeyBinding::Enter],
    },
    ModalActionSpec {
        action: ModalAction::Cancel,
        bindings: &[ModalKeyBinding::Esc],
    },
];

pub(super) const SETTINGS_ACTIONS: &[ModalActionSpec<ModalAction>] = &[
    ModalActionSpec {
        action: ModalAction::Apply,
        bindings: &[ModalKeyBinding::Enter],
    },
    ModalActionSpec {
        action: ModalAction::Close,
        bindings: &[ModalKeyBinding::Esc],
    },
];

#[cfg(test)]
pub(super) fn apply_rename_action(state: &mut AppState, action: ModalAction) {
    match action {
        ModalAction::Save => {
            let new_name = if state.name_input.trim().is_empty() {
                state.name_input.clone()
            } else {
                state.name_input.trim().to_string()
            };
            match state.mode {
                Mode::RenameWorkspace
                    if state.pending_workspace_create_cwd.is_none()
                        && !state.workspaces.is_empty()
                        && !new_name.is_empty() =>
                {
                    let workspace_id = state.workspaces[state.selected].id.clone();
                    state.workspaces[state.selected].set_custom_name(new_name);
                    crate::logging::workspace_renamed(&workspace_id);
                    state.mark_session_dirty();
                }
                Mode::RenameTab if state.creating_new_tab => {
                    state.request_new_tab = true;
                    let default_name = next_new_tab_default_name(state);
                    state.requested_new_tab_name =
                        if new_name.is_empty() || new_name == default_name {
                            None
                        } else {
                            Some(new_name)
                        };
                }
                Mode::RenameTab => {
                    if let Some(ws_idx) = state.active {
                        if let Some(ws) = state.workspaces.get_mut(ws_idx) {
                            let workspace_id = ws.id.clone();
                            let active_tab = ws.active_tab;
                            let keep_auto_name = ws
                                .tabs
                                .get(active_tab)
                                .is_some_and(|tab| tab.is_auto_named())
                                && ws
                                    .tab_display_name(active_tab)
                                    .is_some_and(|name| new_name == name);
                            if let Some(tab) = ws.active_tab_mut() {
                                if !new_name.is_empty() && !keep_auto_name {
                                    tab.set_custom_name(new_name);
                                    let tab_id = ws
                                        .public_tab_number(active_tab)
                                        .map(|number| {
                                            crate::workspace::public_tab_id_for_number(
                                                &workspace_id,
                                                number,
                                            )
                                        })
                                        .unwrap_or_else(|| workspace_id.clone());
                                    crate::logging::tab_renamed(&workspace_id, &tab_id);
                                    state.mark_session_dirty();
                                }
                            }
                        }
                    }
                }
                Mode::RenamePane => {
                    if let (Some(ws_idx), Some(pane_id)) = (state.active, state.rename_pane_target)
                    {
                        if let Some(ws) = state.workspaces.get(ws_idx) {
                            if let Some(pane) = ws.pane_state(pane_id) {
                                let terminal_id = pane.attached_terminal_id.clone();
                                if let Some(terminal) = state.terminals.get_mut(&terminal_id) {
                                    terminal.set_manual_label(new_name);
                                    state.mark_session_dirty();
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
            state.creating_new_tab = false;
            state.pending_workspace_create_cwd = None;
            state.rename_pane_target = None;
            state.name_input.clear();
            state.name_input_replace_on_type = false;
            leave_modal(state);
        }
        ModalAction::Clear => {
            state.name_input.clear();
            state.name_input_replace_on_type = false;
        }
        ModalAction::Cancel => {
            state.creating_new_tab = false;
            state.requested_new_tab_name = None;
            state.pending_workspace_create_cwd = None;
            state.rename_pane_target = None;
            state.name_input.clear();
            state.name_input_replace_on_type = false;
            leave_modal(state);
        }
        _ => {}
    }
}

fn clear_rename_input(state: &mut AppState) {
    state.name_input.clear();
    state.name_input_replace_on_type = false;
}

pub(crate) fn insert_rename_input_text(state: &mut AppState, text: &str) {
    if state.name_input_replace_on_type {
        clear_rename_input(state);
    }
    state.name_input.push_str(text);
}

fn delete_rename_input_char(state: &mut AppState) {
    if state.name_input_replace_on_type {
        clear_rename_input(state);
    } else {
        state.name_input.pop();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WordDeleteClass {
    Word,
    Separator,
}

fn word_delete_class(ch: char) -> WordDeleteClass {
    if ch.is_alphanumeric() || ch == '_' {
        WordDeleteClass::Word
    } else {
        WordDeleteClass::Separator
    }
}

fn delete_rename_input_word(state: &mut AppState) {
    if state.name_input_replace_on_type {
        clear_rename_input(state);
        return;
    }
    delete_last_word(&mut state.name_input);
}

/// Delete trailing whitespace, then the run of like-classed characters before
/// it. Shared by the rename modal and the todo edit modal.
fn delete_last_word(buffer: &mut String) {
    while buffer.chars().last().is_some_and(char::is_whitespace) {
        buffer.pop();
    }
    let Some(class) = buffer.chars().last().map(word_delete_class) else {
        return;
    };
    while buffer
        .chars()
        .last()
        .is_some_and(|ch| !ch.is_whitespace() && word_delete_class(ch) == class)
    {
        buffer.pop();
    }
}

fn handle_pane_todo_edit_text_key(state: &mut AppState, key: KeyEvent) {
    let Some(edit) = state.pane_todo_edit.as_mut() else {
        return;
    };
    match key.code {
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => edit.text.clear(),
        KeyCode::Backspace if key.modifiers.contains(KeyModifiers::SUPER) => edit.text.clear(),
        KeyCode::Backspace
            if key.modifiers.contains(KeyModifiers::CONTROL)
                || key.modifiers.contains(KeyModifiers::ALT) =>
        {
            delete_last_word(&mut edit.text);
        }
        KeyCode::Char('h' | 'w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            delete_last_word(&mut edit.text);
        }
        KeyCode::Backspace => {
            edit.text.pop();
        }
        // The length guard sits in the pattern so a full buffer simply stops
        // matching: stopping at the store's limit means the modal can never
        // compose a todo the server will reject.
        KeyCode::Char(c)
            if key.modifiers.difference(KeyModifiers::SHIFT).is_empty()
                && edit.text.chars().count() < crate::terminal::todo::MAX_TODO_TEXT_LEN =>
        {
            edit.text.push(c);
        }
        _ => {}
    }
}

fn handle_rename_edit_key(state: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            clear_rename_input(state);
        }
        KeyCode::Backspace if key.modifiers.contains(KeyModifiers::SUPER) => {
            clear_rename_input(state);
        }
        KeyCode::Backspace
            if key.modifiers.contains(KeyModifiers::CONTROL)
                || key.modifiers.contains(KeyModifiers::ALT) =>
        {
            delete_rename_input_word(state);
        }
        KeyCode::Char('h' | 'w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            delete_rename_input_word(state);
        }
        KeyCode::Backspace => delete_rename_input_char(state),
        KeyCode::Char(c) if key.modifiers.difference(KeyModifiers::SHIFT).is_empty() => {
            insert_rename_input_text(state, &c.to_string());
        }
        _ => {}
    }
}

#[cfg(test)]
pub(crate) fn handle_rename_key(state: &mut AppState, key: KeyEvent) {
    if let Some(action) = modal_action_from_key(&key, RENAME_ACTIONS) {
        apply_rename_action(state, action);
        return;
    }

    handle_rename_edit_key(state, key);
}

#[cfg(test)]
pub(crate) fn handle_resize_key(state: &mut AppState, raw_key: TerminalKey) {
    let key = raw_key.as_key_event();
    if key.code == KeyCode::Esc
        || key.code == KeyCode::Enter
        || state.keybinds.resize_mode.matches_prefix_key(&raw_key)
        || state.keybinds.resize_mode.matches_direct_key(&raw_key)
    {
        if state.active.is_some() {
            state.mode = Mode::Terminal;
        } else {
            state.mode = Mode::Navigate;
        }
        return;
    }

    match key.code {
        KeyCode::Char('h') | KeyCode::Left => state.resize_pane(NavDirection::Left),
        KeyCode::Char('l') | KeyCode::Right => state.resize_pane(NavDirection::Right),
        KeyCode::Char('j') | KeyCode::Down => state.resize_pane(NavDirection::Down),
        KeyCode::Char('k') | KeyCode::Up => state.resize_pane(NavDirection::Up),
        _ => {}
    }
}

pub(super) fn open_confirm_close(state: &mut AppState) {
    state.begin_workspace_close_confirmation(state.selected);
}

#[cfg(test)]
pub(super) fn confirm_close_accept(state: &mut AppState) {
    if let Some(ws_idx) = state.take_confirmed_workspace_close_index() {
        state.selected = ws_idx;
        state.close_selected_workspace();
    }
    if state.workspaces.is_empty() {
        state.mode = Mode::Navigate;
    } else {
        state.mode = Mode::Terminal;
    }
}

pub(super) fn confirm_close_cancel(state: &mut AppState) {
    state.confirm_close_workspace_id = None;
    state.confirm_close_pane = None;
    state.mode = Mode::Navigate;
}

#[cfg(test)]
pub(crate) fn handle_confirm_close_key(state: &mut AppState, key: KeyEvent) {
    match modal_action_from_key(&key, CONFIRM_CLOSE_ACTIONS) {
        Some(ModalAction::Confirm) => confirm_close_accept(state),
        Some(ModalAction::Cancel) => confirm_close_cancel(state),
        _ => {}
    }
}

#[cfg(test)]
pub(super) fn apply_context_menu_action(
    state: &mut AppState,
    terminal_runtimes: &mut crate::terminal::TerminalRuntimeRegistry,
    menu: ContextMenuState,
    idx: usize,
) {
    let item = menu.items().get(idx).copied();
    match (menu.kind, item) {
        (ContextMenuKind::GitWorkspace { ws_idx, .. }, Some("New worktree")) => {
            state.request_new_linked_worktree = Some(ws_idx);
            leave_modal(state);
        }
        (ContextMenuKind::GitWorkspace { ws_idx, .. }, Some("Delete worktree checkout...")) => {
            state.request_remove_linked_worktree = Some(ws_idx);
            leave_modal(state);
        }
        (ContextMenuKind::GitWorkspace { ws_idx, .. }, Some("Open worktree...")) => {
            state.request_open_existing_worktree = Some(ws_idx);
            leave_modal(state);
        }
        (
            ContextMenuKind::GitWorkspace {
                ws_idx, collapsed, ..
            },
            Some("Collapse" | "Expand"),
        ) => {
            if let Some(key) = state
                .workspaces
                .get(ws_idx)
                .and_then(|ws| ws.worktree_space())
                .map(|space| space.key.clone())
            {
                if collapsed {
                    state.collapsed_space_keys.remove(&key);
                } else {
                    state.collapsed_space_keys.insert(key);
                }
                state.mark_session_dirty();
            }
            leave_modal(state);
        }
        (
            ContextMenuKind::Workspace { ws_idx } | ContextMenuKind::GitWorkspace { ws_idx, .. },
            Some("Rename"),
        ) => {
            open_rename_workspace(state, terminal_runtimes, ws_idx);
        }
        (
            ContextMenuKind::Workspace { ws_idx } | ContextMenuKind::GitWorkspace { ws_idx, .. },
            Some("Close" | "Close group"),
        ) => {
            state.selected = ws_idx;
            if state.confirm_close {
                open_confirm_close(state);
            } else {
                state.close_selected_workspace();
                state.mode = Mode::Navigate;
            }
        }
        (ContextMenuKind::Tab { ws_idx, tab_idx }, Some("New tab")) => {
            state.selected = ws_idx;
            state.active = Some(ws_idx);
            state.switch_tab(tab_idx);
            open_new_tab_dialog(state);
        }
        (ContextMenuKind::Tab { ws_idx, tab_idx }, Some("Rename")) => {
            state.selected = ws_idx;
            state.active = Some(ws_idx);
            state.switch_tab(tab_idx);
            open_rename_active_tab(state, false);
        }
        (ContextMenuKind::Tab { ws_idx, tab_idx }, Some("Close")) => {
            state.selected = ws_idx;
            state.active = Some(ws_idx);
            state.switch_tab(tab_idx);
            if !state.close_tab() {
                state.mode = if state.active.is_some() {
                    Mode::Terminal
                } else {
                    Mode::Navigate
                };
            }
        }
        (ContextMenuKind::Pane { pane_id, .. }, Some("Rename pane")) => {
            open_rename_pane(state, pane_id);
        }
        (
            ContextMenuKind::Pane {
                ws_idx, pane_id, ..
            },
            Some("Clear pane name"),
        ) => {
            if let Some(ws) = state.workspaces.get(ws_idx) {
                if let Some(pane) = ws.pane_state(pane_id) {
                    let terminal_id = pane.attached_terminal_id.clone();
                    if let Some(terminal) = state.terminals.get_mut(&terminal_id) {
                        terminal.clear_manual_label();
                        state.mark_session_dirty();
                    }
                }
            }
            state.mode = Mode::Terminal;
        }
        (
            ContextMenuKind::Pane {
                ws_idx,
                tab_idx,
                pane_id,
                source_pane_id,
                ..
            },
            Some("Swap with focused pane"),
        ) => {
            if let Some(source_pane_id) = source_pane_id {
                state.selected = ws_idx;
                state.active = Some(ws_idx);
                state.switch_tab(tab_idx);
                if let Some(tab) = state
                    .workspaces
                    .get_mut(ws_idx)
                    .and_then(|ws| ws.tabs.get_mut(tab_idx))
                {
                    if tab.layout.swap_panes(source_pane_id, pane_id) {
                        tab.layout.focus_pane(source_pane_id);
                        state.mark_session_dirty();
                    }
                }
            }
            state.mode = Mode::Terminal;
        }
        (
            ContextMenuKind::Pane {
                ws_idx,
                tab_idx,
                pane_id,
                ..
            },
            Some("Split right"),
        ) => {
            state.selected = ws_idx;
            state.active = Some(ws_idx);
            state.switch_tab(tab_idx);
            state.focus_pane_in_workspace(ws_idx, pane_id);
            state.split_pane(terminal_runtimes, Direction::Horizontal);
            state.mode = Mode::Terminal;
        }
        (
            ContextMenuKind::Pane {
                ws_idx,
                tab_idx,
                pane_id,
                ..
            },
            Some("Split down"),
        ) => {
            state.selected = ws_idx;
            state.active = Some(ws_idx);
            state.switch_tab(tab_idx);
            state.focus_pane_in_workspace(ws_idx, pane_id);
            state.split_pane(terminal_runtimes, Direction::Vertical);
            state.mode = Mode::Terminal;
        }
        (
            ContextMenuKind::Pane {
                ws_idx,
                tab_idx,
                pane_id,
                ..
            },
            Some("Zoom"),
        ) => {
            state.selected = ws_idx;
            state.active = Some(ws_idx);
            state.switch_tab(tab_idx);
            state.focus_pane_in_workspace(ws_idx, pane_id);
            state.toggle_zoom();
            state.mode = Mode::Terminal;
        }
        (
            ContextMenuKind::Pane {
                ws_idx,
                tab_idx,
                pane_id,
                ..
            },
            Some("Close pane"),
        ) => {
            state.selected = ws_idx;
            state.active = Some(ws_idx);
            state.switch_tab(tab_idx);
            state.focus_pane_in_workspace(ws_idx, pane_id);
            if !state.close_pane() {
                state.mode = if state.active.is_some() {
                    Mode::Terminal
                } else {
                    Mode::Navigate
                };
            }
        }
        _ => leave_modal(state),
    }
}

#[cfg(test)]
pub(crate) fn handle_notification_center_key(state: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => state.notification_center_move_selection(-1),
        KeyCode::Down | KeyCode::Char('j') => state.notification_center_move_selection(1),
        KeyCode::Enter => activate_notification_center_selection_local(state),
        KeyCode::Char('c') => state.clear_notifications(),
        KeyCode::Char('r') => state.mark_all_notifications_read(),
        KeyCode::Esc | KeyCode::Char('q') => {
            state.close_notification_center();
            leave_modal(state);
        }
        _ => {}
    }
}

/// Pure-state twin of `App::activate_notification_center_selection` for
/// tests without an API runtime.
#[cfg(test)]
fn activate_notification_center_selection_local(state: &mut AppState) {
    let Some((entry_id, target)) = state
        .notification_center_selected_entry()
        .and_then(|entry| Some((entry.id, entry.target.clone()?)))
    else {
        return;
    };
    let Some(ws_idx) = state
        .workspaces
        .iter()
        .position(|workspace| workspace.id == target.workspace_id)
    else {
        return;
    };
    state.notification_log.mark_read(entry_id);
    state.close_notification_center();
    state.focus_pane_in_workspace(ws_idx, target.pane_id);
    state.mode = Mode::Terminal;
}

#[cfg(test)]
pub(crate) fn handle_context_menu_key(
    state: &mut AppState,
    terminal_runtimes: &mut crate::terminal::TerminalRuntimeRegistry,
    key: KeyEvent,
) {
    match key.code {
        KeyCode::Esc => {
            state.context_menu = None;
            leave_modal(state);
        }
        KeyCode::Up => {
            if let Some(menu) = &mut state.context_menu {
                menu.list.move_prev();
            }
        }
        KeyCode::Down => {
            if let Some(menu) = &mut state.context_menu {
                menu.list.move_next(menu.items().len());
            }
        }
        KeyCode::Enter => {
            if let Some(menu) = state.context_menu.take() {
                let idx = menu.list.highlighted;
                apply_context_menu_action(state, terminal_runtimes, menu, idx);
            }
        }
        _ => {}
    }
}

impl App {
    pub(crate) fn handle_notification_center_key_via_api(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.state.notification_center_move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.state.notification_center_move_selection(1),
            KeyCode::Enter => self.activate_notification_center_selection(),
            KeyCode::Char('c') => self.state.clear_notifications(),
            KeyCode::Char('r') => self.state.mark_all_notifications_read(),
            KeyCode::Esc | KeyCode::Char('q') => {
                self.state.close_notification_center();
                leave_modal(&mut self.state);
            }
            _ => {}
        }
    }

    pub(crate) fn handle_pane_todos_key_via_api(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.state.pane_todos_move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.state.pane_todos_move_selection(1),
            // Enter edits rather than jumps: todos are authored, notifications
            // are not. Jumping is on the link chip and `g`.
            KeyCode::Enter => self.apply_pane_todo_action(PaneTodoAction::Edit),
            KeyCode::Char('a') => self.apply_pane_todo_action(PaneTodoAction::Add),
            KeyCode::Char(' ') => self.apply_pane_todo_action(PaneTodoAction::ToggleDone),
            KeyCode::Char('g') => self.apply_pane_todo_action(PaneTodoAction::FollowLink),
            KeyCode::Char('d') => self.apply_pane_todo_action(PaneTodoAction::Remove),
            KeyCode::Char('c') => self.apply_pane_todo_action(PaneTodoAction::ClearDone),
            KeyCode::Esc | KeyCode::Char('q') => {
                self.state.close_pane_todos();
                leave_modal(&mut self.state);
            }
            _ => {}
        }
    }

    /// Apply a panel action to the selected todo. Every mutation goes back
    /// through the `todo.*` API, so the panel, the CLI, and subscribers all
    /// move the same state and `todo.changed` is emitted for free.
    pub(super) fn apply_pane_todo_action(&mut self, action: PaneTodoAction) {
        let Some(pane_id) = self.state.pane_todos.as_ref().map(|panel| panel.pane_id) else {
            return;
        };
        let Some(ws_idx) = self.state.active else {
            return;
        };
        let Some(public_pane_id) = self.public_pane_id(ws_idx, pane_id) else {
            return;
        };

        // Add acts on the pane, not on a selection, so it runs before the
        // selected-todo lookup below — which is what lets an empty panel add.
        if action == PaneTodoAction::Add {
            self.state.open_new_pane_todo(pane_id);
            return;
        }

        if action == PaneTodoAction::ClearDone {
            self.runtime_todo_clear(
                "tui.todo.clear",
                crate::api::schema::TodoClearParams {
                    pane_id: public_pane_id,
                    done_only: true,
                },
            );
            // The list just shrank under the cursor.
            self.state.pane_todos_move_selection(0);
            return;
        }

        let Some(todo) = self.state.selected_pane_todo() else {
            return;
        };
        match action {
            // Handled above, before the selected-todo lookup.
            PaneTodoAction::Add => {}
            PaneTodoAction::Edit => self.state.open_pane_todo_edit(pane_id, todo.id),
            PaneTodoAction::ToggleDone => {
                self.runtime_todo_update(
                    "tui.todo.update",
                    crate::api::schema::TodoUpdateParams {
                        pane_id: public_pane_id,
                        id: todo.id,
                        done: Some(!todo.done),
                        ..Default::default()
                    },
                );
                // Toggling re-sorts the list (done sinks), so re-clamp.
                self.state.pane_todos_move_selection(0);
            }
            PaneTodoAction::Remove => {
                self.runtime_todo_remove(
                    "tui.todo.remove",
                    crate::api::schema::TodoRemoveParams {
                        pane_id: public_pane_id,
                        id: todo.id,
                    },
                );
                self.state.pane_todos_move_selection(0);
            }
            PaneTodoAction::FollowLink => {
                // A dead link is inert: it keeps its label and never resolves
                // to some other pane.
                let Some((target_ws_idx, target_pane_id)) = self.state.pane_todo_link_target(&todo)
                else {
                    return;
                };
                self.state.close_pane_todos();
                self.focus_pane_internal_via_api(target_ws_idx, target_pane_id);
                self.state.mode = Mode::Terminal;
            }
            PaneTodoAction::ClearDone => {}
        }
    }

    pub(crate) fn handle_pane_todo_edit_key_via_api(&mut self, key: KeyEvent) {
        // Commands before text, like `handle_rename_key_via_api`. Anything
        // carrying CTRL/ALT/SUPER can never be swallowed by the text field.
        match key.code {
            KeyCode::Enter => {
                self.save_pane_todo_edit_via_api();
                return;
            }
            KeyCode::Esc => {
                self.close_pane_todo_edit_and_return();
                return;
            }
            KeyCode::Tab => {
                self.state.cycle_pane_todo_edit_priority();
                return;
            }
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state
                    .open_pane_todo_link_picker_from(&self.terminal_runtimes);
                return;
            }
            // Space belongs to the text field here, so the panel's `space`
            // toggle needs a modifier of its own inside the modal.
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.toggle_pane_todo_edit_done();
                return;
            }
            _ => {}
        }
        handle_pane_todo_edit_text_key(&mut self.state, key);
    }

    /// Leave the modal back to the panel it was opened from, or to the
    /// terminal when it was opened straight from a keybinding.
    pub(super) fn close_pane_todo_edit_and_return(&mut self) {
        self.state.close_pane_todo_edit();
        if self.state.pane_todos.is_some() {
            self.state.mode = Mode::PaneTodos;
        } else {
            leave_modal(&mut self.state);
        }
    }

    fn save_pane_todo_edit_via_api(&mut self) {
        let Some((pane_id, todo_id, text, priority, link, done)) =
            self.state.pane_todo_edit.as_ref().map(|edit| {
                (
                    edit.pane_id,
                    edit.todo_id,
                    edit.text.trim().to_string(),
                    edit.priority,
                    edit.link,
                    edit.done,
                )
            })
        else {
            return;
        };
        if text.is_empty() {
            // The store rejects empty text; keep the modal open rather than
            // silently dropping what was typed.
            return;
        }
        let Some(ws_idx) = self.state.active else {
            return;
        };
        let Some(public_pane_id) = self.public_pane_id(ws_idx, pane_id) else {
            return;
        };
        // Resolved against the target's own workspace, not `ws_idx`: a link
        // may point at a pane in any workspace, and scoping the lookup to the
        // active one drops it here with no error.
        let link_pane_id = match link {
            crate::app::state::PaneTodoEditLink::Set(target) => self.session_public_pane_id(target),
            _ => None,
        };
        let clear_link = matches!(link, crate::app::state::PaneTodoEditLink::Clear);

        let response = match todo_id {
            Some(id) => self.runtime_todo_update(
                "tui.todo.update",
                crate::api::schema::TodoUpdateParams {
                    pane_id: public_pane_id,
                    id,
                    text: Some(text),
                    done: Some(done),
                    priority: Some(priority),
                    link_pane_id,
                    clear_link,
                },
            ),
            None => self.runtime_todo_add(
                "tui.todo.add",
                crate::api::schema::TodoAddParams {
                    pane_id: public_pane_id,
                    text,
                    priority: Some(priority),
                    link_pane_id,
                },
            ),
        };
        // The store can still refuse a save the modal cannot pre-check — text
        // over the length cap, the 50-todo per-pane limit, or a todo removed
        // from under the edit. Closing on a rejection would throw away what was
        // typed with no explanation, so say why and leave the modal open.
        if let Ok(error) = serde_json::from_str::<crate::api::schema::ErrorResponse>(&response) {
            tracing::debug!(code = %error.error.code, "pane todo save rejected");
            self.show_pane_move_feedback("todo save failed", error.error.message);
            return;
        }
        self.close_pane_todo_edit_and_return();
        self.state.pane_todos_move_selection(0);
    }

    pub(super) fn apply_pane_todo_edit_action_via_api(&mut self, action: ModalAction) {
        match action {
            ModalAction::Save => self.save_pane_todo_edit_via_api(),
            ModalAction::Cancel => self.close_pane_todo_edit_and_return(),
            _ => {}
        }
    }

    pub(crate) fn handle_rename_key_via_api(&mut self, key: KeyEvent) {
        if let Some(action) = modal_action_from_key(&key, RENAME_ACTIONS) {
            self.apply_rename_mouse_action_via_api(action);
            return;
        }

        handle_rename_edit_key(&mut self.state, key);
    }

    fn save_rename_modal_via_api(&mut self) {
        let new_name = if self.state.name_input.trim().is_empty() {
            self.state.name_input.clone()
        } else {
            self.state.name_input.trim().to_string()
        };

        match self.state.mode {
            Mode::RenameWorkspace => {
                if let Some(cwd) = self.state.pending_workspace_create_cwd.take() {
                    let suggested_name = crate::workspace::derive_label_from_cwd(&cwd);
                    let label = workspace_create_label(&new_name, &suggested_name);
                    self.runtime_workspace_create(
                        "tui.workspace.create_named",
                        crate::api::schema::WorkspaceCreateParams {
                            cwd: Some(cwd.display().to_string()),
                            focus: true,
                            label,
                            env: Default::default(),
                        },
                    );
                } else if !self.state.workspaces.is_empty() && !new_name.is_empty() {
                    let workspace_id = self.public_workspace_id(self.state.selected);
                    self.runtime_workspace_rename(
                        "tui.workspace.rename",
                        crate::api::schema::WorkspaceRenameParams {
                            workspace_id,
                            label: new_name,
                        },
                    );
                }
            }
            Mode::RenameTab if self.state.creating_new_tab => {
                let default_name = next_new_tab_default_name(&self.state);
                let label = if new_name.is_empty() || new_name == default_name {
                    None
                } else {
                    Some(new_name)
                };
                self.runtime_tab_create(
                    "tui.tab.create_named",
                    crate::api::schema::TabCreateParams {
                        workspace_id: None,
                        cwd: None,
                        focus: true,
                        label,
                        env: Default::default(),
                    },
                );
            }
            Mode::RenameTab if !new_name.is_empty() => {
                let Some(ws_idx) = self.state.active else {
                    cancel_rename_modal(&mut self.state);
                    return;
                };
                let tab_idx = self.state.workspaces[ws_idx].active_tab;
                let keep_auto_name = self.state.workspaces[ws_idx]
                    .tabs
                    .get(tab_idx)
                    .is_some_and(|tab| tab.is_auto_named())
                    && self.state.workspaces[ws_idx]
                        .tab_display_name(tab_idx)
                        .is_some_and(|name| new_name == name);
                if !keep_auto_name {
                    if let Some(tab_id) = self.public_tab_id(ws_idx, tab_idx) {
                        self.runtime_tab_rename(
                            "tui.tab.rename",
                            crate::api::schema::TabRenameParams {
                                tab_id,
                                label: new_name,
                            },
                        );
                    }
                }
            }
            Mode::RenamePane => {
                if let (Some(ws_idx), Some(pane_id)) =
                    (self.state.active, self.state.rename_pane_target)
                {
                    if let Some(pane_id) = self.public_pane_id(ws_idx, pane_id) {
                        self.runtime_pane_rename(
                            "tui.pane.rename",
                            crate::api::schema::PaneRenameParams {
                                pane_id,
                                label: Some(new_name),
                            },
                        );
                    }
                }
            }
            _ => {}
        }

        cancel_rename_modal(&mut self.state);
    }

    pub(super) fn apply_rename_mouse_action_via_api(&mut self, action: ModalAction) {
        match action {
            ModalAction::Save => self.save_rename_modal_via_api(),
            ModalAction::Clear => {
                self.state.name_input.clear();
                self.state.name_input_replace_on_type = false;
            }
            ModalAction::Cancel => cancel_rename_modal(&mut self.state),
            _ => {}
        }
    }

    pub(super) fn confirm_close_accept_via_api(&mut self) {
        // A pending pane confirmation is what is on screen; the retry re-enters
        // close_pane, which consumes the token and proceeds. Without this branch
        // the modal would close the whole workspace.
        if let Some(pane_id) = self.state.confirm_close_pane {
            let ws_idx = self.state.selected;
            match self.public_pane_id(ws_idx, pane_id) {
                Some(public_pane_id) => {
                    self.runtime_pane_close("tui.pane.close", public_pane_id);
                }
                None => self.state.confirm_close_pane = None,
            }
            self.state.mode = if self.state.active.is_some() {
                Mode::Terminal
            } else {
                Mode::Navigate
            };
            return;
        }

        if let Some(ws_idx) = self.state.take_confirmed_workspace_close_index() {
            self.close_workspace_idx_with_group_via_api(ws_idx);
        }
        self.state.mode = if self.state.active.is_some() {
            Mode::Terminal
        } else {
            Mode::Navigate
        };
    }

    pub(crate) fn handle_resize_key_via_api(&mut self, raw_key: TerminalKey) {
        let key = raw_key.as_key_event();
        if key.code == KeyCode::Esc
            || key.code == KeyCode::Enter
            || self.state.keybinds.resize_mode.matches_prefix_key(&raw_key)
            || self.state.keybinds.resize_mode.matches_direct_key(&raw_key)
        {
            self.state.mode = if self.state.active.is_some() {
                Mode::Terminal
            } else {
                Mode::Navigate
            };
            return;
        }

        let direction = match key.code {
            KeyCode::Char('h') | KeyCode::Left => Some(NavDirection::Left),
            KeyCode::Char('l') | KeyCode::Right => Some(NavDirection::Right),
            KeyCode::Char('j') | KeyCode::Down => Some(NavDirection::Down),
            KeyCode::Char('k') | KeyCode::Up => Some(NavDirection::Up),
            _ => None,
        };
        if let Some(direction) = direction {
            self.runtime_pane_resize(
                "tui.pane.resize",
                crate::api::schema::PaneResizeParams {
                    pane_id: None,
                    direction: super::navigate::api_pane_direction(direction),
                    amount: None,
                },
            );
        }
    }

    pub(crate) fn handle_confirm_close_key_via_api(&mut self, key: KeyEvent) {
        match modal_action_from_key(&key, CONFIRM_CLOSE_ACTIONS) {
            Some(ModalAction::Confirm) => {
                self.confirm_close_accept_via_api();
            }
            Some(ModalAction::Cancel) => confirm_close_cancel(&mut self.state),
            _ => {}
        }
    }

    pub(crate) fn handle_context_menu_key_via_api(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.state.context_menu = None;
                leave_modal(&mut self.state);
            }
            KeyCode::Up => {
                if let Some(menu) = &mut self.state.context_menu {
                    menu.list.move_prev();
                }
            }
            KeyCode::Down => {
                if let Some(menu) = &mut self.state.context_menu {
                    menu.list.move_next(menu.items().len());
                }
            }
            KeyCode::Enter => {
                if let Some(menu) = self.state.context_menu.take() {
                    let idx = menu.list.highlighted;
                    self.apply_context_menu_action_via_api(menu, idx);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn apply_context_menu_action_via_api(&mut self, menu: ContextMenuState, idx: usize) {
        let item = menu.items().get(idx).copied();
        match (menu.kind, item) {
            (ContextMenuKind::GitWorkspace { ws_idx, .. }, Some("New worktree")) => {
                self.state.request_new_linked_worktree = Some(ws_idx);
                leave_modal(&mut self.state);
            }
            (ContextMenuKind::GitWorkspace { ws_idx, .. }, Some("Delete worktree checkout...")) => {
                self.state.request_remove_linked_worktree = Some(ws_idx);
                leave_modal(&mut self.state);
            }
            (ContextMenuKind::GitWorkspace { ws_idx, .. }, Some("Open worktree...")) => {
                self.state.request_open_existing_worktree = Some(ws_idx);
                leave_modal(&mut self.state);
            }
            (
                ContextMenuKind::GitWorkspace {
                    ws_idx, collapsed, ..
                },
                Some("Collapse" | "Expand"),
            ) => {
                if let Some(key) = self
                    .state
                    .workspaces
                    .get(ws_idx)
                    .and_then(|ws| ws.worktree_space())
                    .map(|space| space.key.clone())
                {
                    if collapsed {
                        self.state.collapsed_space_keys.remove(&key);
                    } else {
                        self.state.collapsed_space_keys.insert(key);
                    }
                    self.state.mark_session_dirty();
                }
                leave_modal(&mut self.state);
            }
            (
                ContextMenuKind::Workspace { ws_idx }
                | ContextMenuKind::GitWorkspace { ws_idx, .. },
                Some("Rename"),
            ) => open_rename_workspace(&mut self.state, &self.terminal_runtimes, ws_idx),
            (
                ContextMenuKind::Workspace { ws_idx }
                | ContextMenuKind::GitWorkspace { ws_idx, .. },
                Some("Close" | "Close group"),
            ) => {
                self.state.selected = ws_idx;
                if self.state.confirm_close {
                    open_confirm_close(&mut self.state);
                } else {
                    self.close_workspace_idx_with_group_via_api(ws_idx);
                    self.state.mode = Mode::Navigate;
                }
            }
            (ContextMenuKind::Tab { ws_idx, tab_idx }, Some("New tab")) => {
                self.focus_workspace_idx_via_api(ws_idx);
                self.focus_tab_idx_via_api(tab_idx);
                open_new_tab_dialog(&mut self.state);
            }
            (ContextMenuKind::Tab { ws_idx, tab_idx }, Some("Rename")) => {
                self.focus_workspace_idx_via_api(ws_idx);
                self.focus_tab_idx_via_api(tab_idx);
                open_rename_active_tab(&mut self.state, false);
            }
            (ContextMenuKind::Tab { ws_idx, tab_idx }, Some("Close")) => {
                self.focus_workspace_idx_via_api(ws_idx);
                self.focus_tab_idx_via_api(tab_idx);
                if !self.close_active_tab_via_api_requires_confirmation() {
                    leave_modal(&mut self.state);
                }
            }
            (ContextMenuKind::Pane { pane_id, .. }, Some("Rename pane")) => {
                open_rename_pane(&mut self.state, pane_id);
            }
            (
                ContextMenuKind::Pane {
                    ws_idx, pane_id, ..
                },
                Some("Clear pane name"),
            ) => {
                if let Some(pane_id) = self.public_pane_id(ws_idx, pane_id) {
                    self.runtime_pane_rename(
                        "tui.pane.clear_name",
                        crate::api::schema::PaneRenameParams {
                            pane_id,
                            label: None,
                        },
                    );
                }
                self.state.mode = Mode::Terminal;
            }
            (
                ContextMenuKind::Pane {
                    ws_idx, pane_id, ..
                },
                Some(action @ ("Send right-clicks to pane" | "Use Herdr right-click menu")),
            ) => {
                if let Some(pane_id) = self.public_pane_id(ws_idx, pane_id) {
                    self.runtime_pane_input_set(
                        "tui.pane.input.set",
                        crate::api::schema::PaneInputSetParams {
                            pane_id,
                            right_click: if action == "Send right-clicks to pane" {
                                crate::api::schema::PaneRightClickTarget::Pane
                            } else {
                                crate::api::schema::PaneRightClickTarget::Herdr
                            },
                        },
                    );
                }
                self.state.mode = Mode::Terminal;
            }
            (
                ContextMenuKind::Pane {
                    ws_idx,
                    pane_id,
                    source_pane_id: Some(source_pane_id),
                    ..
                },
                Some("Swap with focused pane"),
            ) => {
                let source_public_id = self.public_pane_id(ws_idx, source_pane_id);
                let target_public_id = self.public_pane_id(ws_idx, pane_id);
                if let (Some(source_public_id), Some(target_public_id)) =
                    (source_public_id, target_public_id)
                {
                    self.runtime_pane_swap(
                        "tui.pane.swap_exact",
                        crate::api::schema::PaneSwapParams {
                            pane_id: None,
                            direction: None,
                            source_pane_id: Some(source_public_id),
                            target_pane_id: Some(target_public_id),
                        },
                    );
                    self.focus_pane_internal_via_api(ws_idx, source_pane_id);
                }
                self.state.mode = Mode::Terminal;
            }
            (
                ContextMenuKind::Pane {
                    ws_idx, pane_id, ..
                },
                Some("Split right"),
            ) => {
                self.focus_pane_internal_via_api(ws_idx, pane_id);
                self.split_focused_pane_via_api(crate::api::schema::SplitDirection::Right);
                self.state.mode = Mode::Terminal;
            }
            (
                ContextMenuKind::Pane {
                    ws_idx, pane_id, ..
                },
                Some("Split down"),
            ) => {
                self.focus_pane_internal_via_api(ws_idx, pane_id);
                self.split_focused_pane_via_api(crate::api::schema::SplitDirection::Down);
                self.state.mode = Mode::Terminal;
            }
            (
                ContextMenuKind::Pane {
                    ws_idx, pane_id, ..
                },
                Some("Zoom"),
            ) => {
                self.focus_pane_internal_via_api(ws_idx, pane_id);
                self.zoom_focused_pane_via_api();
                self.state.mode = Mode::Terminal;
            }
            (
                ContextMenuKind::Pane {
                    ws_idx, pane_id, ..
                },
                Some("Close pane"),
            ) => {
                self.focus_pane_internal_via_api(ws_idx, pane_id);
                if !self.close_focused_pane_via_api_requires_confirmation() {
                    self.state.mode = if self.state.active.is_some() {
                        Mode::Terminal
                    } else {
                        Mode::Navigate
                    };
                }
            }
            _ => leave_modal(&mut self.state),
        }
    }
}

fn cancel_rename_modal(state: &mut AppState) {
    state.creating_new_tab = false;
    state.requested_new_tab_name = None;
    state.pending_workspace_create_cwd = None;
    state.rename_pane_target = None;
    state.name_input.clear();
    state.name_input_replace_on_type = false;
    leave_modal(state);
}

impl AppState {
    pub(super) fn global_menu_item_at(&self, col: u16, row: u16) -> Option<GlobalMenuAction> {
        let rect = self.global_menu_rect();
        if col <= rect.x
            || col >= rect.x + rect.width.saturating_sub(1)
            || row <= rect.y
            || row >= rect.y + rect.height.saturating_sub(1)
        {
            return None;
        }
        let idx = (row - rect.y - 1) as usize;
        global_menu_actions(self).get(idx).copied()
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::layout::Rect;

    use super::super::{capture_snapshot, state_with_workspaces};
    use super::*;
    use crate::workspace::Workspace;

    fn config_env_lock() -> &'static std::sync::Mutex<()> {
        crate::config::test_config_env_lock()
    }

    fn notification_toast(
        title: &str,
        target: Option<crate::app::state::ToastTarget>,
    ) -> crate::app::state::ToastNotification {
        crate::app::state::ToastNotification {
            kind: crate::app::state::ToastKind::Finished,
            title: title.to_string(),
            context: "ctx".to_string(),
            position: None,
            target,
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    #[test]
    fn notification_center_selection_moves_with_arrows_and_j_k() {
        let mut state = state_with_workspaces(&["one"]);
        for title in ["a", "b", "c"] {
            state.post_notification(notification_toast(title, None));
        }
        state.open_notification_center();

        let selected = |state: &AppState| state.notification_center.as_ref().map(|c| c.selected);
        handle_notification_center_key(&mut state, key(KeyCode::Char('j')));
        assert_eq!(selected(&state), Some(1));
        handle_notification_center_key(&mut state, key(KeyCode::Down));
        handle_notification_center_key(&mut state, key(KeyCode::Down));
        assert_eq!(
            selected(&state),
            Some(2),
            "selection clamps at oldest entry"
        );
        handle_notification_center_key(&mut state, key(KeyCode::Char('k')));
        assert_eq!(selected(&state), Some(1));
        handle_notification_center_key(&mut state, key(KeyCode::Up));
        handle_notification_center_key(&mut state, key(KeyCode::Up));
        assert_eq!(
            selected(&state),
            Some(0),
            "selection clamps at newest entry"
        );
    }

    #[test]
    fn notification_center_c_key_clears_log_and_keeps_panel_open() {
        let mut state = state_with_workspaces(&["one"]);
        for title in ["a", "b", "c"] {
            state.post_notification(notification_toast(title, None));
        }
        state.open_notification_center();
        state.notification_center_move_selection(2);

        handle_notification_center_key(&mut state, key(KeyCode::Char('c')));

        assert!(state.notification_log.is_empty(), "log cleared");
        assert_eq!(
            state.mode,
            Mode::NotificationCenter,
            "panel stays open after clear"
        );
        assert_eq!(
            state.notification_center.as_ref().map(|c| c.selected),
            Some(0),
            "selection resets after clear"
        );
    }

    #[test]
    fn notification_center_enter_jumps_to_target_pane_and_closes() {
        let mut state = state_with_workspaces(&["one", "two"]);
        state.mode = Mode::Terminal;
        let target_ws_id = state.workspaces[1].id.clone();
        let target_pane = state.workspaces[1].tabs[0].root_pane;
        state.post_notification(notification_toast(
            "claude finished",
            Some(crate::app::state::ToastTarget {
                workspace_id: target_ws_id,
                pane_id: target_pane,
            }),
        ));
        state.open_notification_center();

        handle_notification_center_key(&mut state, key(KeyCode::Enter));

        assert_eq!(state.active, Some(1), "target workspace focused");
        assert_eq!(state.workspaces[1].focused_pane_id(), Some(target_pane));
        assert_eq!(state.mode, Mode::Terminal);
        assert!(state.notification_center.is_none(), "panel closed");
        assert_eq!(
            state.notification_log.unread_count(),
            0,
            "the activated entry is marked read"
        );
    }

    #[test]
    fn notification_center_enter_marks_only_the_activated_entry_read() {
        let mut state = state_with_workspaces(&["one", "two"]);
        state.mode = Mode::Terminal;
        let target_ws_id = state.workspaces[1].id.clone();
        let target_pane = state.workspaces[1].tabs[0].root_pane;
        state.post_notification(notification_toast("older, stays unread", None));
        state.post_notification(notification_toast(
            "claude finished",
            Some(crate::app::state::ToastTarget {
                workspace_id: target_ws_id,
                pane_id: target_pane,
            }),
        ));
        state.open_notification_center();

        // Selection starts on the newest entry (the targeted one).
        handle_notification_center_key(&mut state, key(KeyCode::Enter));

        assert_eq!(state.notification_log.unread_count(), 1);
        let read_by_title: Vec<(String, bool)> = state
            .notification_log
            .entries_newest_first()
            .map(|entry| (entry.title.clone(), entry.read))
            .collect();
        assert_eq!(
            read_by_title,
            vec![
                ("claude finished".to_string(), true),
                ("older, stays unread".to_string(), false),
            ]
        );
    }

    #[test]
    fn notification_center_r_key_marks_all_read_and_keeps_panel_and_log() {
        let mut state = state_with_workspaces(&["one"]);
        for title in ["a", "b", "c"] {
            state.post_notification(notification_toast(title, None));
        }
        state.open_notification_center();
        assert_eq!(state.notification_log.unread_count(), 3);

        handle_notification_center_key(&mut state, key(KeyCode::Char('r')));

        assert_eq!(state.notification_log.unread_count(), 0);
        assert_eq!(state.notification_log.len(), 3, "history kept");
        assert_eq!(
            state.mode,
            Mode::NotificationCenter,
            "panel stays open after mark-all-read"
        );
    }

    #[test]
    fn notification_center_enter_on_targetless_entry_is_inert() {
        let mut state = state_with_workspaces(&["one"]);
        state.mode = Mode::Terminal;
        state.post_notification(notification_toast("reloaded config", None));
        state.open_notification_center();

        handle_notification_center_key(&mut state, key(KeyCode::Enter));

        assert_eq!(state.mode, Mode::NotificationCenter, "panel stays open");
        assert!(state.notification_center.is_some());
        assert_eq!(state.active, Some(0));
        assert_eq!(
            state.notification_log.unread_count(),
            1,
            "targetless entries stay unread"
        );
    }

    #[test]
    fn notification_center_esc_and_q_close_without_jumping() {
        for close_key in [KeyCode::Esc, KeyCode::Char('q')] {
            let mut state = state_with_workspaces(&["one", "two"]);
            state.mode = Mode::Terminal;
            let target_ws_id = state.workspaces[1].id.clone();
            let target_pane = state.workspaces[1].tabs[0].root_pane;
            state.post_notification(notification_toast(
                "claude finished",
                Some(crate::app::state::ToastTarget {
                    workspace_id: target_ws_id,
                    pane_id: target_pane,
                }),
            ));
            state.open_notification_center();

            handle_notification_center_key(&mut state, key(close_key));

            assert!(state.notification_center.is_none());
            assert_eq!(state.mode, Mode::Terminal);
            assert_eq!(state.active, Some(0), "no jump on close");
        }
    }

    fn temp_config_path(name: &str) -> std::path::PathBuf {
        let unique = format!(
            "herdr-modal-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        std::env::temp_dir().join(unique).join("config.toml")
    }

    fn app_with_test_workspaces(names: &[&str]) -> App {
        let (_api_tx, api_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut app = App::new(
            &crate::config::Config::default(),
            true,
            None,
            api_rx,
            crate::api::EventHub::default(),
        );
        app.state.workspaces = names.iter().map(|name| Workspace::test_new(name)).collect();
        app.state.ensure_test_terminals();
        app.state.active = (!app.state.workspaces.is_empty()).then_some(0);
        app.state.selected = 0;
        app
    }

    #[test]
    fn workspace_create_label_preserves_auto_name_for_suggestion_or_blank() {
        assert_eq!(workspace_create_label("project", "project"), None);
        assert_eq!(workspace_create_label("", "project"), None);
        assert_eq!(workspace_create_label("   ", "project"), None);
        assert_eq!(
            workspace_create_label("  logs  ", "project").as_deref(),
            Some("logs")
        );
    }

    fn mark_worktree_space_member(state: &mut AppState, ws_idx: usize, key: &str) {
        state.workspaces[ws_idx].worktree_space = Some(crate::workspace::WorktreeSpaceMembership {
            key: key.into(),
            label: "herdr".into(),
            repo_root: "/repo/herdr".into(),
            checkout_path: format!("/repo/worktree-{ws_idx}").into(),
            is_linked_worktree: ws_idx != 0,
        });
    }

    #[test]
    fn custom_resize_key_exits_resize_mode() {
        let mut state = state_with_workspaces(&["test"]);
        state.mode = Mode::Resize;
        state.keybinds.resize_mode = crate::config::ActionKeybinds::prefix("g");

        handle_resize_key(
            &mut state,
            TerminalKey::new(KeyCode::Char('g'), KeyModifiers::empty()),
        );

        assert_eq!(state.mode, Mode::Terminal);
    }

    #[test]
    fn direct_resize_key_exits_resize_mode() {
        let mut state = state_with_workspaces(&["test"]);
        state.mode = Mode::Resize;
        state.keybinds.resize_mode = crate::config::ActionKeybinds::direct("ctrl+alt+r");

        handle_resize_key(
            &mut state,
            TerminalKey::new(
                KeyCode::Char('r'),
                KeyModifiers::CONTROL | KeyModifiers::ALT,
            ),
        );

        assert_eq!(state.mode, Mode::Terminal);
    }

    #[test]
    fn resize_key_exit_matches_enhanced_shifted_punctuation() {
        let mut state = state_with_workspaces(&["test"]);
        state.mode = Mode::Resize;
        state.keybinds.resize_mode = crate::config::ActionKeybinds::prefix("?");

        handle_resize_key(
            &mut state,
            TerminalKey::new(KeyCode::Char('/'), KeyModifiers::SHIFT)
                .with_shifted_codepoint('?' as u32),
        );

        assert_eq!(state.mode, Mode::Terminal);
    }

    #[test]
    fn detach_requests_client_detach_in_persistence_mode() {
        let mut state = state_with_workspaces(&["test"]);
        state.detach_exits = false;

        request_detach(&mut state);

        assert!(state.detach_requested);
        assert!(!state.should_quit);
    }

    #[test]
    fn detach_exits_in_no_session_mode() {
        let mut state = state_with_workspaces(&["test"]);
        state.detach_exits = true;

        request_detach(&mut state);

        assert!(state.should_quit);
        assert!(!state.detach_requested);
    }

    #[test]
    fn global_menu_whats_new_opens_saved_release_notes() {
        let _guard = config_env_lock().lock().unwrap();
        let path = temp_config_path("whats-new-saved-release-notes");
        std::env::set_var(crate::config::CONFIG_PATH_ENV_VAR, &path);
        crate::release_notes::save_pending(env!("CARGO_PKG_VERSION"), "### Changed\n- Menu")
            .unwrap();

        let mut state = state_with_workspaces(&["test"]);
        state.latest_release_notes_available = true;

        assert!(global_menu_actions(&state).contains(&GlobalMenuAction::WhatsNew));

        apply_global_menu_action(&mut state, GlobalMenuAction::WhatsNew);

        assert_eq!(state.mode, Mode::ReleaseNotes);
        assert_eq!(
            state
                .release_notes
                .as_ref()
                .map(|notes| notes.body.as_str()),
            Some("### Changed\n- Menu")
        );

        std::env::remove_var(crate::config::CONFIG_PATH_ENV_VAR);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn rename_modal_keyboard_and_mouse_share_actions() {
        let mut state = state_with_workspaces(&["test"]);
        state.mode = Mode::RenameWorkspace;
        state.name_input = "hello".into();

        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        );
        assert!(state.name_input.is_empty());

        state.name_input = "renamed".into();
        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        );
        assert_eq!(state.mode, Mode::Terminal);
        assert_eq!(state.workspaces[0].display_name(), "renamed");
        let snapshot = capture_snapshot(&state);
        assert_eq!(
            snapshot.workspaces[0].custom_name.as_deref(),
            Some("renamed")
        );

        state.view.sidebar_rect = Rect::new(0, 0, 26, 20);
        state.view.terminal_area = Rect::new(26, 0, 80, 20);
        state.mode = Mode::RenameWorkspace;
        state.name_input = "mouse".into();
        let inner = state.rename_modal_inner().unwrap();
        let (save, _, _) = crate::ui::rename_button_rects(inner);
        let action = modal_action_from_buttons(save.x, save.y, &[(save, ModalAction::Save)]);
        assert_eq!(action, Some(ModalAction::Save));
    }

    #[test]
    fn tab_rename_updates_captured_snapshot() {
        let mut state = state_with_workspaces(&["test"]);
        state.mode = Mode::RenameTab;
        state.name_input = "logs".into();

        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        );

        let snapshot = capture_snapshot(&state);
        assert_eq!(
            snapshot.workspaces[0].tabs[0].custom_name.as_deref(),
            Some("logs")
        );
    }

    #[test]
    fn rename_cancel_returns_to_terminal_when_workspace_is_active() {
        let mut state = state_with_workspaces(&["test"]);
        state.mode = Mode::RenameTab;
        state.name_input = "test".into();

        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
        );

        assert_eq!(state.mode, Mode::Terminal);
        assert!(state.name_input.is_empty());
    }

    #[test]
    fn rename_modal_replaces_prefilled_text_on_first_type() {
        let mut state = state_with_workspaces(&["test"]);
        state.mode = Mode::RenameTab;
        state.name_input = "2".into();
        state.name_input_replace_on_type = true;

        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()),
        );
        assert_eq!(state.name_input, "n");
        assert!(!state.name_input_replace_on_type);

        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::empty()),
        );
        assert_eq!(state.name_input, "ne");
    }

    #[test]
    fn rename_modal_replaces_prefilled_text_on_paste() {
        let mut state = state_with_workspaces(&["test"]);
        state.mode = Mode::RenameTab;
        state.name_input = "2".into();
        state.name_input_replace_on_type = true;

        insert_rename_input_text(&mut state, "feature/logs");

        assert_eq!(state.name_input, "feature/logs");
        assert!(!state.name_input_replace_on_type);

        insert_rename_input_text(&mut state, "-copy");

        assert_eq!(state.name_input, "feature/logs-copy");
    }

    #[test]
    fn rename_modal_handles_line_editing_shortcuts() {
        let mut state = state_with_workspaces(&["test"]);
        state.mode = Mode::RenameWorkspace;
        state.name_input = "website zero".into();

        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()),
        );
        assert_eq!(state.name_input, "website zer");

        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::CONTROL),
        );
        assert_eq!(state.name_input, "website ");

        state.name_input = "website-zero".into();
        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::ALT),
        );
        assert_eq!(state.name_input, "website-");

        state.name_input = "website-zero".into();
        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL),
        );
        assert_eq!(state.name_input, "website-");

        state.name_input = "website-zero".into();
        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
        );
        assert_eq!(state.name_input, "website-");

        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::SUPER),
        );
        assert!(state.name_input.is_empty());

        state.name_input = "website zero".into();
        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
        );
        assert!(state.name_input.is_empty());
    }

    #[test]
    fn rename_modal_does_not_insert_modified_shortcut_chars() {
        let mut state = state_with_workspaces(&["test"]);
        state.mode = Mode::RenameWorkspace;
        state.name_input = "website".into();

        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        );
        assert_eq!(state.name_input, "website");

        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('Z'), KeyModifiers::SHIFT),
        );
        assert_eq!(state.name_input, "websiteZ");
    }

    #[test]
    fn keybind_help_slash_focuses_filter_and_preserves_vim_scroll() {
        let mut state = state_with_workspaces(&["test"]);
        state.keybind_help.query = "stale".into();
        state.keybind_help.search_focused = true;
        state.view.terminal_area = Rect::new(0, 0, 100, 30);

        open_keybind_help(&mut state);
        handle_keybind_help_key(
            &mut state,
            TerminalKey::new(KeyCode::Char('j'), KeyModifiers::empty()),
        );
        assert_eq!(state.keybind_help.scroll, 1);
        handle_keybind_help_key(
            &mut state,
            TerminalKey::new(KeyCode::Char('k'), KeyModifiers::empty()),
        );
        assert_eq!(state.keybind_help.scroll, 0);

        handle_keybind_help_key(
            &mut state,
            TerminalKey::new(KeyCode::Char('w'), KeyModifiers::empty()),
        );
        assert!(state.keybind_help.query.is_empty());

        handle_keybind_help_key(
            &mut state,
            TerminalKey::new(KeyCode::Char('/'), KeyModifiers::empty()),
        );
        for character in "work".chars() {
            state.keybind_help.scroll = 2;
            handle_keybind_help_key(
                &mut state,
                TerminalKey::new(KeyCode::Char(character), KeyModifiers::empty()),
            );
        }

        assert!(state.keybind_help.search_focused);
        assert_eq!(state.keybind_help.query, "work");
        assert_eq!(state.keybind_help.scroll, 0);
    }

    #[test]
    fn keybind_help_query_supports_backspace_clear_and_sanitized_paste() {
        let mut state = state_with_workspaces(&["test"]);
        open_keybind_help(&mut state);
        handle_keybind_help_key(
            &mut state,
            TerminalKey::new(KeyCode::Char('/'), KeyModifiers::empty()),
        );

        insert_keybind_help_query_text(&mut state, "work\nspace");
        assert_eq!(state.keybind_help.query, "workspace");

        handle_keybind_help_key(
            &mut state,
            TerminalKey::new(KeyCode::Backspace, KeyModifiers::empty()),
        );
        assert_eq!(state.keybind_help.query, "workspac");

        handle_keybind_help_key(
            &mut state,
            TerminalKey::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
        );
        assert!(state.keybind_help.query.is_empty());
    }

    #[test]
    fn keybind_help_escape_leaves_search_before_closing() {
        let mut state = state_with_workspaces(&["test"]);
        open_keybind_help(&mut state);
        state.keybind_help.search_focused = true;
        state.keybind_help.query = "work".into();

        handle_keybind_help_key(
            &mut state,
            TerminalKey::new(KeyCode::Esc, KeyModifiers::empty()),
        );
        assert_eq!(state.mode, Mode::KeybindHelp);
        assert!(!state.keybind_help.search_focused);
        assert!(state.keybind_help.query.is_empty());

        handle_keybind_help_key(
            &mut state,
            TerminalKey::new(KeyCode::Esc, KeyModifiers::empty()),
        );
        assert_eq!(state.mode, Mode::Terminal);
    }

    #[test]
    fn enhanced_shifted_slash_focuses_keybind_help_filter() {
        let mut state = state_with_workspaces(&["test"]);
        open_keybind_help(&mut state);

        handle_keybind_help_key(
            &mut state,
            TerminalKey::new(KeyCode::Char('7'), KeyModifiers::SHIFT)
                .with_shifted_codepoint('/' as u32),
        );

        assert!(state.keybind_help.search_focused);
    }

    #[test]
    fn enhanced_shifted_question_mark_closes_keybind_help_when_not_searching() {
        let mut state = state_with_workspaces(&["test"]);
        open_keybind_help(&mut state);

        handle_keybind_help_key(
            &mut state,
            TerminalKey::new(KeyCode::Char('/'), KeyModifiers::SHIFT)
                .with_shifted_codepoint('?' as u32),
        );

        assert_eq!(state.mode, Mode::Terminal);

        open_keybind_help(&mut state);
        handle_keybind_help_key(
            &mut state,
            TerminalKey::new(KeyCode::Char('/'), KeyModifiers::empty()),
        );
        handle_keybind_help_key(
            &mut state,
            TerminalKey::new(KeyCode::Char('/'), KeyModifiers::SHIFT)
                .with_shifted_codepoint('?' as u32),
        );

        assert_eq!(state.keybind_help.query, "?");
    }

    #[test]
    fn navigator_search_accepts_pasted_text_when_focused() {
        let mut state = state_with_workspaces(&["alpha", "beta"]);
        let terminal_runtimes = crate::terminal::TerminalRuntimeRegistry::new();
        state.mode = Mode::Navigator;
        state.navigator.search_focused = true;
        state.navigator.state_filter = Some(NavigatorStateFilter::Working);

        insert_navigator_search_text(&mut state, &terminal_runtimes, "beta");

        assert_eq!(state.navigator.query, "beta");
        assert_eq!(state.navigator.state_filter, None);
    }

    #[test]
    fn navigator_search_ignores_paste_when_search_is_not_focused() {
        let mut state = state_with_workspaces(&["alpha", "beta"]);
        let terminal_runtimes = crate::terminal::TerminalRuntimeRegistry::new();
        state.mode = Mode::Navigator;
        state.navigator.search_focused = false;

        insert_navigator_search_text(&mut state, &terminal_runtimes, "beta");

        assert!(state.navigator.query.is_empty());
    }

    #[test]
    fn navigator_empty_search_escape_returns_to_commands() {
        let mut state = state_with_workspaces(&["alpha", "beta"]);
        let terminal_runtimes = crate::terminal::TerminalRuntimeRegistry::new();
        state.mode = Mode::Navigator;
        state.navigator.search_focused = true;

        handle_navigator_key(
            &mut state,
            &terminal_runtimes,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
        );

        assert_eq!(state.mode, Mode::Navigator);
        assert!(!state.navigator.search_focused);
        assert!(state.navigator.query.is_empty());

        handle_navigator_key(
            &mut state,
            &terminal_runtimes,
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::empty()),
        );

        assert_eq!(
            state.navigator.state_filter,
            Some(NavigatorStateFilter::Working)
        );
        assert!(state.navigator.query.is_empty());

        handle_navigator_key(
            &mut state,
            &terminal_runtimes,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
        );

        assert_eq!(state.mode, Mode::Terminal);
    }

    #[test]
    fn navigator_search_escape_blurs_then_next_escape_closes() {
        let mut state = state_with_workspaces(&["alpha", "beta"]);
        let terminal_runtimes = crate::terminal::TerminalRuntimeRegistry::new();
        state.mode = Mode::Navigator;
        state.navigator.search_focused = true;
        state.navigator.query = "a".into();

        handle_navigator_key(
            &mut state,
            &terminal_runtimes,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
        );

        assert_eq!(state.mode, Mode::Navigator);
        assert!(!state.navigator.search_focused);
        assert_eq!(state.navigator.query, "a");

        handle_navigator_key(
            &mut state,
            &terminal_runtimes,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()),
        );

        assert_eq!(state.navigator.selected, 1);
        assert_eq!(state.navigator.query, "a");

        handle_navigator_key(
            &mut state,
            &terminal_runtimes,
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()),
        );

        assert_eq!(state.mode, Mode::Navigator);
        assert!(state.navigator.search_focused);
        assert_eq!(state.navigator.query, "a");

        handle_navigator_key(
            &mut state,
            &terminal_runtimes,
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()),
        );

        assert_eq!(state.navigator.query, "al");

        handle_navigator_key(
            &mut state,
            &terminal_runtimes,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
        );

        assert_eq!(state.mode, Mode::Navigator);
        assert!(!state.navigator.search_focused);

        handle_navigator_key(
            &mut state,
            &terminal_runtimes,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
        );

        assert_eq!(state.mode, Mode::Terminal);
    }

    #[test]
    fn navigator_ignores_modified_j_and_k() {
        let mut state = state_with_workspaces(&["alpha", "beta"]);
        let terminal_runtimes = crate::terminal::TerminalRuntimeRegistry::new();
        state.mode = Mode::Navigator;
        state.navigator.selected = 1;

        handle_navigator_key(
            &mut state,
            &terminal_runtimes,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
        );

        assert_eq!(state.navigator.selected, 1);

        handle_navigator_key(
            &mut state,
            &terminal_runtimes,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
        );

        assert_eq!(state.navigator.selected, 1);
    }

    #[test]
    fn open_rename_active_tab_can_prefill_default_new_tab_name() {
        let mut state = state_with_workspaces(&["test"]);
        state.workspaces[0].test_add_tab(None);
        state.workspaces[0].switch_tab(1);

        open_rename_active_tab(&mut state, true);

        assert_eq!(state.mode, Mode::RenameTab);
        assert_eq!(state.name_input, "2");
        assert!(state.name_input_replace_on_type);
    }

    #[test]
    fn cancel_new_tab_dialog_leaves_workspace_unchanged() {
        let mut state = state_with_workspaces(&["test"]);
        open_new_tab_dialog(&mut state);

        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
        );

        assert_eq!(state.mode, Mode::Terminal);
        assert!(!state.creating_new_tab);
        assert!(!state.request_new_tab);
        assert!(state.requested_new_tab_name.is_none());
        assert_eq!(state.workspaces[0].tabs.len(), 1);
    }

    #[test]
    fn saving_new_tab_dialog_requests_creation_with_name() {
        let mut state = state_with_workspaces(&["test"]);
        open_new_tab_dialog(&mut state);
        state.name_input = "logs".into();
        state.name_input_replace_on_type = false;

        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        );

        assert_eq!(state.mode, Mode::Terminal);
        assert!(!state.creating_new_tab);
        assert!(state.request_new_tab);
        assert_eq!(state.requested_new_tab_name.as_deref(), Some("logs"));
    }

    #[test]
    fn saving_new_tab_dialog_with_default_name_keeps_tab_auto_named() {
        let mut state = state_with_workspaces(&["test"]);
        open_new_tab_dialog(&mut state);

        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        );

        assert_eq!(state.mode, Mode::Terminal);
        assert!(!state.creating_new_tab);
        assert!(state.request_new_tab);
        assert!(state.requested_new_tab_name.is_none());
    }

    #[test]
    fn closing_first_auto_tab_compacts_remaining_auto_tab_label_and_next_prompt() {
        let mut state = state_with_workspaces(&["test"]);
        open_new_tab_dialog(&mut state);
        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        );

        state.workspaces[0].test_add_tab(state.requested_new_tab_name.as_deref());
        state.request_new_tab = false;
        state.requested_new_tab_name = None;

        state.workspaces[0].close_tab(0);
        state.workspaces[0].switch_tab(0);

        assert_eq!(
            state.workspaces[0].tab_display_name(0).as_deref(),
            Some("1")
        );
        assert!(state.workspaces[0].tabs[0].custom_name.is_none());

        open_new_tab_dialog(&mut state);
        assert_eq!(state.name_input, "2");
    }

    #[test]
    fn renaming_auto_tab_to_its_default_number_keeps_it_auto_named() {
        let mut state = state_with_workspaces(&["test"]);
        state.workspaces[0].test_add_tab(None);
        state.workspaces[0].switch_tab(1);

        open_rename_active_tab(&mut state, false);
        handle_rename_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        );

        assert_eq!(state.mode, Mode::Terminal);
        assert!(state.workspaces[0].tabs[1].custom_name.is_none());
        assert_eq!(
            state.workspaces[0].tab_display_name(1).as_deref(),
            Some("2")
        );
    }

    #[test]
    fn confirm_close_keyboard_actions_are_direct_not_focused() {
        let mut state = state_with_workspaces(&["a", "b"]);
        state.selected = 1;
        open_confirm_close(&mut state);

        handle_confirm_close_key(
            &mut state,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
        );
        assert_eq!(state.mode, Mode::Navigate);
        assert_eq!(state.workspaces.len(), 2);

        open_confirm_close(&mut state);
        handle_confirm_close_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        );
        assert_eq!(state.workspaces.len(), 1);
    }

    #[test]
    fn confirm_close_for_linked_worktree_closes_workspace_only() {
        let mut state = state_with_workspaces(&["main", "issue"]);
        state.selected = 1;
        state.workspaces[1].worktree_space = Some(crate::workspace::WorktreeSpaceMembership {
            key: "repo-key".into(),
            label: "herdr".into(),
            repo_root: "/repo/herdr".into(),
            checkout_path: "/repo/herdr-issue".into(),
            is_linked_worktree: true,
        });

        open_confirm_close(&mut state);
        handle_confirm_close_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
        );

        assert_eq!(state.request_remove_linked_worktree, None);
        assert_eq!(state.workspaces.len(), 1);
        assert_eq!(state.workspaces[0].display_name(), "main");
        assert_eq!(state.mode, Mode::Terminal);
    }

    #[test]
    fn context_menu_close_group_opens_group_close_confirmation() {
        let mut state = state_with_workspaces(&["main", "issue"]);
        state.active = Some(0);
        state.selected = 1;
        state.workspaces[0].worktree_space = Some(crate::workspace::WorktreeSpaceMembership {
            key: "repo-key".into(),
            label: "herdr".into(),
            repo_root: "/repo/herdr".into(),
            checkout_path: "/repo/herdr".into(),
            is_linked_worktree: false,
        });
        state.workspaces[1].worktree_space = Some(crate::workspace::WorktreeSpaceMembership {
            key: "repo-key".into(),
            label: "herdr".into(),
            repo_root: "/repo/herdr".into(),
            checkout_path: "/repo/herdr-issue".into(),
            is_linked_worktree: true,
        });
        let menu = ContextMenuState {
            kind: ContextMenuKind::GitWorkspace {
                ws_idx: 0,
                is_linked_worktree: false,
                has_worktree_children: true,
                collapsed: false,
            },
            x: 0,
            y: 0,
            list: MenuListState::new(0),
        };
        let mut terminal_runtimes = crate::terminal::TerminalRuntimeRegistry::new();

        apply_context_menu_action(&mut state, &mut terminal_runtimes, menu, 1);

        assert_eq!(state.selected, 0);
        assert_eq!(state.mode, Mode::ConfirmClose);

        confirm_close_accept(&mut state);

        assert!(state.workspaces.is_empty());
        assert_eq!(state.mode, Mode::Navigate);
    }

    #[test]
    fn context_menu_toggles_pane_right_click_passthrough() {
        let mut app = app_with_test_workspaces(&["main"]);
        app.state.active = Some(0);
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        let menu = ContextMenuState {
            kind: ContextMenuKind::Pane {
                ws_idx: 0,
                tab_idx: 0,
                pane_id,
                source_pane_id: None,
                has_manual_label: false,
                right_click_passthrough: false,
            },
            x: 0,
            y: 0,
            list: MenuListState::new(0),
        };
        let idx = menu
            .items()
            .iter()
            .position(|item| *item == "Send right-clicks to pane")
            .unwrap();
        app.apply_context_menu_action_via_api(menu, idx);

        assert!(
            app.state.workspaces[0]
                .pane_state(pane_id)
                .unwrap()
                .right_click_passthrough
        );
    }

    #[test]
    fn context_menu_close_pane_last_parent_group_pane_keeps_confirmation_mode() {
        let mut state = state_with_workspaces(&["main", "issue"]);
        state.active = Some(0);
        state.selected = 1;
        state.workspaces[0].worktree_space = Some(crate::workspace::WorktreeSpaceMembership {
            key: "repo-key".into(),
            label: "herdr".into(),
            repo_root: "/repo/herdr".into(),
            checkout_path: "/repo/herdr".into(),
            is_linked_worktree: false,
        });
        state.workspaces[1].worktree_space = Some(crate::workspace::WorktreeSpaceMembership {
            key: "repo-key".into(),
            label: "herdr".into(),
            repo_root: "/repo/herdr".into(),
            checkout_path: "/repo/herdr-issue".into(),
            is_linked_worktree: true,
        });
        let pane_id = state.workspaces[0].tabs[0].root_pane;
        let menu = ContextMenuState {
            kind: ContextMenuKind::Pane {
                ws_idx: 0,
                tab_idx: 0,
                pane_id,
                source_pane_id: None,
                has_manual_label: false,
                right_click_passthrough: false,
            },
            x: 0,
            y: 0,
            list: MenuListState::new(0),
        };
        let idx = menu
            .items()
            .iter()
            .position(|item| *item == "Close pane")
            .expect("close pane item");
        let mut terminal_runtimes = crate::terminal::TerminalRuntimeRegistry::new();

        apply_context_menu_action(&mut state, &mut terminal_runtimes, menu, idx);

        assert_eq!(state.selected, 0);
        assert_eq!(state.mode, Mode::ConfirmClose);
        assert_eq!(state.workspaces.len(), 2);
    }

    #[test]
    fn api_confirm_close_accept_closes_parent_worktree_group() {
        let mut app = app_with_test_workspaces(&["main", "issue"]);
        mark_worktree_space_member(&mut app.state, 0, "repo-key");
        mark_worktree_space_member(&mut app.state, 1, "repo-key");
        app.state.selected = 0;
        open_confirm_close(&mut app.state);

        app.handle_confirm_close_key_via_api(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

        assert!(app.state.workspaces.is_empty());
        assert_eq!(app.state.mode, Mode::Navigate);
        assert_eq!(app.event_hub.events_after(0).len(), 2);
    }

    #[test]
    fn api_confirm_close_accept_keeps_the_original_workspace_target() {
        let mut app = app_with_test_workspaces(&["main", "issue", "other"]);
        mark_worktree_space_member(&mut app.state, 0, "repo-key");
        mark_worktree_space_member(&mut app.state, 1, "repo-key");
        app.state.selected = 0;
        open_confirm_close(&mut app.state);

        app.focus_workspace_idx_via_api(2);
        assert_eq!(app.state.selected, 2);
        assert_eq!(app.state.mode, Mode::ConfirmClose);

        app.handle_confirm_close_key_via_api(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

        assert_eq!(app.state.workspaces.len(), 1);
        assert_eq!(app.state.workspaces[0].display_name(), "other");
        assert_eq!(
            app.event_hub
                .events_after(0)
                .iter()
                .filter(|(_, event)| matches!(
                    event.event,
                    crate::api::schema::EventKind::WorkspaceClosed
                ))
                .count(),
            2
        );
    }

    #[test]
    fn api_context_menu_close_tab_last_parent_group_workspace_keeps_confirmation_mode() {
        let mut app = app_with_test_workspaces(&["main", "issue"]);
        mark_worktree_space_member(&mut app.state, 0, "repo-key");
        mark_worktree_space_member(&mut app.state, 1, "repo-key");
        app.state.active = Some(0);
        app.state.selected = 1;
        app.state.mode = Mode::ContextMenu;
        let menu = ContextMenuState {
            kind: ContextMenuKind::Tab {
                ws_idx: 0,
                tab_idx: 0,
            },
            x: 0,
            y: 0,
            list: MenuListState::new(0),
        };
        let idx = menu
            .items()
            .iter()
            .position(|item| *item == "Close")
            .expect("close tab item");

        app.apply_context_menu_action_via_api(menu, idx);

        assert_eq!(app.state.selected, 0);
        assert_eq!(app.state.mode, Mode::ConfirmClose);
        assert_eq!(app.state.workspaces.len(), 2);
    }

    #[test]
    fn api_context_menu_enter_close_pane_last_parent_group_pane_keeps_confirmation_mode() {
        let mut app = app_with_test_workspaces(&["main", "issue"]);
        mark_worktree_space_member(&mut app.state, 0, "repo-key");
        mark_worktree_space_member(&mut app.state, 1, "repo-key");
        app.state.active = Some(0);
        app.state.selected = 1;
        app.state.mode = Mode::ContextMenu;
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        let mut menu = ContextMenuState {
            kind: ContextMenuKind::Pane {
                ws_idx: 0,
                tab_idx: 0,
                pane_id,
                source_pane_id: None,
                has_manual_label: false,
                right_click_passthrough: false,
            },
            x: 0,
            y: 0,
            list: MenuListState::new(0),
        };
        let close_idx = menu
            .items()
            .iter()
            .position(|item| *item == "Close pane")
            .expect("close pane item");
        menu.list.highlighted = close_idx;
        app.state.context_menu = Some(menu);

        app.handle_context_menu_key_via_api(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

        assert_eq!(app.state.selected, 0);
        assert_eq!(app.state.mode, Mode::ConfirmClose);
        assert_eq!(app.state.workspaces.len(), 2);
        assert!(app.state.context_menu.is_none());
    }

    /// App with one workspace, one pane, and todos on it. Builds on the
    /// module's existing `app_with_test_workspaces`, which already wires
    /// `App::new`, `ensure_test_terminals`, and `active`.
    fn app_with_pane_todos(todos: &[(&str, bool, crate::terminal::todo::TodoPriority)]) -> App {
        let mut app = app_with_test_workspaces(&["todos"]);
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.state.workspaces[0].tabs[0].panes[&pane_id]
            .attached_terminal_id
            .clone();
        let terminal = app
            .state
            .terminals
            .get_mut(&terminal_id)
            .expect("test terminal should exist");
        for (text, done, priority) in todos {
            let todo = terminal
                .add_todo(text, *priority, None, 100)
                .expect("todo should be added");
            if *done {
                terminal
                    .update_todo(
                        todo.id,
                        crate::terminal::todo::TodoUpdate {
                            done: Some(true),
                            ..Default::default()
                        },
                        200,
                    )
                    .expect("todo should be updated");
            }
        }
        app.state.open_pane_todos(pane_id);
        app
    }

    fn panel_todo_texts(app: &App) -> Vec<String> {
        let pane_id = app
            .state
            .pane_todos
            .as_ref()
            .expect("panel should be open")
            .pane_id;
        app.state
            .pane_todos_in_display_order(pane_id)
            .into_iter()
            .map(|todo| todo.text.clone())
            .collect()
    }

    #[test]
    fn pane_todo_panel_selection_moves_with_arrows_and_j_k() {
        let mut app = app_with_pane_todos(&[
            ("first", false, crate::terminal::todo::TodoPriority::High),
            ("second", false, crate::terminal::todo::TodoPriority::Normal),
        ]);

        app.handle_pane_todos_key_via_api(key(KeyCode::Down));
        assert_eq!(app.state.pane_todos.as_ref().expect("panel").selected, 1);
        app.handle_pane_todos_key_via_api(key(KeyCode::Char('k')));
        assert_eq!(app.state.pane_todos.as_ref().expect("panel").selected, 0);
        app.handle_pane_todos_key_via_api(key(KeyCode::Char('j')));
        assert_eq!(app.state.pane_todos.as_ref().expect("panel").selected, 1);
    }

    #[test]
    fn space_toggles_the_selected_todo_through_the_api() {
        let mut app = app_with_pane_todos(&[(
            "toggle me",
            false,
            crate::terminal::todo::TodoPriority::Normal,
        )]);
        app.state.session_dirty = false;

        app.handle_pane_todos_key_via_api(key(KeyCode::Char(' ')));

        let todo = app
            .state
            .selected_pane_todo()
            .expect("a todo should still be selected");
        assert!(todo.done, "space marks the selected todo done");
        assert!(
            app.state.session_dirty,
            "the mutation went through the API, so the session is dirty"
        );

        app.handle_pane_todos_key_via_api(key(KeyCode::Char(' ')));
        assert!(
            !app.state
                .selected_pane_todo()
                .expect("a todo should still be selected")
                .done,
            "space toggles back"
        );
    }

    #[test]
    fn d_removes_and_c_clears_only_done_todos() {
        let mut app = app_with_pane_todos(&[
            (
                "keep me",
                false,
                crate::terminal::todo::TodoPriority::Normal,
            ),
            (
                "finished",
                true,
                crate::terminal::todo::TodoPriority::Normal,
            ),
        ]);

        app.handle_pane_todos_key_via_api(key(KeyCode::Char('c')));
        assert_eq!(panel_todo_texts(&app), vec!["keep me".to_string()]);

        app.handle_pane_todos_key_via_api(key(KeyCode::Char('d')));
        assert!(panel_todo_texts(&app).is_empty());
        assert_eq!(
            app.state.pane_todos.as_ref().expect("panel").selected,
            0,
            "the selection re-clamps once the list shrinks"
        );
    }

    /// Spec: "Following a link → focus moves to the linked pane" via the same
    /// focus path a notification jump uses.
    #[test]
    fn g_follows_a_live_link_and_closes_the_panel() {
        let mut app = app_with_pane_todos(&[(
            "look over there",
            false,
            crate::terminal::todo::TodoPriority::Normal,
        )]);
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        // The link has to point somewhere else: linking a todo to its own pane
        // cannot tell "focus moved" apart from "focus never changed". Split,
        // then focus back to the pane holding the todo.
        let sibling = app.state.workspaces[0].test_split(ratatui::layout::Direction::Vertical);
        app.state.ensure_test_terminals();
        app.state.workspaces[0].tabs[0].layout.focus_pane(pane_id);
        assert_eq!(
            app.state.workspaces[0].focused_pane_id(),
            Some(pane_id),
            "the jump has to start away from the linked pane"
        );
        let terminal_id = app.state.workspaces[0].tabs[0].panes[&pane_id]
            .attached_terminal_id
            .clone();
        let todo_id = app.state.terminals[&terminal_id].todos()[0].id;
        app.state
            .terminals
            .get_mut(&terminal_id)
            .expect("test terminal should exist")
            .update_todo(
                todo_id,
                crate::terminal::todo::TodoUpdate {
                    link: Some(Some(crate::terminal::todo::TodoLink {
                        pane: Some(sibling),
                        label: "infra".into(),
                    })),
                    ..Default::default()
                },
                300,
            )
            .expect("todo should be updated");

        app.handle_pane_todos_key_via_api(key(KeyCode::Char('g')));

        assert_eq!(
            app.state.workspaces[0].focused_pane_id(),
            Some(sibling),
            "following the link focuses the linked pane"
        );
        assert!(app.state.pane_todos.is_none(), "the panel closes on a jump");
        assert_eq!(app.state.mode, Mode::Terminal);
    }

    #[test]
    fn g_on_a_dead_link_is_inert() {
        let mut app =
            app_with_pane_todos(&[("gone", false, crate::terminal::todo::TodoPriority::Normal)]);
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.state.workspaces[0].tabs[0].panes[&pane_id]
            .attached_terminal_id
            .clone();
        let todo_id = app.state.terminals[&terminal_id].todos()[0].id;
        app.state
            .terminals
            .get_mut(&terminal_id)
            .expect("test terminal should exist")
            .update_todo(
                todo_id,
                crate::terminal::todo::TodoUpdate {
                    link: Some(Some(crate::terminal::todo::TodoLink {
                        pane: None,
                        label: "infra".into(),
                    })),
                    ..Default::default()
                },
                300,
            )
            .expect("todo should be updated");

        app.handle_pane_todos_key_via_api(key(KeyCode::Char('g')));

        assert!(
            app.state.pane_todos.is_some(),
            "a dead link changes nothing at all"
        );
        assert_eq!(app.state.mode, Mode::PaneTodos);
    }

    #[test]
    fn esc_and_q_close_the_panel() {
        for code in [KeyCode::Esc, KeyCode::Char('q')] {
            let mut app = app_with_pane_todos(&[(
                "still here",
                false,
                crate::terminal::todo::TodoPriority::Normal,
            )]);
            app.handle_pane_todos_key_via_api(key(code));
            assert!(app.state.pane_todos.is_none());
            assert_ne!(app.state.mode, Mode::PaneTodos);
        }
    }

    trait WithControl {
        fn with_control(self) -> KeyEvent;
    }

    impl WithControl for KeyEvent {
        fn with_control(mut self) -> KeyEvent {
            self.modifiers |= KeyModifiers::CONTROL;
            self
        }
    }

    /// Spec: "its text changed and the change saved → the todo's text and
    /// updated timestamp change while its id, done state, and creation
    /// timestamp are preserved".
    #[test]
    fn saving_an_edit_changes_text_and_keeps_id_done_and_created_at() {
        let mut app =
            app_with_pane_todos(&[("draft", true, crate::terminal::todo::TodoPriority::Normal)]);
        let before = app.state.selected_pane_todo().expect("a selected todo");

        app.handle_pane_todos_key_via_api(key(KeyCode::Enter));
        assert_eq!(app.state.mode, Mode::PaneTodoEdit);
        assert_eq!(
            app.state.pane_todo_edit.as_ref().expect("edit state").text,
            "draft",
            "the modal opens prefilled"
        );

        for _ in 0..5 {
            app.handle_pane_todo_edit_key_via_api(key(KeyCode::Backspace));
        }
        for ch in "final".chars() {
            app.handle_pane_todo_edit_key_via_api(key(KeyCode::Char(ch)));
        }
        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Enter));

        let after = app.state.selected_pane_todo().expect("a selected todo");
        assert_eq!(after.text, "final");
        assert_eq!(after.id, before.id, "id is preserved");
        assert!(after.done, "done state is untouched by a text edit");
        assert_eq!(
            after.created_at_unix, before.created_at_unix,
            "created_at is preserved"
        );
        assert!(after.updated_at_unix >= before.updated_at_unix);
        assert!(app.state.pane_todo_edit.is_none());
        assert_eq!(
            app.state.mode,
            Mode::PaneTodos,
            "saving returns to the panel it was opened from"
        );
    }

    #[test]
    fn tab_cycles_priority_and_cancel_discards_the_buffer() {
        let mut app = app_with_pane_todos(&[(
            "keep me",
            false,
            crate::terminal::todo::TodoPriority::Normal,
        )]);
        app.handle_pane_todos_key_via_api(key(KeyCode::Enter));

        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Tab));
        assert_eq!(
            app.state
                .pane_todo_edit
                .as_ref()
                .expect("edit state")
                .priority,
            crate::terminal::todo::TodoPriority::High
        );
        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Char('!')));
        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Esc));

        let todo = app.state.selected_pane_todo().expect("a selected todo");
        assert_eq!(todo.text, "keep me", "cancel writes nothing");
        assert_eq!(todo.priority, crate::terminal::todo::TodoPriority::Normal);
        assert_eq!(app.state.mode, Mode::PaneTodos);
    }

    /// The panel toggles done with `space`, but inside the modal `space` is
    /// text, so the same action needs `ctrl+d`.
    #[test]
    fn ctrl_d_toggles_done_in_the_edit_modal_and_saves_it() {
        let mut app = app_with_pane_todos(&[(
            "finish me",
            false,
            crate::terminal::todo::TodoPriority::Normal,
        )]);
        app.handle_pane_todos_key_via_api(key(KeyCode::Enter));
        assert!(
            !app.state.pane_todo_edit.as_ref().expect("edit state").done,
            "the modal opens carrying the todo's current done state"
        );

        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Char('d')).with_control());
        assert!(app.state.pane_todo_edit.as_ref().expect("edit state").done);
        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Enter));

        let todo = app
            .state
            .selected_pane_todo()
            .expect("the todo should survive the save");
        assert!(
            todo.done,
            "the toggle reaches the store through todo.update"
        );
        assert_eq!(todo.text, "finish me", "text is untouched by the toggle");

        // And back again, so this is a toggle rather than a one-way latch.
        app.handle_pane_todos_key_via_api(key(KeyCode::Enter));
        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Char('d')).with_control());
        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Enter));
        assert!(
            !app.state
                .selected_pane_todo()
                .expect("a selected todo")
                .done
        );
    }

    #[test]
    fn cancelling_discards_a_done_toggle() {
        let mut app = app_with_pane_todos(&[(
            "leave me alone",
            false,
            crate::terminal::todo::TodoPriority::Normal,
        )]);
        app.handle_pane_todos_key_via_api(key(KeyCode::Enter));
        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Char('d')).with_control());
        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Esc));

        assert!(
            !app.state
                .selected_pane_todo()
                .expect("a selected todo")
                .done,
            "cancel writes nothing, including the done toggle"
        );
    }

    /// `todo.add` carries no `done`, so offering the toggle while composing
    /// would promise something the save cannot keep.
    #[test]
    fn ctrl_d_is_inert_while_composing_a_new_todo() {
        let mut app = app_with_pane_todos(&[(
            "existing",
            false,
            crate::terminal::todo::TodoPriority::Normal,
        )]);
        let pane_id = app
            .state
            .pane_todos
            .as_ref()
            .expect("panel should be open")
            .pane_id;
        app.state.open_new_pane_todo(pane_id);

        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Char('d')).with_control());

        assert!(
            !app.state.pane_todo_edit.as_ref().expect("edit state").done,
            "composing a new todo cannot mark it done"
        );
    }

    #[test]
    fn an_empty_buffer_never_saves() {
        let mut app = app_with_pane_todos(&[(
            "keep me",
            false,
            crate::terminal::todo::TodoPriority::Normal,
        )]);
        app.handle_pane_todos_key_via_api(key(KeyCode::Enter));
        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Char('u')).with_control());

        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Enter));

        assert_eq!(
            app.state.mode,
            Mode::PaneTodoEdit,
            "the store rejects empty text, so the modal stays open instead of dropping the edit"
        );
        assert_eq!(
            app.state.selected_pane_todo().expect("a todo").text,
            "keep me"
        );
    }

    #[test]
    fn the_add_action_creates_a_todo_on_the_focused_pane() {
        let mut app = app_with_pane_todos(&[]);
        app.state.close_pane_todos();
        app.state.mode = Mode::Terminal;
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;

        app.execute_tui_navigate_action(
            super::super::navigate::NavigateAction::AddPaneTodo,
            super::super::navigate::ActionContext::Prefix,
        );
        assert_eq!(
            app.state.mode,
            Mode::PaneTodoEdit,
            "the action opens the modal itself, without going through the panel"
        );
        assert_eq!(
            app.state
                .pane_todo_edit
                .as_ref()
                .expect("edit state")
                .pane_id,
            pane_id
        );

        for ch in "write it down".chars() {
            app.handle_pane_todo_edit_key_via_api(key(KeyCode::Char(ch)));
        }
        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Enter));

        let todos = app.state.pane_todos_in_display_order(pane_id);
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].text, "write it down");
        assert!(
            app.state.pane_todos.is_none(),
            "opened without a panel, it returns to the terminal rather than opening one"
        );
    }

    #[test]
    fn a_save_the_store_rejects_keeps_the_modal_open_with_the_typed_text() {
        let full = vec![
            ("filler", false, crate::terminal::todo::TodoPriority::Normal);
            crate::terminal::todo::MAX_TODOS_PER_PANE
        ];
        let mut app = app_with_pane_todos(&full);
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;

        app.execute_tui_navigate_action(
            super::super::navigate::NavigateAction::AddPaneTodo,
            super::super::navigate::ActionContext::Prefix,
        );
        assert_eq!(app.state.mode, Mode::PaneTodoEdit);
        for ch in "one too many".chars() {
            app.handle_pane_todo_edit_key_via_api(key(KeyCode::Char(ch)));
        }
        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Enter));

        assert_eq!(
            app.state.mode,
            Mode::PaneTodoEdit,
            "the pane is at the todo cap, so the save is refused and the modal stays open"
        );
        assert_eq!(
            app.state.pane_todo_edit.as_ref().expect("edit state").text,
            "one too many",
            "a refused save must not eat what was typed"
        );
        assert_eq!(
            app.state.pane_todos_in_display_order(pane_id).len(),
            crate::terminal::todo::MAX_TODOS_PER_PANE
        );
        let toast = app.state.toast.as_ref().expect("rejection feedback");
        assert_eq!(toast.title, "todo save failed");
        assert!(
            toast.context.contains("maximum"),
            "the store's reason is surfaced verbatim: {}",
            toast.context
        );
    }

    /// Builds a workspace with three panes so the todo's own pane has two
    /// linkable siblings, and puts one todo on the root pane.
    fn app_with_linkable_panes() -> (App, crate::layout::PaneId, Vec<crate::layout::PaneId>) {
        let mut app = app_with_test_workspaces(&["links"]);
        app.state.workspaces[0].test_split(ratatui::layout::Direction::Horizontal);
        app.state.workspaces[0].test_split(ratatui::layout::Direction::Vertical);
        app.state.ensure_test_terminals();
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.state.workspaces[0].tabs[0].panes[&pane_id]
            .attached_terminal_id
            .clone();
        app.state
            .terminals
            .get_mut(&terminal_id)
            .expect("test terminal should exist")
            .add_todo(
                "rerun the deploy",
                crate::terminal::todo::TodoPriority::Normal,
                None,
                100,
            )
            .expect("todo should be added");
        let candidates: Vec<crate::layout::PaneId> = app.state.workspaces[0].tabs[0]
            .layout
            .pane_ids()
            .into_iter()
            .filter(|candidate| *candidate != pane_id)
            .collect();
        assert_eq!(candidates.len(), 2, "two sibling panes should be linkable");
        (app, pane_id, candidates)
    }

    /// Choose a link target the way a user does: ctrl+l opens the picker,
    /// then a pane row is selected and accepted.
    fn choose_link_target(app: &mut App, target: crate::layout::PaneId) {
        app.handle_pane_todo_edit_key_via_api(KeyEvent::new(
            KeyCode::Char('l'),
            KeyModifiers::CONTROL,
        ));
        app.state.navigator.selected = app
            .state
            .navigator_rows_from(&app.terminal_runtimes)
            .iter()
            .position(|row| {
                matches!(
                    row.target,
                    crate::app::state::NavigatorTarget::Pane { pane_id, .. } if pane_id == target
                )
            })
            .expect("the target pane should be offered");
        app.state
            .accept_navigator_selection_from(&app.terminal_runtimes);
    }

    /// Clear the link through the picker's explicit entry.
    fn choose_clear_link(app: &mut App) {
        app.handle_pane_todo_edit_key_via_api(KeyEvent::new(
            KeyCode::Char('l'),
            KeyModifiers::CONTROL,
        ));
        app.state.navigator.selected = app
            .state
            .navigator_rows_from(&app.terminal_runtimes)
            .iter()
            .position(|row| matches!(row.target, crate::app::state::NavigatorTarget::ClearLink))
            .expect("the clear entry should be offered");
        app.state
            .accept_navigator_selection_from(&app.terminal_runtimes);
    }

    fn stored_todo(app: &App, pane_id: crate::layout::PaneId) -> crate::terminal::todo::PaneTodo {
        app.state
            .pane_terminal(pane_id)
            .expect("pane terminal")
            .todos()
            .first()
            .expect("one todo")
            .clone()
    }

    /// The tri-state link control is only worth having if the choice survives
    /// the save. Without this, `PaneTodoEditLink::Set(_) => public_pane_id(..)`
    /// in `save_pane_todo_edit_via_api` could return `None` and every test
    /// would still pass.
    #[test]
    fn a_link_chosen_in_the_edit_modal_reaches_the_store() {
        let (mut app, pane_id, candidates) = app_with_linkable_panes();
        let todo_id = stored_todo(&app, pane_id).id;
        app.state.open_pane_todo_edit(pane_id, todo_id);

        // ctrl+l opens the picker; choose the first sibling pane from it.
        app.handle_pane_todo_edit_key_via_api(KeyEvent::new(
            KeyCode::Char('l'),
            KeyModifiers::CONTROL,
        ));
        assert_eq!(app.state.mode, Mode::Navigator);
        app.state.navigator.selected = app
            .state
            .navigator_rows_from(&app.terminal_runtimes)
            .iter()
            .position(|row| {
                matches!(
                    row.target,
                    crate::app::state::NavigatorTarget::Pane { pane_id: candidate, .. }
                        if candidate == candidates[0]
                )
            })
            .expect("the sibling pane should be offered");
        app.state
            .accept_navigator_selection_from(&app.terminal_runtimes);
        assert_eq!(
            app.state.pane_todo_edit.as_ref().expect("edit state").link,
            crate::app::state::PaneTodoEditLink::Set(candidates[0]),
        );

        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Enter));

        let saved = stored_todo(&app, pane_id);
        let link = saved.link.expect("the chosen link must reach the store");
        assert_eq!(
            link.pane,
            Some(candidates[0]),
            "the modal's link target must be the pane that was selected"
        );
        // The label is resolved server-side from the target pane's label or
        // agent name, falling back to its public id; these test panes have
        // neither, so the public id is what gets captured.
        let expected_label = app
            .public_pane_id(0, candidates[0])
            .expect("target pane has a public id");
        assert_eq!(
            link.label, expected_label,
            "the link captures a label identifying its target"
        );
        assert!(
            app.state.pane_todo_edit.is_none(),
            "saving closes the modal"
        );
    }

    /// Spec: "adding a todo from the panel". The panel's keys were selection,
    /// toggle, edit, follow, remove, clear and close — no add — and
    /// `keys.add_pane_todo` ships unbound, so out of the box there was no
    /// keyboard route to creating a todo at all.
    #[test]
    fn a_adds_a_todo_from_the_panel_and_saving_returns_to_it() {
        let (mut app, pane_id, _) = app_with_linkable_panes();
        app.state.open_pane_todos(pane_id);

        app.handle_pane_todos_key_via_api(key(KeyCode::Char('a')));

        assert_eq!(app.state.mode, Mode::PaneTodoEdit);
        let edit = app.state.pane_todo_edit.as_ref().expect("edit state");
        assert_eq!(edit.pane_id, pane_id);
        assert!(edit.todo_id.is_none(), "a new todo, not an existing one");

        for ch in "water the plants".chars() {
            app.handle_pane_todo_edit_key_via_api(key(KeyCode::Char(ch)));
        }
        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Enter));

        assert_eq!(
            app.state.mode,
            Mode::PaneTodos,
            "saving returns to the panel it was opened from"
        );
        assert!(app
            .state
            .pane_terminal(pane_id)
            .expect("pane terminal")
            .todos()
            .iter()
            .any(|todo| todo.text == "water the plants"));
    }

    /// The empty panel is the case that matters: every pane now carries an
    /// indicator that opens one, so it has to offer a way forward.
    #[test]
    fn the_panel_can_add_on_a_pane_with_no_todos() {
        let mut app = app_with_test_workspaces(&["empty"]);
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        app.state.open_pane_todos(pane_id);
        assert!(app
            .state
            .pane_terminal(pane_id)
            .expect("pane terminal")
            .todos()
            .is_empty());

        app.handle_pane_todos_key_via_api(key(KeyCode::Char('a')));

        assert_eq!(app.state.mode, Mode::PaneTodoEdit);
        assert_eq!(
            app.state
                .pane_todo_edit
                .as_ref()
                .expect("edit state")
                .pane_id,
            pane_id
        );
    }

    /// Pane ids are unique across the session but public pane ids are scoped
    /// to a workspace, so `save_pane_todo_edit_via_api` resolving the target
    /// against `state.active` dropped any cross-workspace link on save — no
    /// error, no toast, the link simply was not there afterwards.
    #[test]
    fn a_link_to_a_pane_in_another_workspace_reaches_the_store() {
        let mut app = app_with_test_workspaces(&["here", "there"]);
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        let target = app.state.workspaces[1].tabs[0].root_pane;
        assert_ne!(pane_id, target);
        assert!(
            app.public_pane_id(0, target).is_none(),
            "the target must be unresolvable in the active workspace, \
             otherwise this test proves nothing"
        );

        let terminal_id = app.state.workspaces[0].tabs[0].panes[&pane_id]
            .attached_terminal_id
            .clone();
        app.state
            .terminals
            .get_mut(&terminal_id)
            .expect("test terminal should exist")
            .add_todo(
                "ship it",
                crate::terminal::todo::TodoPriority::Normal,
                None,
                100,
            )
            .expect("todo should be added");

        let todo_id = stored_todo(&app, pane_id).id;
        app.state.open_pane_todo_edit(pane_id, todo_id);
        app.state.pane_todo_edit.as_mut().expect("edit state").link =
            crate::app::state::PaneTodoEditLink::Set(target);

        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Enter));

        let link = stored_todo(&app, pane_id)
            .link
            .expect("a cross-workspace link must survive the save");
        assert_eq!(
            link.pane,
            Some(target),
            "the stored link must point at the pane that was chosen"
        );
    }

    /// The Clear arm maps to `clear_link: true`; without this, hardcoding
    /// `clear_link = false` leaves the suite green.
    #[test]
    fn clearing_the_link_in_the_edit_modal_removes_it_from_the_store() {
        let (mut app, pane_id, candidates) = app_with_linkable_panes();
        let todo_id = stored_todo(&app, pane_id).id;

        // Give the todo a link first, through the same save path.
        app.state.open_pane_todo_edit(pane_id, todo_id);
        choose_link_target(&mut app, candidates[0]);
        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Enter));
        assert_eq!(
            stored_todo(&app, pane_id)
                .link
                .expect("link should be set")
                .pane,
            Some(candidates[0]),
        );

        // Reopen and pick the clear entry, then save.
        app.state.open_pane_todo_edit(pane_id, todo_id);
        choose_clear_link(&mut app);
        assert_eq!(
            app.state.pane_todo_edit.as_ref().expect("edit state").link,
            crate::app::state::PaneTodoEditLink::Clear,
        );
        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Enter));

        assert_eq!(
            stored_todo(&app, pane_id).link,
            None,
            "choosing Clear must remove the link from the store"
        );
    }

    /// `Keep` must leave an existing link untouched, which is the arm the other
    /// two tests cannot distinguish from a no-op save.
    #[test]
    fn keeping_the_link_in_the_edit_modal_leaves_it_alone() {
        let (mut app, pane_id, candidates) = app_with_linkable_panes();
        let todo_id = stored_todo(&app, pane_id).id;
        app.state.open_pane_todo_edit(pane_id, todo_id);
        choose_link_target(&mut app, candidates[0]);
        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Enter));

        // Reopen, change only the text, save with the link left on Keep.
        app.state.open_pane_todo_edit(pane_id, todo_id);
        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Char('!')));
        assert_eq!(
            app.state.pane_todo_edit.as_ref().expect("edit state").link,
            crate::app::state::PaneTodoEditLink::Keep,
        );
        app.handle_pane_todo_edit_key_via_api(key(KeyCode::Enter));

        let saved = stored_todo(&app, pane_id);
        assert_eq!(
            saved.link.expect("Keep must not drop the link").pane,
            Some(candidates[0]),
        );
        assert!(
            saved.text.ends_with('!'),
            "the text edit still saved: {}",
            saved.text
        );
    }

    fn close_pane_via_api(app: &mut App, pane_id: crate::layout::PaneId) -> serde_json::Value {
        let public_pane_id = app
            .public_pane_id(0, pane_id)
            .expect("pane should have a public id");
        let raw = app.handle_api_request(crate::api::schema::Request {
            id: "test".into(),
            method: crate::api::schema::Method::PaneClose(crate::api::schema::PaneTarget {
                pane_id: public_pane_id,
            }),
        });
        serde_json::from_str(&raw).expect("response should be json")
    }

    /// Spec: "a pane with at least one not-done todo is closed -> a
    /// confirmation is requested before the pane is destroyed".
    #[test]
    fn closing_a_pane_with_outstanding_todos_asks_first() {
        let mut app = app_with_pane_todos(&[(
            "unfinished",
            false,
            crate::terminal::todo::TodoPriority::Normal,
        )]);
        app.state.close_pane_todos();
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;

        let response = close_pane_via_api(&mut app, pane_id);

        assert_eq!(response["error"]["code"], "confirmation_required");
        assert_eq!(app.state.mode, Mode::ConfirmClose);
        assert_eq!(app.state.confirm_close_pane, Some(pane_id));
        assert!(
            !app.state.workspaces.is_empty(),
            "nothing is destroyed before the answer"
        );
    }

    #[test]
    fn accepting_the_confirmation_closes_the_pane() {
        let mut app = app_with_pane_todos(&[(
            "unfinished",
            false,
            crate::terminal::todo::TodoPriority::Normal,
        )]);
        app.state.close_pane_todos();
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        close_pane_via_api(&mut app, pane_id);

        app.confirm_close_accept_via_api();

        assert!(
            app.state.confirm_close_pane.is_none(),
            "the pending token is consumed, so the retry goes through"
        );
        assert!(
            app.state.workspaces.is_empty(),
            "the last pane closing takes its workspace with it"
        );
    }

    #[test]
    fn cancelling_the_confirmation_keeps_the_pane_and_drops_the_token() {
        let mut app = app_with_pane_todos(&[(
            "unfinished",
            false,
            crate::terminal::todo::TodoPriority::Normal,
        )]);
        app.state.close_pane_todos();
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;
        close_pane_via_api(&mut app, pane_id);

        confirm_close_cancel(&mut app.state);

        assert!(app.state.confirm_close_pane.is_none());
        assert!(!app.state.workspaces.is_empty());
    }

    /// Spec: "every todo on the pane is done -> the pane closes without
    /// additional confirmation".
    #[test]
    fn a_pane_whose_todos_are_all_done_closes_without_a_prompt() {
        let mut app = app_with_pane_todos(&[(
            "finished",
            true,
            crate::terminal::todo::TodoPriority::Normal,
        )]);
        app.state.close_pane_todos();
        let pane_id = app.state.workspaces[0].tabs[0].root_pane;

        let response = close_pane_via_api(&mut app, pane_id);

        assert!(response["result"].is_object(), "no prompt: {response:?}");
        assert!(app.state.confirm_close_pane.is_none());
    }
}
