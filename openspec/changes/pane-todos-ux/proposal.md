# Pane todos: always-on affordance, add from the panel, and a real link picker

## Why

Dogfooding the shipped `pane-todos` feature surfaced three gaps, all in the same
area: getting a todo onto a pane, and pointing it at another pane.

**The affordance is invisible until it is already in use.** The indicator is
drawn only for panes that hold todos, which was a deliberate "a quiet pane is
unchanged" decision. In practice it means the one pane you want to add a todo to
— an empty one — offers nothing to click, so the feature is undiscoverable by
mouse and looks absent.

**The panel cannot add.** Its keys are selection, toggle, edit, follow, remove,
clear, close. There is no add. `keys.add_pane_todo` exists but ships unbound, so
out of the box there is no keyboard path to create a todo at all, and opening an
empty panel is a dead end.

**Linking is unusable and quietly wrong.** `ctrl+l` cycles `keep → clear → each
candidate → keep`, which does not scale past a handful of panes. Worse, the
candidate list is restricted to the todo's own workspace, so most panes in a real
session simply never appear — the reported symptom was "no idea why it cycles
only on some panes". Non-agent panes are technically offered but labelled
`pane 3`, so a shell running a command is unidentifiable and reads as missing.

The data layer already supports what is wanted here. Pane ids are unique across
the session, link resolution already searches every workspace, and restore
already anticipates a target "in another workspace entirely" and remaps through a
restore-wide id map. Only the candidate list and the picking interaction are
narrow — plus one latent bug that would silently drop any cross-workspace link on
save.

## What Changes

- The pane todo indicator is drawn on **every** pane, in three visually distinct
  states: outstanding (count, priority colour), all done (glyph, normal dim), and
  empty (glyph, dimmest). Clicking an empty pane's indicator opens its panel.
- The panel gains an **add** action, offered even when the pane holds no todos,
  with a footer button alongside the existing ones.
- Choosing a link **opens the session navigator in a selection mode** instead of
  cycling: every pane in every workspace, searchable and filterable, labelled the
  way the navigator labels panes — so a shell is named by its launched command.
  Non-pane rows are context only, the todo's own pane is excluded, and an
  explicit entry clears the link.
- Link targets are resolved to their public identifier **in the target's own
  workspace** rather than the active one, which is what makes cross-workspace
  links actually persist.
- `cycle_pane_todo_edit_link` and `pane_link_candidates` are removed; the picker
  is the single mechanism.

## Impact

- Affected specs: `pane-todos` (three requirements modified: indicator, links,
  panel and editing).
- Affected code: `src/ui/panes.rs` (indicator states), `src/ui/todo_panel.rs`
  (add affordance, empty-state footer), `src/app/input/modal.rs` (panel add key,
  link control opens the picker), `src/app/state.rs` (navigator selection
  purpose, removal of the cycling helpers), `src/app/actions.rs` +
  `src/ui/navigator.rs` (selection mode), `src/app/ids.rs` (session-wide public
  pane id lookup), `src/ui/keybind_help.rs`.
- No server, API, protocol, or persistence changes: `todo.update` already carries
  `link_pane_id`, and the stored link shape is unchanged. This is TUI
  presentation and input only, per the runtime/client boundary guardrail.
- Behaviour change for existing users: every pane's title loses the columns the
  indicator reserves, and `ctrl+l` no longer cycles. Accepted deliberately —
  uniform placement of the affordance was the goal.
