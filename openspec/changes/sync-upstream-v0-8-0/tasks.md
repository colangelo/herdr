# Tasks — sync-upstream-v0-8-0

## 1. Assessment (done during proposal)

- [x] 1.1 Fetch upstream; measure divergence (112 up, 218 fork, base `d4e0dd3d`)
- [x] 1.2 Read v0.8.0 rationale; identify the three render optimizations
- [x] 1.3 Map dual-touched files (53) and heaviest overlaps
- [x] 1.4 Confirm timer deletion upstream and `ListMotion::next_due()` fit
- [x] 1.5 Detect protocol collision (fork 19 vs upstream 19)

## 2. Rebase (scratch branch, in the sync worktree)

- [ ] 2.1 `git branch -f reconcile-test master` and
      `git rebase --onto upstream/master d4e0dd3d reconcile-test`
- [ ] 2.2 Resolve infra conflicts per skill §2: `.gitignore`, `release.yml`,
      `ci.yml`, `justfile` (union of test modules), `tests/cli/sessions.rs`
- [ ] 2.3 Resolve `docs/next/CHANGELOG.md` per-heading; repair the
      auto-merge contamination (fork entries folded into upstream's
      released `[0.8.0]` section without conflicting): released `##`
      sections must end up byte-identical to upstream's, fork entries only
      under `## Unreleased`; run both duplicate checks (dup `##` headings;
      dup `###`/entries inside a section)
- [ ] 2.4 Resolve source conflicts toward upstream structure (design D2);
      during replay, drop fork hunks that arm/style the spinner
      (`next_animation_tick`, `sync_animation_timer*`,
      `agent_panel_has_animation`, spinner glyph styling)
- [ ] 2.5 Keep fork-scoped `website/latest.json` over upstream's org-transfer
      rewrite

## 3. Motion port (design D1)

- [ ] 3.1 Add `sort_motion_deadline` one-shot beside `toast_deadline` in app
      state; check it in `handle_scheduled_tasks` and
      `handle_scheduled_tasks_headless`; fold into
      `next_headless_loop_deadline`
- [ ] 3.2 Arm from `workspace_list_motion.next_due()` min
      `agent_panel_motion.next_due()`; re-sync at every target-order
      divergence site (state transitions, pane view/focus, config reload);
      `None` when settled or `ui.sort_motion = "instant"`
- [ ] 3.3 On fire: tick both motions, request render, re-arm
- [ ] 3.4 Tests: arming at each divergence site; zero wakes when settled;
      instant mode never arms; motion cadence follows settle/step timing
- [ ] 3.5 Delete any remaining `ANIMATION_INTERVAL`/`spinner_tick` residue

## 4. Glyph re-layer (design D2)

- [ ] 4.1 Verify editorial style, `[ui.state_colors]`, jump numbers,
      working-display-state each render correctly on upstream's static
      marks + distinct indicators
- [ ] 4.2 Point `[ui.state_colors]` fallbacks at upstream's new palette slots
- [ ] 4.3 Confirm each layer is a separable commit series on the rebased
      history (extractable as `upstream/master` + cherry-picks)

## 5. Protocol (design D3)

- [ ] 5.1 Bump `PROTOCOL_VERSION` to 20 in `src/protocol/wire.rs`
- [ ] 5.2 Update `tests/cli/sessions.rs` and any manual protocol fixtures

## 6. Verification

- [ ] 6.1 Fork-surface check per skill §4 (no upstream file deletions;
      release.yml fork hunks; `release-ac`; workflow YAML parses)
- [ ] 6.2 Handoff-resurface vs hidden-pane skip: prove a resurfaced working
      pane still renders; add/extend a characterization test if none covers it
- [ ] 6.2b Mouse-driven fork UI vs motion decoupling: verify pane todo
      panel and notification center hover/hit affordances still update;
      route through the hover-sensitive-zone path if they relied on
      passive-motion redraws
- [ ] 6.2c Sidebar aggregate collision (U21): fork behavior wins over
      upstream's `aggregate_state_done_unseen_beats_working` pinned test;
      adapt/replace it, keep the fork characterization tests green
- [ ] 6.3 `just check` with `cargo nextest run --locked --no-fail-fast`;
      upstream-baseline any failure before attributing it to the rebase
- [ ] 6.4 Re-run both changelog duplicate checks

## 7. Adopt and post-sync (skill §5–§6)

- [ ] 7.1 Fast-forward `master` to the reconciled branch; force-push with
      lease to `origin` and `internal`
- [ ] 7.2 Disable any new upstream bot workflows on `colangelo/herdr`
      (watch for renames after the `herdrdev` org transfer)
- [ ] 7.3 CI green on pushed master; `just latest-json-check`
- [ ] 7.4 Drift check: upstream changes to `justfile` release recipes,
      `scripts/changelog.py`, `release.yml` → update herdr-release skill and
      `release-ac` if the flow moved; update the CONTEXT PROJECTS entry
- [ ] 7.5 Remove the task worktree and branch after integration

## 8. Contribution-effort handoff (ticket #50 resolved-when; do not re-run research)

- [ ] 8.1 Record the post-rebase unit→SHA mapping for the 30-unit
      inventory (`internal/research/upstream-unit-inventory`), keyed off
      the `wayfinder/pre-v0.8.0-rebase` tag; keep the tag until re-mapped
- [ ] 8.2 Close https://gitea.cat-bluegill.ts.net/AC-forks/herdr/issues/50
      with the mapping; re-validation of the contribution findings resumes
      as ticket #51 (see the resume brief on
      `internal/research/upstream-proposal-pack-resume-brief`)
