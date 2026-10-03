## ADDED Requirements

### Requirement: A source can report that an agent waits on the user

herdr SHALL accept a hint for a pane through `pane.report_hint` (and
`herdr pane report-hint`): a `kind` of `question` or `permission`, the source,
the agent, an optional dialog `id`, an optional `ttl_ms` (default 15000, at most
60000) and a `seq`. A hint with a `seq` lower than the live one from the same
source SHALL be ignored. A hint with `clear` SHALL end the source's live hint.
Hints SHALL be runtime state: not saved in the session snapshot and not carried
through a live handoff. The method SHALL NOT change `PROTOCOL_VERSION`.

#### Scenario: Open and clear

- **WHEN** a source reports `question` for a pane and then reports `clear`
- **THEN** the pane reads `blocked` with reason `question` between the two and
  detection is unchanged after

#### Scenario: A stale report

- **WHEN** a report with `seq` 5 arrives after one with `seq` 9 from the same
  source
- **THEN** it is ignored

#### Scenario: Restart

- **WHEN** the server restarts or hands off with a hint live
- **THEN** no pane has a hint afterwards

### Requirement: A question hint holds the blocked state against the screen

While a live `question` hint exists for a pane whose agent is the hint's agent,
the pane SHALL be `blocked` with reason `question` whatever the screen shows,
until the hint is cleared, expires, or the agent leaves.

#### Scenario: Screen reads idle behind the dialog

- **WHEN** a `question` hint is live and the screen detection reads `idle`
- **THEN** the pane stays `blocked` with reason `question`

#### Scenario: Esc

- **WHEN** the dialog ends by Esc and the source clears the hint
- **THEN** the pane leaves `blocked` within one detection pass of the clear

### Requirement: A permission hint raises the state and the screen may clear it

While a live `permission` hint exists, the pane SHALL be `blocked` with reason
`permission`. The hint SHALL be dropped when screen detection has read
something other than `blocked` continuously for 1.5 seconds.

#### Scenario: The user answered

- **WHEN** a `permission` hint is live and the screen has shown no blocked dialog
  for 1.5 s
- **THEN** the hint is dropped and the pane shows what the screen shows

### Requirement: Hints age out

A hint SHALL expire `ttl_ms` after its last report. Expiry SHALL be checked by
the server loop on a deadline, not during rendering. A source that has an
open dialog SHALL re-report it every 5 seconds so a long dialog stays blocked
and a dead source ages out.

#### Scenario: The source died

- **WHEN** a `question` hint is live and no report arrives for 15 s
- **THEN** the hint is gone and the pane shows what the screen shows

### Requirement: No hint, no change

Without a live hint detection SHALL behave as it did before hints existed, and
no hint code SHALL run per rendered frame.

#### Scenario: A pane without the mod

- **WHEN** a Claude pane has never reported a hint
- **THEN** its blocked state and reason are exactly the screen's

### Requirement: The mod reports only what is open

The Claude Code mod SHALL report a `question` when `AskUserQuestion` opens for
the main agent, and a `permission` when `tool.check` resolves to `ask` for any
agent. It SHALL clear when the call settles or aborts, on `turn.complete`, and
on `session.end`, and SHALL spawn no process for a clear when nothing is open.
It SHALL log only a failed report.

#### Scenario: A turn ends with nothing open

- **WHEN** a turn completes and no dialog is open
- **THEN** the mod starts no process

#### Scenario: A subagent

- **WHEN** a subagent's tool call needs permission
- **THEN** the pane reads `blocked` with reason `permission`

### Requirement: One install path, gated on the Claude Code version

`herdr integration install claude` SHALL install the mod when the installed
Claude Code is version 2.1.287 or later, and SHALL otherwise install the
settings hooks only and say why, without failing. `uninstall` SHALL remove the
mod. Running Claude sessions SHALL keep working without the mod until reloaded.

#### Scenario: Old Claude

- **WHEN** Claude Code is older than 2.1.287
- **THEN** install skips the mod with a one-line reason and exits successfully

#### Scenario: A running session

- **WHEN** the mod is installed while a Claude session runs
- **THEN** that session keeps screen detection until `/reload-plugins` or a
  restart, and nothing fails
