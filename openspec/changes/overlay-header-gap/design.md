# Design

## The header block mirrors the footer block

`FOOTER_ROWS = 2` is the button row plus the blank row above it, and it lives in
the kit rather than at each call site because the panels that sized their
content as "everything but the footer" each lost the gap that way.
`HEADER_ROWS = 2` is the same shape at the other end: the title row plus the
blank row under it.

`header_split(inner)` serves the overlays that place rows by offset from
`inner` — the board, the pane-move picker, the open-worktree dialog. The ones
built from an explicit `Layout::vertical` spend the same rows as a
`Constraint::Length(1)` spacer instead, because threading a helper through a
constraint list would obscure rather than share.

The pane-move picker's header is two lines — a title and a subtitle — so it
gets `PANE_MOVE_TARGET_HEADER_ROWS = HEADER_ROWS + 1`, named rather than
open-coded because its renderer and its mouse hit-test both consume it and must
not drift.

## Why the heading and the link chip lead with different things

The heading becomes `infra · imap-jmap-mcp [w2:pP]`, while the todo link chip
stays `→ w2:pC · claude`. That looks inconsistent and is deliberate.

The chip shares a row with the todo's own text and is truncated from the right
when the row is narrow. Leading with the identifier is what keeps the part you
can act on — with `herdr pane`, `herdr agent read`, or a sibling agent's prompt
— from being the part that disappears. A heading owns a whole row and competes
with nothing, so it can afford to read in the order you think in: which space,
which agent, and only then the address.

The rule is therefore not "identifier first" but "the part that must survive
truncation goes first". In a chip that is the identifier; in a heading nothing
is at risk, so readability wins.

## Why the space, and why first

The board is read across spaces — that is its entire justification over the
per-pane panel. The heading previously named the space only as the first
character of `w2:pP`, which is an identifier to decode rather than a name to
recognise. The space name is what the sidebar already calls it, so the board
borrows that rather than inventing a second name for the same thing.

## Reversing the board's `clear done` scope

`todo-board` decided that `c` on the board should clear the *selected* todo's
pane, so the key would mean exactly what it means on that pane's own panel, and
so a session-wide sweep would not hide behind a familiar letter. Dogfooding
overturned it within a day.

The failure mode is that a scoped action with no visible scope is
indistinguishable from a broken one. The footer says `clear done`. Press it with
the selection parked on a pane whose todos are all outstanding — the common case,
since the selection opens on the first todo in the list — and nothing happens and
nothing is said. It was reported as "c does not work", which is the correct
reading of what it did.

So `c` now clears the completed todos of every pane the board is showing. The
destructiveness argument does not survive contact either: the todos being removed
are the ones already marked done, the board is showing exactly which, and the
per-pane panel still offers the narrower version for anyone who wants it.

The general rule this leaves behind: an action on a session-scoped surface takes
the scope of that surface, or it must show its scope. It cannot quietly take a
narrower one.

## Alternatives considered

**A blank row under every title, including the ones that already have one.**
Rejected as a non-change: the four conforming overlays already look right, and
touching them would re-record snapshots for no visual difference.

**Keeping the identifier first and appending the space.**
`w2:pP · infra · imap-jmap-mcp` keeps one ordering rule across chip and
heading, but leads with the least readable field in the one place there is room
to do better. The heading's job is triage, not addressing.
