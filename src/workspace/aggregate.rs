use std::collections::HashMap;

use crate::agent_priority::{attention_priority, display_priority};
use crate::detect::{Agent, AgentState};
use crate::layout::PaneId;
use crate::terminal::{TerminalId, TerminalState};

use super::{Tab, Workspace};

/// Detail info for a single pane, used by the agent detail panel.
pub struct PaneDetail {
    pub pane_id: PaneId,
    pub tab_idx: usize,
    pub tab_label: String,
    pub label: String,
    pub pane_label: Option<String>,
    pub terminal_title: Option<String>,
    pub terminal_title_stripped: Option<String>,
    /// Not-yet-done todos on the pane, and the highest priority among them.
    pub outstanding_todos: usize,
    pub highest_todo_priority: Option<crate::terminal::todo::TodoPriority>,
    pub agent_label: String,
    pub agent_kind_label: Option<String>,
    pub agent: Option<Agent>,
    pub state: AgentState,
    pub seen: bool,
    pub last_agent_state_change_seq: Option<u64>,
    pub state_labels: HashMap<String, String>,
    pub tokens: HashMap<String, String>,
}

impl Tab {
    /// The state this tab *is*, for anything that renders or reports it.
    /// There is deliberately no attention-ranked counterpart: nothing sorts
    /// tabs by attention today, and adding one blind would re-create the
    /// ambiguity this pair of rankings exists to remove.
    pub fn display_state(
        &self,
        terminals: &HashMap<TerminalId, TerminalState>,
    ) -> (AgentState, bool) {
        self.panes
            .values()
            .filter_map(|pane| {
                terminals
                    .get(&pane.attached_terminal_id)
                    .map(|terminal| (terminal.state, pane.seen))
            })
            .max_by_key(|(state, seen)| display_priority(*state, *seen))
            .unwrap_or((AgentState::Unknown, true))
    }

    fn pane_details(
        &self,
        terminals: &HashMap<TerminalId, TerminalState>,
        tab_idx: usize,
        tab_label: &str,
    ) -> Vec<PaneDetail> {
        self.layout
            .pane_ids()
            .iter()
            .filter_map(|id| {
                let pane = self.panes.get(id)?;
                let terminal = terminals.get(&pane.attached_terminal_id)?;
                let agent_kind_label = terminal.effective_agent_label().map(str::to_string);
                let fallback_agent_label = terminal
                    .agent_name
                    .as_deref()
                    .or(agent_kind_label.as_deref())?
                    .to_string();
                let agent_label = terminal
                    .effective_display_agent()
                    .unwrap_or_else(|| fallback_agent_label.clone());
                let presentation = terminal.effective_presentation();
                Some(PaneDetail {
                    pane_id: *id,
                    tab_idx,
                    tab_label: tab_label.to_string(),
                    label: agent_label.clone(),
                    pane_label: terminal
                        .effective_title()
                        .or_else(|| terminal.manual_label.clone()),
                    terminal_title: terminal.terminal_title.clone(),
                    terminal_title_stripped: terminal.terminal_title_stripped(),
                    outstanding_todos: terminal.outstanding_todo_count(),
                    highest_todo_priority: terminal.highest_outstanding_todo_priority(),
                    agent_label,
                    agent_kind_label,
                    agent: terminal.effective_known_agent(),
                    state: terminal.state,
                    seen: pane.seen,
                    last_agent_state_change_seq: terminal.last_agent_state_change_seq,
                    state_labels: presentation.state_labels,
                    tokens: terminal.metadata_tokens.values(),
                })
            })
            .collect()
    }
}

impl Workspace {
    fn pane_states<'a>(
        &'a self,
        terminals: &'a HashMap<TerminalId, TerminalState>,
    ) -> impl Iterator<Item = (AgentState, bool)> + 'a {
        self.tabs
            .iter()
            .flat_map(|tab| tab.panes.values())
            .filter_map(|pane| {
                terminals
                    .get(&pane.attached_terminal_id)
                    .map(|terminal| (terminal.state, pane.seen))
            })
    }

    /// The state this workspace *is*, for anything that renders or reports it.
    pub fn display_state(
        &self,
        terminals: &HashMap<TerminalId, TerminalState>,
    ) -> (AgentState, bool) {
        self.pane_states(terminals)
            .max_by_key(|(state, seen)| display_priority(*state, *seen))
            .unwrap_or((AgentState::Unknown, true))
    }

    /// The state this workspace *wants the user for*, for attention-ordered
    /// sorting. Not interchangeable with [`Workspace::display_state`]: a
    /// finished-but-unseen pane outranks a working one here and only here.
    pub fn attention_state(
        &self,
        terminals: &HashMap<TerminalId, TerminalState>,
    ) -> (AgentState, bool) {
        self.pane_states(terminals)
            .max_by_key(|(state, seen)| attention_priority(*state, *seen))
            .unwrap_or((AgentState::Unknown, true))
    }

    /// Most recent agent state change across all panes, for recency tiebreaks
    /// in attention-sorted lists. `None` when no pane has recorded a change.
    pub fn last_agent_state_change_seq(
        &self,
        terminals: &HashMap<TerminalId, TerminalState>,
    ) -> Option<u64> {
        self.tabs
            .iter()
            .flat_map(|tab| tab.panes.values())
            .filter_map(|pane| {
                terminals
                    .get(&pane.attached_terminal_id)
                    .and_then(|terminal| terminal.last_agent_state_change_seq)
            })
            .max()
    }

    pub fn pane_details(&self, terminals: &HashMap<TerminalId, TerminalState>) -> Vec<PaneDetail> {
        let multi_tab = self.tabs.len() > 1;
        self.tabs
            .iter()
            .enumerate()
            .flat_map(|(tab_idx, tab)| {
                let tab_label = self
                    .tab_display_name(tab_idx)
                    .unwrap_or_else(|| (tab_idx + 1).to_string());
                tab.pane_details(terminals, tab_idx, &tab_label).into_iter()
            })
            .map(|mut detail| {
                if multi_tab {
                    detail.label = format!("{}·{}", detail.tab_label, detail.agent_label);
                }
                detail
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Direction;

    use super::*;
    use crate::detect::Agent;

    fn terminal_for_pane(ws: &Workspace, pane_id: PaneId) -> TerminalState {
        TerminalState::new(ws.terminal_id(pane_id).unwrap().clone(), "/tmp".into())
    }

    /// A single-tab workspace whose panes carry the given `(state, seen)` pairs,
    /// one pane per pair, plus the terminal map backing them.
    fn workspace_with_pane_states(
        states: &[(AgentState, bool)],
    ) -> (Workspace, HashMap<TerminalId, TerminalState>) {
        let mut ws = Workspace::test_new("test");
        for _ in 1..states.len() {
            ws.test_split(Direction::Horizontal);
        }
        let pane_ids = ws.tabs[0].layout.pane_ids();
        assert_eq!(pane_ids.len(), states.len());

        let mut terminals = HashMap::new();
        for (pane_id, (state, seen)) in pane_ids.iter().zip(states) {
            let mut terminal = terminal_for_pane(&ws, *pane_id);
            terminal.state = *state;
            terminals.insert(terminal.id.clone(), terminal);
            ws.tabs[0].panes.get_mut(pane_id).unwrap().seen = *seen;
        }
        (ws, terminals)
    }

    #[test]
    fn display_state_all_unknown() {
        let ws = Workspace::test_new("test");
        let mut terminals = HashMap::new();
        let root = ws.tabs[0].root_pane;
        let terminal = terminal_for_pane(&ws, root);
        terminals.insert(terminal.id.clone(), terminal);
        let (state, seen) = ws.display_state(&terminals);
        assert_eq!(state, AgentState::Unknown);
        assert!(seen);
    }

    #[test]
    fn display_state_prefers_working_over_seen_idle() {
        let mut ws = Workspace::test_new("test");
        let id2 = ws.test_split(Direction::Horizontal);
        let root_id = ws.tabs[0]
            .panes
            .keys()
            .find(|id| **id != id2)
            .copied()
            .unwrap();
        let mut terminals = HashMap::new();
        let mut root_terminal = terminal_for_pane(&ws, root_id);
        root_terminal.state = AgentState::Idle;
        terminals.insert(root_terminal.id.clone(), root_terminal);
        let mut second_terminal = terminal_for_pane(&ws, id2);
        second_terminal.state = AgentState::Working;
        terminals.insert(second_terminal.id.clone(), second_terminal);

        let (state, seen) = ws.display_state(&terminals);

        assert_eq!(state, AgentState::Working);
        assert!(seen);
    }

    /// The issue-39 fix: a pane that finished while unseen ("done") must not
    /// mask an actively working sibling. The same pair still ranks the other
    /// way for attention, which is what keeps the priority sorts useful.
    ///
    /// <https://gitea.cat-bluegill.ts.net/AC-forks/herdr/issues/39>
    #[test]
    fn display_state_working_beats_done_unseen() {
        let (ws, terminals) =
            workspace_with_pane_states(&[(AgentState::Idle, false), (AgentState::Working, true)]);

        assert_eq!(ws.display_state(&terminals), (AgentState::Working, true));
        assert_eq!(ws.attention_state(&terminals), (AgentState::Idle, false));
    }

    /// The displayed state is a function of the panes a row covers and nothing
    /// else. Marking a finished sibling seen — which is what focusing a pane
    /// does to every pane in its tab — must not change what the row shows.
    ///
    /// Before the fix this was the entire bug: the row read `done` until a
    /// focus switch dropped the finished pane's rank, and only then revealed
    /// the working agent that had been there the whole time.
    #[test]
    fn display_state_is_unchanged_by_focusing_a_done_sibling() {
        let (mut ws, terminals) =
            workspace_with_pane_states(&[(AgentState::Idle, false), (AgentState::Working, true)]);

        let before = ws.display_state(&terminals);

        // The tab-wide `seen` sweep that focusing any pane in the tab performs.
        for pane in ws.tabs[0].panes.values_mut() {
            pane.seen = true;
        }

        assert_eq!(ws.display_state(&terminals), before);
        assert_eq!(before, (AgentState::Working, true));
    }

    /// Must survive the display/attention split: blocked outranks working in
    /// both rankings, because a blocked agent cannot proceed at all.
    #[test]
    fn display_state_blocked_beats_working() {
        let (ws, terminals) =
            workspace_with_pane_states(&[(AgentState::Working, true), (AgentState::Blocked, true)]);

        assert_eq!(ws.display_state(&terminals), (AgentState::Blocked, true));
    }

    /// Must survive the display/attention split: a finished-and-unseen pane
    /// still outranks one that is idle and already seen.
    #[test]
    fn display_state_done_beats_seen_idle() {
        let (ws, terminals) =
            workspace_with_pane_states(&[(AgentState::Idle, true), (AgentState::Idle, false)]);

        assert_eq!(ws.display_state(&terminals), (AgentState::Idle, false));
    }

    /// Tab-level aggregation is a display path too — the navigator's tab rows
    /// and the API's `TabInfo.agent_status` both read it.
    #[test]
    fn tab_display_state_working_beats_done_unseen() {
        let (ws, terminals) =
            workspace_with_pane_states(&[(AgentState::Idle, false), (AgentState::Working, true)]);

        assert_eq!(
            ws.tabs[0].display_state(&terminals),
            (AgentState::Working, true)
        );
    }

    #[test]
    fn tab_display_state_is_unknown_without_terminals() {
        let ws = Workspace::test_new("test");

        assert_eq!(
            ws.tabs[0].display_state(&HashMap::new()),
            (AgentState::Unknown, true)
        );
    }

    #[test]
    fn last_agent_state_change_seq_is_max_across_panes() {
        let mut ws = Workspace::test_new("test");
        let id2 = ws.test_split(Direction::Horizontal);
        let root_id = ws.tabs[0]
            .panes
            .keys()
            .find(|id| **id != id2)
            .copied()
            .unwrap();
        let mut terminals = HashMap::new();
        let mut root_terminal = terminal_for_pane(&ws, root_id);
        root_terminal.last_agent_state_change_seq = Some(3);
        terminals.insert(root_terminal.id.clone(), root_terminal);
        let mut second_terminal = terminal_for_pane(&ws, id2);
        second_terminal.last_agent_state_change_seq = Some(7);
        terminals.insert(second_terminal.id.clone(), second_terminal);

        assert_eq!(ws.last_agent_state_change_seq(&terminals), Some(7));
    }

    #[test]
    fn last_agent_state_change_seq_none_without_changes() {
        let ws = Workspace::test_new("test");
        let mut terminals = HashMap::new();
        let root = ws.tabs[0].root_pane;
        let terminal = terminal_for_pane(&ws, root);
        terminals.insert(terminal.id.clone(), terminal);

        assert_eq!(ws.last_agent_state_change_seq(&terminals), None);
    }

    #[test]
    fn pane_details_prefers_agent_name_over_detected_agent_label() {
        let ws = Workspace::test_new("test");
        let root_pane = ws.tabs[0].root_pane;
        let mut terminals = HashMap::new();
        let mut terminal = terminal_for_pane(&ws, root_pane);
        terminal.set_detected_state(Some(Agent::Pi), AgentState::Working);
        terminal.set_agent_name("planner".into());
        terminals.insert(terminal.id.clone(), terminal);

        let labels: Vec<_> = ws
            .pane_details(&terminals)
            .into_iter()
            .map(|detail| (detail.label, detail.agent_label, detail.agent))
            .collect();

        assert_eq!(
            labels,
            vec![("planner".into(), "planner".into(), Some(Agent::Pi))]
        );
    }

    #[test]
    fn pane_details_includes_tab_context_for_multi_tab_workspace() {
        let mut ws = Workspace::test_new("test");
        ws.tabs[0].custom_name = Some("main".into());
        let root_pane = ws.tabs[0].root_pane;
        let second_tab = ws.test_add_tab(Some("review"));
        let review_pane = ws.tabs[second_tab].root_pane;
        let mut terminals = HashMap::new();
        let mut root_terminal = terminal_for_pane(&ws, root_pane);
        root_terminal.set_hook_authority(
            "test".into(),
            "pi".into(),
            AgentState::Working,
            None,
            None,
        );
        terminals.insert(root_terminal.id.clone(), root_terminal);
        let mut review_terminal = terminal_for_pane(&ws, review_pane);
        review_terminal.set_hook_authority(
            "test".into(),
            "claude".into(),
            AgentState::Idle,
            None,
            None,
        );
        terminals.insert(review_terminal.id.clone(), review_terminal);

        let labels: Vec<_> = ws
            .pane_details(&terminals)
            .into_iter()
            .map(|detail| (detail.label, detail.agent_label, detail.agent))
            .collect();

        assert_eq!(
            labels,
            vec![
                ("main·pi".into(), "pi".into(), Some(Agent::Pi)),
                ("review·claude".into(), "claude".into(), Some(Agent::Claude)),
            ]
        );
    }

    #[test]
    fn pane_details_use_tab_vector_index_not_stable_public_tab_number() {
        let mut ws = Workspace::test_new("test");
        let removed_tab = ws.test_add_tab(Some("removed"));
        let survivor_tab = ws.test_add_tab(Some("survivor"));
        let survivor_pane = ws.tabs[survivor_tab].root_pane;
        assert!(ws.close_tab(removed_tab));

        let mut terminals = HashMap::new();
        let mut terminal = terminal_for_pane(&ws, survivor_pane);
        terminal.detected_agent = Some(Agent::Codex);
        terminals.insert(terminal.id.clone(), terminal);

        let details = ws.pane_details(&terminals);
        let survivor = details
            .iter()
            .find(|detail| detail.pane_id == survivor_pane)
            .expect("surviving tab agent should be listed");

        assert_eq!(ws.tabs[1].number, 3);
        assert_eq!(survivor.tab_idx, 1);
    }
}
