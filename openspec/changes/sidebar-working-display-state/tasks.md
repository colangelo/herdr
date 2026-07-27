# Tasks: sidebar-working-display-state

Tracked in https://gitea.cat-bluegill.ts.net/AC-forks/herdr/issues/39

## 1. Characterization tests (before behavior changes)

- [ ] 1.1 Pin today's behavior: `Workspace::aggregate_state` with `{done, working}` returns done — an intentionally-failing-later test documenting the bug
- [ ] 1.2 Pin `{blocked, working}` → blocked and `{done, idle-seen}` → done, which must survive the change
- [ ] 1.3 Pin the current attention sort order of `apply_agent_view` for a mixed-state fixture, so the sorting behavior is provably unchanged by this work
- [ ] 1.4 Assert the three ranking tables are currently identical, so collapsing them is provably behavior-preserving at the attention-ranking level

## 2. Shared ranking definitions

- [ ] 2.1 Add one shared module defining `display_priority` and `attention_priority` with neutral, non-UI names (per the runtime/client boundary guardrail)
- [ ] 2.2 `attention_priority` reproduces the existing table exactly (Blocked 4, Idle+unseen 3, Working 2, Idle+seen 1, Unknown 0)
- [ ] 2.3 `display_priority` orders Blocked 4, Working 3, Idle+unseen 2, Idle+seen 1, Unknown 0
- [ ] 2.4 Unit-test both rankings as total orders over all five (state, seen) combinations
- [ ] 2.5 Delete the three duplicate tables (`src/workspace/aggregate.rs:83`, `src/ui/sidebar.rs:293`, `src/app/api_helpers.rs:1`) and route every caller to the shared definitions

## 3. Display aggregate

- [ ] 3.1 Switch `Workspace::aggregate_state` to `display_priority`, keeping its `(AgentState, bool)` signature so `state_dot` and `agent_panel_status_key` are untouched
- [ ] 3.2 Audit every `aggregate_state` call site; any caller using it for ordering or notification moves to an explicit attention-ranked helper instead of inheriting display order
- [ ] 3.3 Flip test 1.1 to assert the corrected behavior (`{done, working}` → working)
- [ ] 3.4 Add the sibling-focus invariant test: rendered state identical before and after focusing a done sibling

## 4. Sort / motion consistency

- [ ] 4.1 Confirm `apply_agent_view` and the workspace priority sort use `attention_priority`; test 1.3 must still pass unchanged
- [ ] 4.2 Point `agent_panel_target_keys` and `workspace_unit_target_keys` at the same ranking as the sort they animate
- [ ] 4.3 Test that motion target order equals sorted order for a mixed-state fixture (the never-settles guard from design Risk 1)

## 5. API and boundary check

- [ ] 5.1 Confirm no JSON API field changes meaning: per-pane `agent_status` is untouched; if any workspace/tab-level aggregate is exposed, document which ranking it uses
- [ ] 5.2 Confirm `src/protocol/wire.rs::PROTOCOL_VERSION` needs no bump and no integration-asset version markers are touched

## 6. Verification

- [ ] 6.1 `just check` clean (fmt + nextest + maintenance script tests)
- [ ] 6.2 Live check against a build: a space holding one done and one working pane renders working, and stays working across a sibling focus switch — the issue-39 capture, inverted
- [ ] 6.3 Confirm no regression in sidebar priority sorting or bubble motion with `agent_panel_sort = "priority"`, `workspace_sort = "priority"`, `sort_motion_easing = "bubble"`

## 7. Docs

- [ ] 7.1 If the space-row state rule is user-visible enough to document, stage it under `docs/next/` — not the stable website docs
