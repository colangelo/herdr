Planned 2026-09-29 (JOB 14; ac: "your pick, included", comment 17422 on
https://gitea.cat-bluegill.ts.net/AC-forks/herdr/issues/129).

## 1. Tests first (red)

- [ ] 1.1 placement: centered in the pane; fallback when too small, missing, hidden
- [ ] 1.2 producers: OSC 52 and herdr copies carry the pane; a note records the focused pane
- [ ] 1.3 rendered layout: pane placement vs default; config parse and defaults

## 2. Build (green)

- [ ] 2.1 `ClipboardWrite.source_pane`, copy request pane, `CopyFeedback.source_pane`
- [ ] 2.2 `ToastNotification.anchor_pane` set by `set_pane_move_feedback`
- [ ] 2.3 config keys, template, reference, docs; placement in `render_notifications`

## 3. Verify

- [ ] 3.1 throwaway: copy in a pane with `position = "pane"`, capture the screen
- [ ] 3.2 `just check`, push, notes on #129
