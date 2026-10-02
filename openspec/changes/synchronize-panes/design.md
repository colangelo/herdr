## Context

Typed input arrives at the server from each client (`route_client_input`, then
`handle_terminal_key`, `handle_text_commit`, `handle_paste`) and is written to
the focused pane's runtime (`focused_runtime_in_workspace`, then
`try_send_bytes` / `send_bytes` / `send_paste`). The server also renders every
client's frame, so a flag in server state reaches every client with no protocol
work. Pane borders are drawn in `ui::panes::render_pane_borders` from
`AppState::pane_border_color`, and the red of the labels view is drawn by
`ui::display_panes` (pane chips, the summary bar, the mode bar).

## Decisions

- **State.** A per-tab `SyncPanes { excluded: HashSet<PaneId> }` held as
  `Option<SyncPanes>` on `Tab`: `Some` means the tab is syncing, and the set is
  every pane of the tab except `excluded`. Using the exclusion set means a pane
  created while sync is on joins the set by default and a closed pane leaves
  it with nothing to clean up beyond dropping its id.
- **Not persisted, not carried.** `Tab.sync` is runtime state: it is not in the
  session snapshot and not in the live-handoff snapshot. After a restart or a
  handoff the tab is not syncing. A mode that types into several panes fails
  safe by turning off, and the yellow makes a surviving mode obvious; the cost
  is a handoff turning it off, which the issue accepts. (Open point 2.)
- **Fan-out at the three input entry points.** A single helper
  `sync_targets(ws_idx) -> Vec<&TerminalRuntime>` returns the focused pane's
  runtime first, then the other synced panes' runtimes; the three paths
  (`handle_terminal_key` for encoded keys, `handle_text_commit`, `handle_paste`)
  write to each. The focused pane stays first so its echo is never delayed by
  the others. A pane that is not in the set receives nothing; if the focused
  pane itself is excluded, typing goes to it alone (the exclusion wins, so a
  pane can be opted out and still typed into).
- **Encoding per pane.** Keys are encoded for each target by that pane's own
  keyboard state (kitty keyboard, application cursor keys), not copied as bytes
  from the focused pane: two panes can disagree about how an arrow key is
  spelled. Paste uses each pane's own bracketed-paste state.
- **What is not synced.** herdr's own keys (prefix chords, navigate and the
  other modes) are never sent. Mouse events are not synced: clicks and
  selections act on the pane under the pointer only. Scrolling is not synced.
  Focus-in/out reports are not synced.
- **Right click.** While the tab syncs, a right click on a pane toggles its
  membership (no menu). A right click on a pane when the cursor is in another
  pane does the same, which is what ac asked for and is how a click works
  anyway: it acts on the pane under it. The existing pane menu gets a
  "Sync input: on/off" item for mouse users who want a menu, and when sync is
  off the right click keeps opening the pane menu as today. The pane menu item
  is how sync is turned on from the mouse for the current tab.
- **Yellow.** A new palette entry `sync_hint` (bright yellow) drives two things
  while `Tab.sync` is `Some` for the tab on screen: `pane_border_color` returns
  it for a synced pane (focused or not), and the bottom bar gets a yellow
  `SYNC <n> panes` chip. An excluded pane keeps its normal border. When the
  red labels view is also showing, the labels view wins for the bar and the
  borders keep the yellow (the two never share a surface).
- **API.** `tab.sync { tab_id, mode: on|off|toggle }` and
  `pane.sync { pane_id, mode: on|off|toggle }` (membership; only meaningful
  while the tab syncs, otherwise `sync_not_active`). `TabInfo.sync: bool` and
  `PaneInfo.synced: bool`, both optional with `skip_serializing_if`, so no
  `PROTOCOL_VERSION` bump. CLI `herdr tab sync on|off|toggle [tab]` and
  `herdr pane sync on|off|toggle [pane]`.
- **Keys.** `toggle_sync_panes`, default `prefix+shift+s` (ac, issue comment
  18977), with a `help_entry` in `ui/keybind_help.rs`.

## Risks

- **Agents.** Typing into several Claude sessions at once is a sharp edge: a
  stray Enter submits in every one of them. ac chose "all panes". Mitigations
  that need no new behaviour: the yellow hint, the per-pane opt-out, and the
  mode being off after a restart. A later option, not built now, is a one-key
  "skip agent panes" opt-out.
- **Mixed shells.** Panes at different prompts get the same keys; that is the
  point of the mode, and tmux behaves the same.
- **Paste.** Large pastes are sent to each target in turn; a slow pane does not
  block the others (each send is awaited per runtime, as the single pane path
  does today).
- **Multiplicative cost.** Per keystroke the work is one write per synced pane
  of one tab, with nothing in render, view computation or detection added. The
  render change is a colour lookup per border cell.

## Verification

Unit tests on `AppState`/`Tab` (membership, exclusion, new and closed panes),
an input test that fans a key, a text commit and a paste out to N fake runtimes
and honours exclusion, rendered-layout tests for the yellow borders and bar,
a mouse test for the right-click toggle and the menu item, an API schema
round-trip, and a live proof on a throwaway server: three panes, sync on,
type, capture all three, exclude one by right click, type again, capture.
