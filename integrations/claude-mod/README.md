# herdr-attention (spike)

A Claude Code mod that reports `AskUserQuestion` open and closed to herdr.
Spike for https://gitea.cat-bluegill.ts.net/AC-forks/herdr/issues/137 (part C).
Not installed anywhere: load it for one session with
`claude --plugin-dir integrations/claude-mod`.

- `HERDR_PANE_ID` (set by herdr in every pane) names the pane to report on.
- `HERDR_MOD_BIN` overrides the herdr binary (default `herdr-beta`).

## What the spike showed (Claude Code 2.1.287, Haiku, throwaway herdr session)

The mod sets a `question` pane token (`open:<tool_use_id>`) when an
`AskUserQuestion` call starts and clears it when the question ends.

| Dialog ends by | Token cleared | Closed by |
|---|---|---|
| Answer (Enter) | within 1 s | `finally` after `next(e)` |
| Chat about this | within 1 s | `finally` |
| Esc | within 1 s | the abort signal (`next.signal`); `next(e)` does not settle on Esc, so `finally` alone is not enough. `turn.complete` also fires. |
| Process killed (SIGTERM) | within 1 s | herdr drops the pane's tokens |
| /clear | not testable: the dialog holds the keyboard | `session.end` hook closes it |
| Subagent question | never opened | a subagent cannot call AskUserQuestion in this build; the `agentId` guard stays as a belt |

`spike-scenario.sh` is the driver used for the table (needs a throwaway
herdr session with `claude --plugin-dir integrations/claude-mod` in w1:p1).
