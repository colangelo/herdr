Planned 2026-09-28, not started: built in the batch with JOB 8 (#102), which ac approved.
Issues: https://gitea.cat-bluegill.ts.net/AC-forks/herdr/issues/103 to /108 (umbrella #108).

## 1. Tests first (red)

- [ ] 1.1 #103 (already committed, `#[ignore]`d, red): un-ignore `move_picker_names_a_space_made_by_a_move_the_way_the_sidebar_does` (`src/app/input/navigate.rs`). Today: headings `["master", "master"]`.
- [ ] 1.2 #103: `pane_move_to_new_space_seeds_identity_from_the_live_cwd` (`src/app/api/panes.rs`)
- [ ] 1.3 #108 kit: `list_search_matches_every_word_ignoring_case`, `list_search_keys_route_chords_edits_and_esc` (`src/ui/overlay/search.rs`)
- [ ] 1.4 #108 kit: `centred_panel_is_measured_bounded_and_centred`, `detail_is_omitted_under_three_rows` (`src/ui/overlay/geometry.rs`)
- [ ] 1.5 #105: `navigator_is_at_most_120_wide_in_a_310x56_window` (`src/ui/navigator.rs`); `snapshot_navigator` must still pass unedited after the search extraction
- [ ] 1.6 #104: `move_picker_slash_filters_to_a_space_and_enter_moves_there`, `move_picker_heading_stays_while_a_destination_matches`, `move_picker_no_match_keeps_the_query` (`src/app/input/navigate.rs`)
- [ ] 1.7 #104/#108: `snapshot_move_picker` and `snapshot_move_picker_searching` (`src/ui/dialogs.rs`, the picker's first rendered-layout tests); `move_picker_button_row_near_miss_does_not_close`
- [ ] 1.8 #106: `detail_shows_for_a_cut_one_line_todo`, `detail_is_the_same_in_any_space`, `panel_height_is_steady_as_the_selection_moves` (`src/ui/todo_panel.rs`); `snapshot_detail_box` replaces the rule-line layout
- [ ] 1.9 #107: `board_slash_filters_todos_and_keeps_their_heading`, `board_letters_type_while_searching`, `board_esc_clears_the_query_then_closes` (`src/app/input/modal.rs`); `snapshot_board_searching` (`src/ui/todo_board.rs`)
- [ ] 1.10 #108 mouse: `inside_click_off_rows_does_not_close` for the picker, panel and board

## 2. Build (green)

- [ ] 2.1 Naming: picker headings via `display_name_from` (the builder takes the runtime registry); new-space identity from the live cwd; audit the other user-facing `display_name_from_terminals` callers
- [ ] 2.2 Kit: `src/ui/overlay/search.rs` (`ListSearch`, matching, keys, search row, caret); the navigator moves onto it
- [ ] 2.3 Kit: centred placement in `AnchoredPanelSpec`, `LIST_DIALOG_MAX_WIDTH = 120`, detail minimum of 3 rows; the navigator, picker and board resolve through it (`TodoBoardGeometry` and the percentage margins go)
- [ ] 2.4 Move picker: search, `ButtonRow`, `ListCursor::window`, half a page = visible rows
- [ ] 2.5 Detail box helper; the pane todo panel and the board use it; reserved height across the visible todos
- [ ] 2.6 Todo board: search; letter commands in list focus only
- [ ] 2.7 Uniform mouse rule (hover selects, row click activates, inside off-row click inert, outside closes)
- [ ] 2.8 Docs: `docs/next/website/src/content/docs/keyboard.mdx` (search in the picker and the board, Esc order, the detail box); no CHANGELOG edit

## 3. Ship (with the batch)

- [ ] 3.1 `just check` green; `openspec validate dialog-overhaul --strict`
- [ ] 3.2 Beta (the same beta as JOB 8); live proof on m4m at 310x56: navigator at most 120 wide and centred; move a pane to a new space and reopen the picker, it shows the new space's own name; `/` in the picker and on the board; a multi-line and a cut one-line todo show the boxed detail in "master" and in "herdr"
- [ ] 3.3 Comment the SHAs, beta and implementation notes on #103 to #108; close each after the live check
