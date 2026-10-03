# herdr-attention

A Claude Code mod (Claude Code 2.1.287 or later) that tells herdr when Claude
waits on the user: a question (`AskUserQuestion`) or a permission prompt. Herdr
shows such a pane as `blocked` with that reason, exactly, where reading the
screen can be late or wrong (an Esc'd or half-drawn dialog).

Design and decisions: `openspec/changes/claude-mod-attention/`. Issues:
https://gitea.cat-bluegill.ts.net/AC-forks/herdr-max/issues/137 (spike) and
https://gitea.cat-bluegill.ts.net/AC-forks/herdr-max/issues/157 (build).

## How it works

- `hooks/register.js` wraps the tool call as middleware. It reports a hint
  through `hooks/herdr-hint.sh`, which writes one `pane.report_hint` request to
  `$HERDR_SOCKET_PATH` for `$HERDR_PANE_ID`. No CLI, so no dependence on which
  herdr binary name is on `PATH`; outside a herdr pane it does nothing.
- **question**: `AskUserQuestion` opens (main agent only). Closed when the call
  settles or aborts; Esc only aborts, so the abort signal is the other close.
- **permission**: `tool.check` resolved to `ask`, for any agent. Closed when the
  call settles or aborts (deny, Esc). Allow gives a mod no signal until the tool
  ends, so herdr's screen clears the hint (see the table below).
- `turn.complete` (also an interrupted turn) and `session.end` clear everything.
- Open dialogs are repeated every 5 s (herdr's TTL is 15 s), so a long dialog
  stays blocked and a dead mod ages out. A dialog open for 30 minutes is
  released, with one log line.
- `seq` is wall-clock microseconds plus a counter, so a plugin reload (which
  restarts the module's counter) never looks stale to herdr.
- It logs only failures, at most one line a minute.
- A mod's hooks module does not load until the workspace is trusted; in an
  untrusted directory nothing is reported and herdr reads the screen as before.

## What was measured (Claude Code 2.1.288, Haiku)

How the end of a permission prompt shows to a mod, relative to the key press:

| Event | Allow | Deny | Esc |
|---|---|---|---|
| `tool.call` abort | never | +8 ms | +63 ms |
| `tool.call` settles | at the tool's end | +9 ms | +63 ms |
| `classic.PermissionDenied` | never | never | never |
| `ui.render` Spinner / ToolUse | no change | no change | no change |

And for a question: answer, "Chat about this", Esc (abort) and SIGTERM each clear
in about 1 s (spike, Claude Code 2.1.287).

## Test

```bash
claude plugin validate integrations/claude-mod
claude plugin test integrations/claude-mod        # needs the claude CLI
python3 -m unittest scripts.test_claude_mod_hint  # the helper; part of just check
```

## Try it in one session

```bash
claude --plugin-dir integrations/claude-mod
```

Fleet-wide install is `herdr integration install claude`, which installs this
mod next to the Claude hooks when Claude Code is new enough.
