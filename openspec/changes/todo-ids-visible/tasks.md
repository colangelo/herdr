## 1. Rows

- [ ] 1.1 `render_pane_todo_row` draws `#<id>` dim at the row's right edge, with the link chip beside it and the text truncating first
- [ ] 1.2 The board's content-width measurement includes the id

## 2. The editor

- [ ] 2.1 Title reads `edit todo/note #<id>` when editing; `new todo/note` unchanged when composing

## 3. Tests

- [ ] 3.1 Row shows the id; with a chip both survive and the text truncates first
- [ ] 3.2 Editor title with and without an id
- [ ] 3.3 Re-record board snapshots

## 4. Verification

- [ ] 4.1 `just check` green
- [ ] 4.2 Dogfood on the `-ac-beta` channel, including referencing a shown id straight into `herdr todo done`
