## ADDED Requirements

### Requirement: Panes created with no client attached fit their slots

With no client attached, herdr SHALL size every pane to its layout slot at the
no-client size, which SHALL be the last attached client's size, or the size a
handoff carried, or `server.headless_cols/rows` when neither exists.

#### Scenario: A split made while detached fits its slots

- **WHEN** no client is attached and a pane is split down with ratio 0.65
- **THEN** each pane's PTY matches its layout rect

#### Scenario: Detaching keeps the client's size

- **WHEN** a 173×59 client detaches
- **THEN** the server's no-client size is 173×59 and existing panes keep their size
