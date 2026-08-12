Ordered so each group lands green on its own. Group 1 is pure data and carries
most of the test weight; group 2 is the dispatch that makes the new destinations
reachable; group 3 is what the user sees.

No prerequisite change. `overlay-ui-kit` will later absorb the picker's geometry
into the shared panel kit — keep the rendering in group 3 expressed through
`src/ui/widgets.rs` helpers so that absorption stays a move, not a rewrite.

## 1. Destinations as data

- [ ] 1.1 Replace `PaneMoveTargetEntry`'s tab-only shape in `src/app/state.rs` with an entry carrying the display fields plus a `PaneMoveTarget` (`Tab { tab_id }` / `NewTab { workspace_id }` / `NewSpace`) and the id of the space it belongs to
- [ ] 1.2 Add the space-heading rows to `PaneMoveTargetPickerState` as non-selectable items, so render and selection read one list rather than deriving headings twice
- [ ] 1.3 Rewrite `pane_move_target_picker_for_state` (`src/app/input/navigate.rs`) to enumerate every space: own space first, then the rest in sidebar order; tabs in tab order, then that space's new-tab entry; the new-space entry last
- [ ] 1.4 Keep the existing exclusions (source tab, zoomed tabs) and keep returning `Err` — not an empty picker — when nothing is offerable. Note that the new-space destination means "nothing offerable" is now reachable only when the pane cannot be moved at all
- [ ] 1.5 Selection movement skips headings in both directions, and clamps without landing on one at either end
- [ ] 1.6 Tests on `AppState::test_new()`: ordering across three spaces; source tab excluded; headings unselectable and skipped by next/prev; a single-space single-tab single-pane session still offers new-space; a zoomed source still errors

## 2. Dispatch

- [ ] 2.1 Map `PaneMoveTarget` onto `PaneMoveDestination` at submit — `Tab`, `NewTab { workspace_id }`, `NewWorkspace { label: None, tab_label: None }` — all with `focus: true`
- [ ] 2.2 Route every variant through the existing `dispatch_pane_move_with_feedback` so `pane.move` rejections surface exactly as they do for tab moves
- [ ] 2.3 Confirm the active space follows the pane for cross-space and new-space moves, and that a source tab emptied by the move does not linger
- [ ] 2.4 Tests: each destination variant produces the expected `pane.move` params; a cross-space move leaves the destination space active with the pane focused; moving the last pane of a tab to a new space leaves no empty tab

## 3. Picker rendering and discoverability

- [ ] 3.1 Render space headings in the existing modal language — a dim, non-selectable row, matching the sidebar's section-heading weight rather than inventing a style
- [ ] 3.2 Size the picker for the grouped list, with the existing scroll behaviour when it exceeds the available height
- [ ] 3.3 Confirm `prefix+m`'s `help_entry` still reads correctly now that it moves more than "to tab"; reword if it does not, keeping the entry present while unbound
- [ ] 3.4 Render tests: headings render dim and unselected; a destination in another space shows its space; the new-space row renders last

## 4. Docs

- [ ] 4.1 Update `docs/next/website/src/content/docs/keyboard.mdx` for what `prefix+m` now reaches
- [ ] 4.2 Changelog entry under `docs/next/CHANGELOG.md`
- [ ] 4.3 ja/zh-cn parity per the docs translation check, or record the gap on the existing backfill issue

## 5. Verification

- [ ] 5.1 `just check` green
- [ ] 5.2 Dogfood on the `-ac-beta` channel: move a running agent pane to another space and to a new space, and confirm the process and scrollback survive both
- [ ] 5.3 Confirm the diff stays free of fork-only styling, so the change can be lifted as a UI-only upstream PR (refs the wayfinder proposal pack)
