## ADDED Requirements

### Requirement: A session report outlives an older process exit

An agent session reported for a pane SHALL NOT be cleared by the detected exit
of an agent process that was last seen running before the report arrived. It
SHALL be cleared, as today, by the exit of the process that reported it.

#### Scenario: A Claude restarted with pane run keeps its session

- **WHEN** the Claude in a pane exits and a new Claude is started at once with
  `pane run`, its SessionStart hook reports a session, and detection then
  observes the first Claude's exit
- **THEN** the pane's agent session is the new Claude's

#### Scenario: A real exit still clears it

- **WHEN** the Claude that reported the pane's session exits and nothing
  replaces it
- **THEN** the pane's agent session is cleared
