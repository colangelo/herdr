# Design: sidebar-working-display-state

## Context

Three byte-identical copies of one ranking table currently serve two different jobs:

| Location | Function | Consumed by |
|---|---|---|
| `src/workspace/aggregate.rs:83` | `pane_attention_priority` | `Workspace::aggregate_state` → sidebar space row **display** |
| `src/ui/sidebar.rs:293` | `workspace_attention_priority` | `agent_panel_target_keys` → bubble-motion **target order** |
| `src/app/api_helpers.rs:1` | `tab_attention_priority` | `apply_agent_view` → agent panel **sort order** |

```rust
(Blocked, _)  => 4
(Idle, false) => 3   // done
(Working, _)  => 2
(Idle, true)  => 1
(Unknown, _)  => 0
```

Two jobs, one table. For *sorting by attention* the ranking is defensible: a finished agent
wants you more than a working one. For *choosing a displayed state* it is wrong — a space
that contains a working agent is working.

## Goals / Non-Goals

**Goals**
- A sidebar row containing a working agent renders as working unless something genuinely
  more urgent (blocked) is present.
- A row's rendered state is a pure function of its panes' states — never changed by
  focusing an unrelated sibling.
- One definition of each ranking, not three copies.

**Non-Goals**
- Redefining `seen`, attention semantics, or notification ordering.
- Per-pane agent rows (already correct).

## Decisions

### Decision 1: Two named rankings, one home

Replace the three copies with one module exposing both rankings explicitly:

```rust
/// Ranking for *what a row is*: what the user sees in the dot.
pub fn display_priority(state: AgentState, seen: bool) -> u8 {
    match (state, seen) {
        (Blocked, _)  => 4,
        (Working, _)  => 3,   // <-- above done
        (Idle, false) => 2,   // done
        (Idle, true)  => 1,
        (Unknown, _)  => 0,
    }
}

/// Ranking for *what wants you next*: sorting and notifications.
pub fn attention_priority(state: AgentState, seen: bool) -> u8 { /* unchanged table */ }
```

Only the relative order of `Working` and `Idle+unseen` differs. `Blocked` stays top in both
— a blocked agent genuinely outranks a working one for display too, because it is stalled
and cannot proceed.

**Why not one ranking?** Because the two questions have genuinely different answers.
Collapsing them would either bury working agents (today's bug) or claim a space needs
attention when its agents are merely busy.

**Alternative rejected:** making `aggregate_state` return "working if any pane is working,
else max-by-attention". That is `display_priority` written as a special case, but with the
precedence hidden in control flow instead of stated in a table.

### Decision 2: `aggregate_state` returns display state

`Workspace::aggregate_state` is the space-row source and switches to `display_priority`.
Its `(AgentState, bool)` signature is unchanged, so `state_dot` and
`agent_panel_status_key` need no changes.

Call sites must be audited: any caller using `aggregate_state` to decide *ordering* or
*notification* should move to an explicit attention-ranked helper rather than silently
inheriting display order.

### Decision 3: Sorting follows attention; motion follows the sort

`apply_agent_view` and the workspace priority sort keep `attention_priority` — a finished
agent should still surface above a working one in an attention-sorted list; that is the
feature's point.

The binding constraint is *consistency*: `agent_panel_target_keys` and
`workspace_unit_target_keys` feed bubble motion, and motion animates toward whatever order
the sort produced. They must use the same ranking as the sort they animate. Today they
happen to agree only because all three tables are identical; after the split that agreement
must be explicit, or the motion will chase a different target than the sort and never
settle.

This is the highest-risk part of the change and is why the tasks pin it with a test.

### Decision 4: Runtime/client boundary

Per the project guardrail, the *display* ranking is TUI presentation and belongs in the
client/UI layer. The *attention* ranking is a shared runtime fact already surfaced through
the API (`agent_status` distinguishes `done` from `idle`).

Keep the shared definition in a neutral location with neutral names — `display_priority` /
`attention_priority`, not `sidebar_priority` / `row_priority`. Any workspace- or tab-level
aggregate exposed through the JSON API must state which ranking it uses; do not change the
API's meaning silently as a side effect of a UI fix.

## Risks

| Risk | Mitigation |
|---|---|
| Motion target and sort order diverge → bubble motion never settles | Test that target keys and sorted order match for a mixed-state fixture |
| A caller of `aggregate_state` wanted attention order | Audit every call site as an explicit task; move ordering users to the attention helper |
| A space with `{blocked, working}` regresses | Scenario pins blocked still winning |
| Silent API meaning change | Non-goal + explicit task to confirm no API aggregate changes meaning |

## Verification

- `just check` (fmt + nextest + maintenance script tests).
- State-level tests via `AppState::test_new()` / `Workspace::test_new()`; no PTYs needed —
  this is pure state-to-display projection.
- Live confirmation: reproduce the issue-39 capture (a space holding one `done` and one
  `working` pane renders `working`, and stays `working` across a sibling focus switch).
