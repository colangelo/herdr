Planned 2026-09-29 (JOB 14 from herdr-helper; ac approved the design step for
https://gitea.cat-bluegill.ts.net/AC-forks/herdr/issues/126). Code waits for
approval of this change.

## 1. Tests first (red)

- [ ] 1.1 terminal: epoch changes on begin and end only; begin returns a render delay of `SYNC_HOLD_MAX` (`src/pane/terminal.rs`)
- [ ] 1.2 helper: mid-block pane in the target holds; past the deadline draws; hidden tab does not hold; popup counts
- [ ] 1.3 server: full render holds and owes a frame; the end sends it; epoch change during the build drops the frame; retained patch falls back (`src/server/headless.rs` tests)

## 2. Build (green)

- [ ] 2.1 epoch + begin instant in `GhosttyPaneCore`, accessor through `PaneRuntime` / `TerminalRuntime`
- [ ] 2.2 the helper, used by full render, retained path and local loop
- [ ] 2.3 hold + owed render in the headless per-client loop

## 3. Verify

- [ ] 3.1 `just bench-render-scale` 1 vs 15 panes, before/after, numbers on the issue
- [ ] 3.2 #126 reproducer against the beta: torn-at-middle with keys goes to 0
- [ ] 3.3 `just check`, push origin and internal
