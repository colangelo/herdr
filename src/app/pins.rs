//! Pinning spaces and agents to the top of the sidebar (fork issue 148).
//!
//! A pin is a number: pinned rows are listed first by ascending `pin_order`, so
//! the first pinned is first and the next second, under every sort. Pinning
//! takes one more than the largest number in use among the same kind of row,
//! which needs no stored counter and continues where a restored session left
//! off. State only: the order itself is applied where the lists are built.

use super::state::AppState;

impl AppState {
    /// Pin the space at `ws_idx` below the spaces pinned before it. A space
    /// that is already pinned keeps its place. `true` when something changed.
    pub(crate) fn pin_workspace(&mut self, ws_idx: usize) -> bool {
        let next = self
            .workspaces
            .iter()
            .filter_map(|ws| ws.pin_order)
            .max()
            .map_or(1, |max| max + 1);
        let Some(ws) = self.workspaces.get_mut(ws_idx) else {
            return false;
        };
        if ws.pin_order.is_some() {
            return false;
        }
        ws.pin_order = Some(next);
        self.mark_session_dirty();
        true
    }

    /// Unpin the space at `ws_idx`; it goes back where the sort puts it.
    pub(crate) fn unpin_workspace(&mut self, ws_idx: usize) -> bool {
        let Some(ws) = self.workspaces.get_mut(ws_idx) else {
            return false;
        };
        if ws.pin_order.take().is_none() {
            return false;
        }
        self.mark_session_dirty();
        true
    }

    /// Pin an unpinned space, unpin a pinned one. `true` when pinned now.
    pub(crate) fn toggle_pin_workspace(&mut self, ws_idx: usize) -> bool {
        if self.unpin_workspace(ws_idx) {
            return false;
        }
        self.pin_workspace(ws_idx)
    }
}

#[cfg(test)]
mod tests {
    use crate::app::state::AppState;
    use crate::workspace::Workspace;

    fn three_spaces() -> AppState {
        let mut state = AppState::test_new();
        state.workspaces = ["a", "b", "c"]
            .into_iter()
            .map(Workspace::test_new)
            .collect();
        state
    }

    #[test]
    fn pins_take_the_next_number_and_keep_their_place() {
        let mut state = three_spaces();
        assert!(state.pin_workspace(2));
        assert!(state.pin_workspace(0));
        assert_eq!(state.workspaces[2].pin_order, Some(1));
        assert_eq!(state.workspaces[0].pin_order, Some(2));
        assert!(!state.pin_workspace(2), "pinning again changes nothing");
        assert_eq!(state.workspaces[2].pin_order, Some(1));
        assert_eq!(state.workspaces[1].pin_order, None);
    }

    #[test]
    fn unpin_clears_and_a_later_pin_goes_last() {
        let mut state = three_spaces();
        state.pin_workspace(0);
        state.pin_workspace(1);
        assert!(state.unpin_workspace(0));
        assert!(!state.unpin_workspace(0), "already unpinned");
        assert!(state.pin_workspace(0));
        assert_eq!(state.workspaces[1].pin_order, Some(2));
        assert_eq!(state.workspaces[0].pin_order, Some(3), "re-pinned last");
    }

    #[test]
    fn toggle_flips_and_reports_the_new_state() {
        let mut state = three_spaces();
        assert!(state.toggle_pin_workspace(1));
        assert!(!state.toggle_pin_workspace(1));
        assert_eq!(state.workspaces[1].pin_order, None);
        assert!(!state.toggle_pin_workspace(9), "no such space");
    }

    #[test]
    fn pinned_spaces_hold_the_state_invariants_even_on_adversarial_state() {
        let mut state = AppState::test_with_adversarial_identity_state();
        state.workspaces.push(Workspace::test_new("extra"));
        state.ensure_test_terminals();
        state.pin_workspace(1);
        state.pin_workspace(0);
        state.assert_invariants_for_test();
        state.unpin_workspace(1);
        state.pin_workspace(1);
        state.assert_invariants_for_test();
    }
}
