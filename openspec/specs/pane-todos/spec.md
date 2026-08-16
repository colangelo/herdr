# pane-todos Specification

## Purpose
Define the per-pane todo list: the store and its limits, display ordering,
persistence with the pane across restart and handoff, links to another pane,
the socket API, event, and CLI that expose the same state to external clients,
and the TUI surfaces — the pane indicator, the panel, and the editor including
its text composition keys.

## Requirements

### Requirement: Pane-scoped todo store

The server SHALL keep an ordered list of todos per pane. Each todo SHALL carry a
per-pane monotonic id, text, a `done` flag, a priority of high, normal, or low,
an optional link to another pane, and created/updated timestamps. The list SHALL
live in server state as pure data, constructible and testable without a PTY.

The server SHALL reject a todo whose text is empty or exceeds 500 characters,
and SHALL reject an addition that would take a pane beyond 50 todos, returning
an explicit error in each case rather than truncating or silently discarding.

Todo ids SHALL NOT be reused within a pane, so a stale id refers to nothing
rather than to a different todo.

#### Scenario: A todo is added to a pane

- **WHEN** a todo is added to a pane with text and no explicit priority
- **THEN** it is appended with a fresh per-pane id, `done` false, and normal
  priority
- **AND** its created and updated timestamps are set

#### Scenario: Text limits are enforced

- **WHEN** a todo is added with empty text
- **THEN** the call fails with `todo_text_empty` and the pane's list is unchanged
- **WHEN** a todo is added with text longer than 500 characters
- **THEN** the call fails with `todo_text_too_long` and the pane's list is
  unchanged

#### Scenario: The per-pane cap is enforced

- **WHEN** a pane already holds 50 todos and another is added
- **THEN** the call fails with `todo_limit_reached` and no todo is evicted

#### Scenario: Ids are not reused

- **WHEN** a todo is removed and a new one is added to the same pane
- **THEN** the new todo receives an id greater than every id previously issued
  for that pane

### Requirement: Todo display ordering

Stored order SHALL be insertion order. Presentation order SHALL be not-done
before done, then priority descending, then creation order. Changing a todo's
priority SHALL NOT change its id or its stored position.

#### Scenario: Priority orders the list

- **WHEN** a pane holds a normal todo added first and a high todo added second
- **THEN** the high todo is presented first

#### Scenario: Done todos sink

- **WHEN** a high-priority todo is marked done and a normal-priority todo is not
- **THEN** the not-done normal todo is presented before the done high one

### Requirement: Todos persist with their pane

Todos SHALL be written into the pane's session snapshot alongside its label and
agent name, so they survive server restart and live handoff. The snapshot field
SHALL default to empty when absent, so session files written before this feature
load unchanged, and SHALL be omitted entirely for panes with no todos.

Closing a pane that still has outstanding todos SHALL ask for confirmation
before the pane is destroyed.

#### Scenario: Todos survive a restart

- **WHEN** a pane with todos is saved and the session is restored
- **THEN** the pane's todos are present with their text, done state, priority,
  and ids intact

#### Scenario: Older session files still load

- **WHEN** a session file written without a todos field is restored
- **THEN** restore succeeds and the pane has an empty todo list

#### Scenario: Closing a pane with outstanding todos is confirmed

- **WHEN** a pane with at least one not-done todo is closed
- **THEN** a confirmation is requested before the pane is destroyed
- **WHEN** every todo on the pane is done
- **THEN** the pane closes without additional confirmation

### Requirement: Cross-pane todo links

A todo MAY link to any other pane in the session, including panes in other
workspaces and panes running no agent. The link SHALL store the target's
internal pane identity plus a label captured when the link was created.

The captured label SHALL name the target rather than merely locate it: a
manually labelled pane by its label, an agent pane by its agent, and a pane
running a plain shell by the command it launched. Only a target offering none
of these SHALL fall back to its identifier. Because the label is captured once
and read back long afterwards, it SHALL prefer those stable names over the
pane's live terminal title, which the session navigator leads with but which
drifts underneath a stored link.

Because pane identity is unique across the session while public pane
identifiers are scoped to a workspace, resolving a link target to its public
identifier SHALL locate the target's own workspace rather than assuming the
active one.

On restore, the target SHALL be remapped through the same identifier map that
remaps layout references. A link whose target cannot be resolved SHALL be
preserved as a dead link that retains its label, is presented as inert, and
SHALL NOT resolve to any other pane.

#### Scenario: A link survives restore

- **WHEN** a todo links to another pane and the session is restored
- **THEN** the link resolves to the same pane it referred to before the restart

#### Scenario: A link crosses workspaces

- **WHEN** a todo links to a pane in a different workspace and is saved
- **THEN** the link is stored rather than discarded
- **AND** following it moves focus to that pane in its own workspace
- **AND** it still resolves after the session is restored

#### Scenario: A shell pane is a valid target

- **WHEN** a link targets a pane running no agent
- **THEN** the link is created and its captured label names the launched command
- **WHEN** that same pane also carries a manual label or an agent
- **THEN** those name it instead

#### Scenario: A target with no name keeps its identifier

- **WHEN** a link targets a pane with no manual label, no agent, and no
  launched command on record
- **THEN** its captured label is the pane's own identifier

#### Scenario: A live link is addressed before it is named

- **WHEN** a todo carries a live link to a pane
- **THEN** the link is presented with that pane's public identifier first and its
  captured label after it

#### Scenario: A moved target is addressed by where it is now

- **WHEN** a linked pane's public identifier changes and the todo is presented
  again
- **THEN** the presented identifier is the target's current one

#### Scenario: A link to a closed pane goes dead

- **WHEN** a todo's link target no longer exists
- **THEN** the todo is retained, its link is presented as dead with its captured
  label, and activating it does not change focus

#### Scenario: Ambiguous link targets are rejected

- **WHEN** a link is requested by an agent name matching more than one live agent
- **THEN** the call fails with `todo_link_unresolved` and no todo is created or
  modified

### Requirement: Todo socket API and event

The server SHALL expose `todo.list`, `todo.add`, `todo.update`, `todo.remove`,
and `todo.clear`. `todo.list` SHALL return the todos of one pane when given a
pane, and every pane's todos when not, each entry carrying its owning pane. Each
mutating call SHALL emit a `todo.changed` event carrying the affected pane on the
existing subscription stream, so external consumers can follow the same state.

#### Scenario: Listing every pane's todos

- **WHEN** `todo.list` is called without a pane
- **THEN** todos from all panes are returned, each identifying its owning pane

#### Scenario: Mutations are evented

- **WHEN** a todo is added, updated, removed, or cleared
- **THEN** a `todo.changed` event naming the affected pane is emitted to
  subscribers

#### Scenario: Unknown targets are reported

- **WHEN** a call names a pane that does not exist
- **THEN** it fails with `pane_not_found`
- **WHEN** a call names a todo id that does not exist on the pane
- **THEN** it fails with `todo_not_found`

### Requirement: Todo CLI

The CLI SHALL provide `herdr todo` verbs to add, list, complete, reopen, edit,
remove, and clear todos. The target pane SHALL be resolved from an explicit pane
argument, a current-pane flag, or the calling pane's environment, matching the
resolution the existing pane verbs use. With no target given, commands SHALL act
on the calling pane, so an agent can record its own next steps without knowing
its pane id.

#### Scenario: An agent records its own next steps

- **WHEN** a process running inside a pane runs the add verb with only text
- **THEN** the todo is created on that pane

#### Scenario: Another pane is targeted explicitly

- **WHEN** the add verb is given an explicit pane target
- **THEN** the todo is created on that pane rather than the calling one

#### Scenario: Machine-readable output

- **WHEN** the list verb is run with the JSON flag
- **THEN** it prints the todos as JSON including ids, done state, priority, and
  link target

### Requirement: Pane todo panel and editing

Activating the indicator, by click or by a bindable action, SHALL open a panel
anchored to that pane listing its todos in presentation order, following the
existing overlay language. The panel SHALL support moving the selection,
toggling done, removing a todo, clearing done todos, opening a todo for editing,
adding a new todo, following a todo's link, and closing.

The panel SHALL offer the add action even when the pane holds no todos, so
opening a quiet pane's panel is never a dead end.

Opening a todo for editing SHALL present a modal built from the existing dialog
structure allowing its text, priority, link, and done state to be changed, with
explicit save and cancel. Because the modal's text field owns the panel's
done-toggle key, the modal SHALL offer the toggle under a distinct binding. A
todo being composed SHALL NOT offer the done toggle, since it cannot be already
done.

Choosing a link SHALL present the session navigator in a selection mode rather
than cycling through candidates, so the target can be searched and filtered.
Rows that are not panes SHALL be context only and SHALL NOT be selectable as a
target, the todo's own pane SHALL NOT be offered, and the selection SHALL
include an explicit entry that clears the link. Leaving the selection without
choosing SHALL leave the link as it was.

Following a link SHALL move focus to the linked pane through the same focus path
used when jumping to a notification's pane.

Every action introduced SHALL appear in the keybinding help panel, including any
left unbound by default.

#### Scenario: Opening the panel

- **WHEN** the indicator is clicked or the open action is invoked on a pane with
  todos
- **THEN** a panel anchored to that pane lists its todos in presentation order

#### Scenario: Adding a todo from the panel

- **WHEN** the add action is invoked from the panel
- **THEN** the edit modal opens on a new todo for that pane
- **AND** saving returns to the panel with the new todo listed

#### Scenario: An empty panel can still add

- **WHEN** the panel is opened on a pane with no todos
- **THEN** the add action is offered

#### Scenario: A multi-line todo occupies one panel row

- **WHEN** the panel lists a todo whose text holds more than one line
- **THEN** it occupies a single row showing the first line with a marker

#### Scenario: Editing a todo

- **WHEN** a todo is opened for editing, its text changed, and the change saved
- **THEN** the todo's text and updated timestamp change while its id, done state,
  and creation timestamp are preserved

#### Scenario: Commit and done-toggle stay clear of the editing set

- **WHEN** a key belonging to the text field's editing set is pressed in the edit
  modal
- **THEN** it edits the text and neither commits the edit nor toggles done

#### Scenario: Toggling done from the edit modal

- **WHEN** a todo is opened for editing and the done toggle is activated and the
  change saved
- **THEN** the todo's done state flips while its text, id, and creation timestamp
  are preserved
- **WHEN** the edit is cancelled instead
- **THEN** the todo's done state is unchanged

#### Scenario: Choosing a link target

- **WHEN** the link control is activated in the edit modal
- **THEN** the navigator opens in selection mode listing panes across every
  workspace
- **WHEN** a pane row is chosen
- **THEN** the edit returns with that pane staged as the link target

#### Scenario: Pane rows carry the identifier they stage

- **WHEN** the picker lists a pane row
- **THEN** the row shows that pane's public identifier as well as its name

#### Scenario: The picker moves without arrow keys while searching

- **WHEN** the picker's search is focused and the move-down chord is pressed
- **THEN** the selection moves down and the search text is unchanged
- **WHEN** the same chord is pressed with the search not focused
- **THEN** the selection moves down as well

#### Scenario: Non-pane rows are not targets

- **WHEN** a workspace row is activated in selection mode
- **THEN** it expands or collapses, no link target is chosen, and the
  selection stays open
- **WHEN** a tab row is activated instead
- **THEN** nothing happens, since only workspaces carry expansion state

#### Scenario: Leaving selection keeps the previous link

- **WHEN** selection mode is dismissed without choosing
- **THEN** the todo's link is whatever it was before the selection opened

#### Scenario: Following a link

- **WHEN** a todo with a live link is followed
- **THEN** focus moves to the linked pane

#### Scenario: Actions are discoverable

- **WHEN** the keybinding help panel is shown
- **THEN** every action introduced by this feature is listed, showing `unset` for
  any that has no binding

### Requirement: Todo text composition

Composing a todo's text SHALL use a field with an explicit insertion point.
Typed and pasted text SHALL be inserted at the insertion point rather than
appended, and deletions SHALL act relative to it.

The field SHALL support, at minimum: moving the insertion point by character and
by word in both directions and to the start and end of the text; deleting the
character before and the character after it; killing to the start and to the end
of the text; killing the word before it; yanking the most recent kill back at the
insertion point; and undoing recent edits. Motions and kills SHALL be reachable
without the arrow keys, using the conventional readline chords, while the arrow
keys SHALL continue to work.

Undo SHALL be bound to a chord that a terminal without the enhanced keyboard
protocol can still deliver, and MAY additionally accept an enhanced-protocol-only
chord. No editing capability SHALL be reachable *only* through a chord that
requires the enhanced keyboard protocol.

Todo text MAY contain newlines. Where the field accepts newlines, the key that
inserts a newline SHALL NOT also commit the edit: committing SHALL have its own
distinct key, and cancelling SHALL remain unchanged. Pasted text SHALL retain
its newlines and SHALL continue to drop other control characters.

The field SHALL enforce the store's text limit as text is composed, so the modal
cannot compose a todo the server will reject.

#### Scenario: Text is inserted at the insertion point

- **WHEN** the insertion point is moved to the start of the text and a character
  is typed
- **THEN** the character appears at the start and the rest of the text is
  preserved

#### Scenario: Readline motions and kills

- **WHEN** the insertion point is moved to the end of the text and kill-to-start
  is invoked
- **THEN** the text is emptied
- **WHEN** yank is invoked
- **THEN** the killed text is restored at the insertion point

#### Scenario: Deleting forward and backward

- **WHEN** the insertion point sits between two characters and delete-forward is
  invoked
- **THEN** the character after it is removed and the insertion point does not
  move
- **WHEN** delete-backward is invoked instead
- **THEN** the character before it is removed and the insertion point moves back

#### Scenario: Undo restores the previous text

- **WHEN** text is typed and then killed, and undo is invoked
- **THEN** the text before the kill is restored

#### Scenario: Undo does not require the enhanced keyboard protocol

- **WHEN** the terminal reports keys without the enhanced keyboard protocol
- **THEN** undo is still reachable by a chord that terminal can deliver

#### Scenario: A todo can hold more than one line

- **WHEN** the newline key is pressed while composing and text is typed after it
- **THEN** the todo's text holds both lines
- **AND** the edit is not committed by that keypress
- **WHEN** the commit key is pressed
- **THEN** the todo is saved with both lines and survives a session restore

#### Scenario: Paste keeps newlines and drops other control characters

- **WHEN** text containing a newline and an escape character is pasted into the
  field
- **THEN** the newline is kept and the escape character is dropped

#### Scenario: The store's limit is enforced while composing

- **WHEN** the field already holds the maximum number of characters
- **THEN** further typed characters are ignored rather than composing a todo the
  store would reject

### Requirement: Always-on pane todo indicator

Every pane that draws a top border SHALL show a todo indicator at the far right
of it, whether or not the pane holds todos, so the affordance is in the same
place on every such pane and a pane with no todos can still be opened by mouse.
A pane that draws no top border SHALL show no indicator, since there is no
chrome to carry it; the bindable action remains the path there.

The indicator SHALL distinguish three states: a pane with outstanding todos
carries their count, a pane whose todos are all done carries the glyph without a
count, and a pane with no todos carries the glyph rendered in the dimmest tone
so it reads as an empty affordance rather than as completed work.

The indicator SHALL be colored by the highest outstanding priority when
outstanding todos exist, and SHALL be configurable off, suppressing all three
states. The region used to draw the indicator and the region that accepts a
click SHALL be derived from a single shared definition. When pane width forces a
choice the indicator SHALL win: it is laid out first and the title takes what is
left, dropping itself when that is too narrow. The indicator SHALL be omitted
only when the pane cannot carry the glyph and its enclosing border corners at
all.

#### Scenario: Outstanding count is shown

- **WHEN** a pane holds three not-done todos and one done todo
- **THEN** its indicator shows a count of three

#### Scenario: An empty pane still offers the affordance

- **WHEN** a pane with a top border has no todos
- **THEN** the indicator is drawn without a count in the dimmest tone
- **AND** activating it opens that pane's todo panel

#### Scenario: A pane without a top border carries no indicator

- **WHEN** a pane draws no top border
- **THEN** no indicator is drawn for it regardless of its todos

#### Scenario: The indicator outlives the title in a squeeze

- **WHEN** a pane is too narrow to show both the indicator and its title
- **THEN** the indicator is drawn and the title gives up the space
- **WHEN** the pane cannot carry the glyph and its border corners at all
- **THEN** the indicator is omitted

#### Scenario: Empty is distinguishable from all-done

- **WHEN** one pane has no todos and another pane's todos are all done
- **THEN** both show the glyph without a count, rendered in different tones

#### Scenario: The indicator can be turned off

- **WHEN** the pane todo indicator is configured off
- **THEN** no indicator is drawn for any pane, including panes holding todos

#### Scenario: Click target matches what is drawn

- **WHEN** the indicator is drawn for a pane
- **THEN** the cells that respond to a click are exactly the cells drawn
