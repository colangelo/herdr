## 1. The kit

- [ ] 1.1 `ListCursor::scroll_by`: move the window, leave the selection, clamp at both ends
- [ ] 1.2 `reveal_scroll_with_context`: reveal the selection, then pull the context index in when both fit

## 2. The board

- [ ] 2.1 `TodoBoardState::group_heading_index`: the nearest heading at or before an index
- [ ] 2.2 Selection-driven scrolling reveals the selection with its heading as context
- [ ] 2.3 Pointer motion updates the hovered button only, never the selection
- [ ] 2.4 The wheel scrolls the window instead of moving the selection
- [ ] 2.5 A click on an unselected row selects and returns; on the selected row it opens the owner; a chip still follows its link

## 3. Tests

- [ ] 3.1 The first heading is reachable again after scrolling to the bottom and back, by keyboard and by wheel
- [ ] 3.2 A group heading stays visible while its todos are selected, and the selection wins when the group is taller than the window
- [ ] 3.3 Pointer motion does not move the selection; the wheel does not move the selection
- [ ] 3.4 First click selects, second click on the same row opens the owner, chip follows on the first click

## 4. Verification

- [ ] 4.1 `just check` green
- [ ] 4.2 Dogfood on the `-ac-beta` channel against the real 37-todo board
