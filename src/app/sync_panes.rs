//! Synchronized input for a tab (fork issue 141): the app-level switches over
//! `Tab::sync`. The state and its set arithmetic live on `Tab`; this is the
//! part that knows which workspace and tab are on screen.

use super::state::AppState;
use crate::layout::PaneId;

impl AppState {
    /// Turn sync on or off for the tab on screen. `true` when it is on now,
    /// `false` when it went off or there is no tab.
    pub(crate) fn toggle_sync_panes(&mut self) -> bool {
        let Some(tab) = self
            .active
            .and_then(|ws_idx| self.workspaces.get_mut(ws_idx))
            .and_then(crate::workspace::Workspace::active_tab_mut)
        else {
            return false;
        };
        tab.set_sync(!tab.is_syncing());
        tab.is_syncing()
    }

    /// Set sync for one tab. `false` when the tab does not exist.
    pub(crate) fn set_tab_sync(&mut self, ws_idx: usize, tab_idx: usize, on: bool) -> bool {
        let Some(tab) = self
            .workspaces
            .get_mut(ws_idx)
            .and_then(|ws| ws.tabs.get_mut(tab_idx))
        else {
            return false;
        };
        tab.set_sync(on);
        true
    }

    /// Take a pane out of its tab's synced set or put it back. `None` when its
    /// tab does not sync.
    pub(crate) fn toggle_pane_sync(&mut self, ws_idx: usize, pane_id: PaneId) -> Option<bool> {
        let tab_idx = self
            .workspaces
            .get(ws_idx)?
            .find_tab_index_for_pane(pane_id)?;
        self.workspaces
            .get_mut(ws_idx)?
            .tabs
            .get_mut(tab_idx)?
            .toggle_pane_sync(pane_id)
    }

    /// The panes that also get what is typed into the focused pane of the
    /// workspace on screen. Empty unless its tab syncs and the focused pane
    /// is in the set.
    #[cfg(test)]
    pub(crate) fn sync_peer_panes(&self, ws_idx: usize) -> Vec<PaneId> {
        let Some(ws) = self.workspaces.get(ws_idx) else {
            return Vec::new();
        };
        let (Some(focused), Some(tab)) = (ws.focused_pane_id(), ws.active_tab()) else {
            return Vec::new();
        };
        tab.sync_peers(focused)
    }
}

#[cfg(test)]
mod tests {
    use crate::app::state::AppState;
    use crate::layout::PaneId;
    use crate::workspace::Workspace;
    use ratatui::layout::Direction;

    /// One workspace, one tab, three panes (a, b, c), `a` focused.
    fn three_panes() -> (AppState, [PaneId; 3]) {
        let mut state = AppState::test_new();
        let mut ws = Workspace::test_new("one");
        let a = ws.tabs[0].root_pane;
        let b = ws.test_split(Direction::Horizontal);
        let c = ws.test_split(Direction::Vertical);
        ws.tabs[0].layout.focus_pane(a);
        state.workspaces = vec![ws];
        state.active = Some(0);
        state.selected = 0;
        state.ensure_test_terminals();
        (state, [a, b, c])
    }

    #[test]
    fn sync_is_off_by_default_and_toggles_per_tab() {
        let (mut state, _) = three_panes();
        assert!(!state.workspaces[0].tabs[0].is_syncing());
        assert!(state.sync_peer_panes(0).is_empty());

        assert!(state.toggle_sync_panes());
        assert!(state.workspaces[0].tabs[0].is_syncing());
        assert!(!state.toggle_sync_panes());
        assert!(!state.workspaces[0].tabs[0].is_syncing());
    }

    #[test]
    fn turning_it_on_puts_every_pane_in_and_the_peers_exclude_the_focused_one() {
        let (mut state, [a, b, c]) = three_panes();
        state.toggle_sync_panes();
        let tab = &state.workspaces[0].tabs[0];
        assert!([a, b, c].iter().all(|pane| tab.pane_synced(*pane)));
        let peers = state.sync_peer_panes(0);
        assert_eq!(peers.len(), 2);
        assert!(peers.contains(&b) && peers.contains(&c) && !peers.contains(&a));
    }

    #[test]
    fn a_pane_can_be_taken_out_and_put_back() {
        let (mut state, [_, b, c]) = three_panes();
        state.toggle_sync_panes();

        assert_eq!(state.toggle_pane_sync(0, b), Some(false));
        assert_eq!(state.sync_peer_panes(0), vec![c]);
        assert_eq!(state.toggle_pane_sync(0, b), Some(true));
        assert_eq!(state.sync_peer_panes(0).len(), 2);
    }

    #[test]
    fn an_excluded_focused_pane_is_typed_into_alone() {
        let (mut state, [a, _, _]) = three_panes();
        state.toggle_sync_panes();
        state.toggle_pane_sync(0, a);
        assert!(state.sync_peer_panes(0).is_empty());
    }

    #[test]
    fn toggling_a_pane_does_nothing_while_the_tab_does_not_sync() {
        let (mut state, [_, b, _]) = three_panes();
        assert_eq!(state.toggle_pane_sync(0, b), None);
        assert!(state.sync_peer_panes(0).is_empty());
    }

    #[test]
    fn a_pane_made_while_sync_is_on_joins_and_a_closed_one_leaves() {
        let (mut state, [a, b, _]) = three_panes();
        state.toggle_sync_panes();
        let d = state.workspaces[0].test_split(Direction::Horizontal);
        state.workspaces[0].tabs[0].layout.focus_pane(a);
        assert!(
            state.workspaces[0].tabs[0].pane_synced(d),
            "a new pane joins"
        );

        state.toggle_pane_sync(0, b);
        state.workspaces[0].tabs[0].panes.remove(&b);
        assert!(
            !state.workspaces[0].tabs[0].pane_synced(b),
            "a closed pane is out"
        );
    }

    #[test]
    fn turning_it_on_again_forgets_earlier_exclusions() {
        let (mut state, [_, b, _]) = three_panes();
        state.toggle_sync_panes();
        state.toggle_pane_sync(0, b);
        state.toggle_sync_panes();
        state.toggle_sync_panes();
        assert!(state.workspaces[0].tabs[0].pane_synced(b));
    }

    #[test]
    fn another_tab_is_not_touched() {
        let (mut state, _) = three_panes();
        state.workspaces[0].test_add_tab(Some("two"));
        state.workspaces[0].switch_tab(0);
        state.toggle_sync_panes();
        assert!(state.workspaces[0].tabs[0].is_syncing());
        assert!(!state.workspaces[0].tabs[1].is_syncing());
    }
}
