## Why

With no client attached, a new pane is spawned at `estimate_pane_size()` (the
first pane of whatever tab the server shows) and is never sized to its own layout
slot until a client attaches. Agent-driven workspaces created while ac is away run
their agents at a wrong width: both panes of a down split came out 39×46 where the
layout said 91×25 and 91×14.
https://gitea.cat-bluegill.ts.net/AC-forks/herdr/issues/95

## What Changes

- When the last client detaches, the server keeps that client's size as its
  no-client size instead of dropping to `server.headless_cols/rows`. Panes already
  keep that size (nothing resizes them on detach), so layout and PTYs now agree.
  The headless default applies only before any client has attached, or when a
  handoff carried no size.
- The no-client render sizes every pane to its slot at the no-client size, not
  only on a fresh server's first frame. New panes and splits therefore fit their
  slots, and existing panes are unchanged because they already have that size.

## Impact

- `src/server/headless.rs`; the `live-handoff-client` spec's carried-size
  requirement generalises to any detach.
- Cost: with no client attached, a render resizes panes whose size changed; the
  resize is a no-op compare for the rest.
