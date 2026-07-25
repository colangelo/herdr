Build order is phased. **Phase 1** is groups 1–5 plus 6.1–6.3: server state,
persistence, API, CLI, indicator, and a read-only panel. That is independently
useful and independently testable — an agent can record its next steps and you
can see and follow them. **Phase 2** is 6.4–6.6: in-TUI editing and the
bindings. Group 7 closes out whichever phase is landing.

## 1. Todo store in server state

- [x] 1.1 Add `PaneTodo`, `TodoPriority`, `TodoLink` in `src/terminal/todo.rs` and the todo list + monotonic id counter to `TerminalState` in `src/terminal/state.rs` (not `PaneState`, which is viewport-only and is not what `PaneSnapshot` captures)
- [x] 1.2 Add store operations (add, update, remove, clear, toggle done) enforcing the 50-todo and 500-character limits with explicit error variants
- [x] 1.3 Add the presentation-order helper (not-done first → priority desc → creation order), keeping stored order as insertion order
- [x] 1.4 Unit tests on `TerminalState` (where the todos live, so no `AppState` or PTY is involved): add/update/remove/clear, ordering, limits, id monotonicity across removal

## 2. Persistence and link remapping

- [x] 2.1 Add `PaneTodoSnapshot` and the `todos` field to `PaneSnapshot` (`#[serde(default, skip_serializing_if = "Vec::is_empty")]`), storing link targets as old raw pane ids
- [x] 2.2 Remap todo link targets through `id_map` in `src/persist/restore.rs`; unresolvable targets become dead links that keep their label
- [ ] 2.3 Confirm pane close when outstanding todos remain, reusing the existing confirm modal (deferred to phase 2 with the rest of the UI work)
- [x] 2.4 Tests: snapshot round-trip with and without todos, a session file predating the field, link remap, and the unresolvable-link path

## 3. Socket API, event, protocol

- [x] 3.1 Add `src/api/schema/todos.rs` with `TodoInfo` and the params types; wire the methods into `src/api/schema.rs` and `src/api/server.rs`
- [x] 3.2 Implement `todo.list/add/update/remove/clear` handlers in `src/app/api/todos.rs`, including link target resolution by public pane id and the error codes
- [x] 3.3 Emit `todo.changed` on every mutation through `src/api/subscriptions.rs`
- [x] 3.4 Confirm no `PROTOCOL_VERSION` bump is needed (source 19 already exceeds the 18 released in `v0.7.4-ac`); leave the protocol expectations in `tests/cli/sessions.rs`, `tests/api_ping.rs`, `tests/support/mod.rs` at 19
- [x] 3.5 Regenerate `docs/next/api/herdr-api.schema.json` and extend `src/api/schema/tests.rs`

## 4. CLI

- [x] 4.1 Add `src/cli/todo.rs` with the add/list/done/undone/edit/rm/clear verbs, registered in `src/cli/spec.rs` and `src/main.rs`
- [x] 4.2 Reuse the pane target grammar (`--pane`, `--current`, `HERDR_PANE_ID` default) from `src/cli/pane.rs`; add `--priority`, `--link`, `--unlink`, `--all`, `--json`
- [x] 4.3 Tests for target resolution and flag parsing, mirroring the existing `parse_pane_current_args` tests
- [ ] 4.4 Resolve `--link` targets by unique live agent name as well as public pane id, erroring on ambiguous names (deferred out of phase 1: phase 1 resolves links with `App::parse_pane_id`, so every agent name — unique or ambiguous — returns `todo_link_unresolved`; see `design.md`)

## 5. Pane indicator

- [ ] 5.1 Add `pane_todo_indicator_rect()` and render the `▾N` glyph at the far right of the pane top border in `src/ui/panes.rs`, reserving its cells before title layout
- [ ] 5.2 Color by highest outstanding priority; add `ui.show_pane_todo_indicator` and `ui.pane_todo_color` to `src/config/model.rs` and the config reference
- [ ] 5.3 Hit-test the same rect in `src/app/input/mouse.rs`
- [ ] 5.4 Tests: drawn cells equal `pane_todo_indicator_rect`, hidden when empty, count counts only outstanding todos, narrow-pane fallback

## 6. Panel and edit modal

- [ ] 6.1 Add `Mode::PaneTodos` and `src/ui/todo_panel.rs` rendering rows (priority glyph, text, link chip), done rows dimmed and struck, dead links inert
- [ ] 6.2 Panel input in `src/app/input/modal.rs`: selection, toggle done, remove, clear done, open edit, follow link, close
- [ ] 6.3 Follow-link jumps via `focus_pane_in_workspace`; clicking the link chip does the same
- [ ] 6.4 Edit modal on the existing dialog structure: text input, `Tab` priority cycle, link set/clear, save/cancel
- [ ] 6.5 Add `keys.open_pane_todos` (default `prefix+ctrl+t`) and `keys.add_pane_todo` (unbound) with `help_entry` rows in `src/ui/keybind_help.rs`
- [ ] 6.6 Tests: panel row rendering and ordering, key handling, dead-link inertness, help panel lists both actions

## 7. Docs and validation

- [ ] 7.1 Stage the changelog entry and config-reference updates under `docs/next/`, extending existing pages rather than adding new `.mdx` that would need ja/zh-cn translations
- [ ] 7.2 Document the `herdr todo` verbs and the `todo.*` socket methods on the existing CLI and socket API pages
- [ ] 7.3 Run `just check` and dogfood the feature in a live herdr build before marking the change complete
