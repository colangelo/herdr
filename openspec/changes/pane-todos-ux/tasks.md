Ordered so each group lands green on its own. Group 1 is the correctness fix and
must precede group 4; shipping the picker without it would create links that
silently vanish on save.

## 1. Session-wide public pane id (correctness)

- [ ] 1.1 Add a session-wide public pane id lookup in `src/app/ids.rs` that locates the pane's own workspace, mirroring the search `AppState::pane_todo_link_target` already performs
- [ ] 1.2 Use it in `save_pane_todo_edit_via_api` (`src/app/input/modal.rs`) instead of resolving against `self.state.active`
- [ ] 1.3 Test: saving a link whose target is in another workspace stores the link and round-trips through snapshot restore. Mutation-check by reverting 1.2 and confirming the test fails

## 2. Always-on indicator

- [ ] 2.1 Change `pane_todo_indicator_label` (`src/ui/panes.rs`) to always return a glyph, with the count only when outstanding todos exist
- [ ] 2.2 Give the three states distinct tones: priority colour (outstanding), normal dim (all done), dimmest (empty); keep `ui.show_pane_todo_indicator = false` suppressing all three
- [ ] 2.3 Keep both existing guards: no indicator when the pane draws no top border, and the indicator wins a width squeeze (title drops itself), omitted only when the pane cannot carry the glyph plus its two border corners
- [ ] 2.4 Tests: empty pane draws the glyph and is clickable; empty and all-done render in different tones; drawn cells still equal `pane_todo_indicator_rect`; configured-off draws nothing on a pane holding todos; a pane with no top border draws nothing; the title yields before the indicator does

## 3. Add from the panel

- [ ] 3.1 Bind `a` in `handle_pane_todos_key_via_api` (`src/app/input/modal.rs`) to `AppState::open_new_pane_todo` for the panel's pane
- [ ] 3.2 Add the matching footer button in `src/ui/todo_panel.rs`, and render the footer in the empty state so an empty panel is not a dead end
- [ ] 3.3 Tests: `a` opens the edit modal on a new todo and saving returns to the panel with it listed; the empty-state panel offers the add affordance

## 4. Navigator selection mode

- [ ] 4.1 Add a navigator purpose to `AppState` (goto vs. selecting a link target for a todo), defaulting to goto and reset on close
- [ ] 4.2 In selection mode, resolve on pane rows only; make workspace and tab rows expand/collapse without resolving
- [ ] 4.3 Offer a synthetic clear-the-link entry, and exclude the todo's own pane
- [ ] 4.4 Return to `Mode::PaneTodoEdit` on both resolve and dismiss, leaving the staged link untouched on dismiss, mirroring `close_pane_todo_edit_and_return`
- [ ] 4.5 Title the overlay for the selection purpose rather than reusing the goto title
- [ ] 4.6 Capture the link label from the navigator's pane label chain, so a shell is named by its launched command
- [ ] 4.7 Point the edit modal's link control (key and click) at the picker; delete `cycle_pane_todo_edit_link` and `pane_link_candidates`
- [ ] 4.8 Tests: a pane row stages the target; workspace/tab rows do not; the clear entry clears; own pane absent; dismiss preserves the prior link; a shell target's captured label names its command

## 5. Docs and validation

- [ ] 5.1 Update `docs/next/website/src/content/docs/keyboard.mdx` for the always-on indicator, the panel add key, and the link picker replacing the cycle
- [ ] 5.2 Add the changelog entry under `docs/next/CHANGELOG.md`, and add `help_entry` rows in `src/ui/keybind_help.rs` for any new action
- [ ] 5.3 `just check` green apart from the known macOS `live_handoff` failure (fork issue #33), plus the changelog duplicate checks from the sync skill
- [ ] 5.4 Dogfood on the `-ac-beta` channel: click an empty pane's indicator, add a todo from the panel, and link it to a pane in another workspace
