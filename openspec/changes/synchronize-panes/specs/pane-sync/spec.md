## ADDED Requirements

### Requirement: Sync mode types into every pane of the tab

While sync mode is on for a tab, text typed into the focused pane of that tab
SHALL also be sent to every other pane of that tab that is in the synced set.
The set SHALL be every pane of the tab when the mode is turned on, and a pane
created in the tab while the mode is on SHALL join it.

#### Scenario: Three panes

- **WHEN** a tab has panes A, B and C, sync is on, and `ls` Enter is typed into A
- **THEN** B and C receive `ls` Enter as well

#### Scenario: Another tab is untouched

- **WHEN** sync is on in tab 1 and a pane in tab 2 is focused and typed into
- **THEN** only that pane receives the input

### Requirement: A pane can be taken out of the set

While sync is on, a right click on a pane SHALL take it out of the set, and a
right click on it again SHALL put it back, whichever pane holds the cursor.

#### Scenario: Opt out

- **WHEN** sync is on and pane B is right-clicked
- **THEN** typing into A reaches C but not B, and B's border is no longer yellow

#### Scenario: Typing into an excluded pane

- **WHEN** B is excluded and is focused
- **THEN** typing reaches B alone

### Requirement: Keys are encoded per target pane

Each synced pane SHALL receive input encoded for its own keyboard and paste
state. herdr's own keys, mouse events, scrolling and focus reports SHALL NOT be
sent to the other panes.

#### Scenario: Prefix key

- **WHEN** the prefix chord is typed with sync on
- **THEN** herdr handles it and no pane receives it

### Requirement: Sync is visible

While sync is on, the borders of the synced panes and the bottom bar SHALL be
bright yellow, and a pane outside the set SHALL keep its normal border.

#### Scenario: Hint

- **WHEN** sync turns on for the tab on screen
- **THEN** every synced pane border and the bottom bar are bright yellow

### Requirement: Sync is shared, runtime-only state

Sync mode SHALL be server state shared by every attached client, available
through the JSON API and CLI, and SHALL NOT be saved in the session snapshot or
carried through a live handoff.

#### Scenario: Restart

- **WHEN** the server restarts or hands off with sync on
- **THEN** no tab is syncing afterwards
