Ordered so each group lands green on its own. Group 1 is pure data and carries
the ordering tests; group 2 is the surface; group 3 makes rows actionable.

Prerequisites:

- The panel footer convention (`FOOTER_ROWS` / `footer_split` in
  `src/ui/widgets.rs`) — the board is a new panel with a footer button row and
  must not reintroduce the flush layout.
- `pane-todos-ux` and `todo-editing-keyboard` archived, since group 3 reuses the
  link-following and editing behaviour those changes are still holding open.

`overlay-ui-kit` will absorb this board's geometry into the shared panel kit.
Build group 2 on the existing `src/ui/widgets.rs` helpers and resist adding a
third bespoke `*_rect` / `*_button_rects` / `*_list_window` triple, so that
absorption is a move rather than a rewrite.

## 1. The aggregate, as data

- [ ] 1.1 Add a board projection on `AppState`: every pane holding todos, grouped space > tab > pane, each group carrying the pane's addressable id and label and its todos in existing presentation order
- [ ] 1.2 Reuse `pane_todos_in_display_order` per pane rather than re-deriving ordering, so the board and the panel cannot drift
- [ ] 1.3 Omit panes with no todos
- [ ] 1.4 Board state: selection over selectable rows only, with headings interleaved as non-selectable items so render and selection read one list
- [ ] 1.5 Selection movement skips headings in both directions and clamps without landing on one at either end
- [ ] 1.6 Tests on `AppState::test_new()`: grouping order across two spaces; within-pane ordering matches the panel's; empty panes omitted; headings skipped by next/prev; selection survives a todo being removed underneath it

## 2. The board surface

- [ ] 2.1 Add `src/ui/todo_board.rs` rendering a centred modal from the existing shell, header and footer-button row, reserving the footer block
- [ ] 2.2 Render group headings with the addressable id leading, then the label, in the dim non-selectable style the move picker uses
- [ ] 2.3 Render rows identically to the pane todo panel's — same marker, priority colour, done styling, link chip — so a todo looks the same in both surfaces
- [ ] 2.4 Scroll window when the list exceeds the available height, keeping the selection visible
- [ ] 2.5 Empty state when no pane holds a todo, with the footer still offering close
- [ ] 2.6 `KeysConfig` action shipping unbound, plus its `help_entry` in `src/ui/keybind_help.rs` (present while unbound), and the board's own chords in the help panel's fixed-chord section
- [ ] 2.7 Render tests: heading style and content; a row matching the panel's rendering; the separator row above the footer blank; the empty state

## 3. Actions

- [ ] 3.1 Wire the panel's actions at board scope under the same keys: toggle done, edit, remove, clear done, follow link, close
- [ ] 3.2 Activation (Enter) focuses the todo's owning pane through the existing focus path and closes the board
- [ ] 3.3 Keep `g` targeting the *linked* pane, so a linked todo's two destinations stay distinct
- [ ] 3.4 Editing from the board opens the existing edit modal and writes against the owning pane
- [ ] 3.5 Mouse: click to select, click a footer button, click outside to dismiss — matching the panel's hit-testing rules
- [ ] 3.6 Tests: activation focuses the owner across a space boundary and closes the board; `g` reaches the link, not the owner; a toggle from the board is visible in that pane's panel; removing the last todo of a pane drops its heading

## 4. Docs

- [ ] 4.1 Update `docs/next/website/src/content/docs/keyboard.mdx` with the board action and its chords
- [ ] 4.2 Changelog entry under `docs/next/CHANGELOG.md`
- [ ] 4.3 ja/zh-cn parity per the docs translation check, or record the gap on the existing backfill issue

## 5. Verification

- [ ] 5.1 `just check` green
- [ ] 5.2 Confirm the board adds no per-render work to the pane-scaled paths — it renders only while open, and the projection is built on open, not per frame
- [ ] 5.3 Dogfood on the `-ac-beta` channel: todos on panes in two spaces, triage from the board, jump to an owner across a space boundary
