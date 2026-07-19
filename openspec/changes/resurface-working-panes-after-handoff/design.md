# Design: Resurface Working Agent Panes After Live-Handoff Restart

## Context

A herdr update/protocol restart is a live handoff: the old server duplicates each pane's PTY master fd and passes it to the new server over a unix socket (SCM_RIGHTS, `src/server/handoff.rs`), so agent processes never die. Detection already runs for every pane on a background timer with no focus gating. The post-restart "everything looks idle" symptom is caused by three interacting facts:

1. Restore seeds every pane's agent state as `Idle`, unconditionally (`src/persist/restore.rs:629-638` for imported panes), even when the agent was mid-task. The handoff manifest (`HandoffRuntimeState`) carries fds, pids, and terminal metadata but not the live `AgentState`.
2. The re-adopted agent TUI does not repaint into the new server's ghostty grid on its own, so `detection_text()` (bottom rows of the grid) stays stale.
3. The idle-scan throttle (`should_skip_idle_screen_scan`, `src/pane/agent_detection.rs:91-138`) skips re-scanning an Idle pane until `detection_content_seq` advances — which only happens on new PTY bytes or a `PaneRuntime::resize`.

The only existing recovery is `nudge_handoff_panes_on_first_client_attach` (`src/server/headless.rs:1326`): a one-shot, all-panes-at-once SIGWINCH nudge deferred until the first client attaches. Empirically it is not sufficient; the reliable trigger is the real geometry change when the user views/clicks each pane.

Direction was decided in Gitea issue https://gitea.cat-bluegill.ts.net/AC-forks/herdr/issues/17: seed prior state + verify.

## Goals / Non-Goals

**Goals:**

- After a live handoff, every pane whose agent was Working/Blocked before the restart shows that state in the sidebar immediately, across all workspaces, without any user interaction.
- A server-side background sweep re-verifies each restored agent pane within a few seconds, correcting panes whose agents finished during the restart.
- The sweep runs whether or not a TUI client is attached.

**Non-Goals:**

- Cold restore (`resume_agents_on_restore`, dead processes relaunched): freshly relaunched agents repaint on their own and must not be seeded as Working. No sweep needed there.
- Changing detection cadence, throttling semantics, or manifests.
- Windows: live handoff is unix-only; all new code is `#[cfg(unix)]`-gated alongside the existing handoff paths.
- Client-driven pane visiting (rejected: violates the runtime/client boundary guardrail — this is a shared runtime fact and belongs server-side).

## Decisions

### 1. Carry live `AgentState` in the handoff runtime manifest, not the persisted snapshot

The pre-restart state travels per-pane in `HandoffRuntimeState` (`src/handoff_runtime.rs`) as an optional field with a serde default of absent → seed `Idle` (today's behavior). Rationale:

- The handoff manifest exists only for live handoff, which is exactly the scope where seeding a non-Idle state is correct. Putting it in the persisted snapshot (`src/persist/snapshot.rs`) would leak Working states into cold restore, where they would be wrong.
- An additive optional field keeps old-server → new-server handoffs working during the upgrade itself (the very first handoff after this change is sent by an old server that doesn't know the field).

Restore (`src/persist/restore.rs`) seeds the imported pane's terminal with the manifest state instead of hardcoded `Idle`; non-imported panes keep today's `Idle` seeding.

### 2. Server-side ordered sweep after handoff commit (approach A)

After the new server commits the handoff and un-quiesces the PTY actors (`src/server/headless.rs` around `assume_handoff_ownership`/`unpause_handoff_readers`), spawn one background tokio task that iterates all imported agent panes in workspace/tab/pane order. Per pane:

1. SIGWINCH repaint nudge — reuse the existing shrink/restore mechanism (`nudge_child_redraw_after_handoff`, `src/pty/actor/unix.rs:695-739`).
2. Forced detection rescan — bump the pane's detection content seq (`mark_detection_content_changed`) and wake the detection task (`detect_reset` notify), defeating the idle-scan skip so the detector reads the freshly repainted grid.
3. Stagger ~150ms before the next pane to avoid a thundering herd of full-TUI repaints.

Alternatives considered:

- (B) Fixing the existing first-attach nudge: still does nothing until a client attaches, and keeps the all-at-once repaint burst. Rejected.
- Seeding alone without a sweep: leaves stale Working spinners for agents that finished during the restart, potentially forever (no new PTY bytes → idle-skip never lifts). The sweep is what makes seeding safe.

### 3. The sweep replaces the deferred first-attach nudge

`pending_handoff_repaint_nudge` / `nudge_handoff_panes_on_first_client_attach` is subsumed: the sweep does strictly more (per-pane nudge + forced rescan, not gated on attach). Remove the deferred mechanism rather than leaving two overlapping nudge paths; the first-attach viewport resize (`resize_shared_runtime_to_effective_size`) is untouched and continues to serve attaching clients. The sweep must be idempotent per pane and harmless if the user focuses a pane mid-sweep (a nudge on an already-repainted pane is a no-op-sized resize wiggle; a forced rescan on a fresh grid just confirms the current state).

### 4. Hook-authority panes are covered by the same path

`full_lifecycle_authority_active` is runtime-only (referenced in `src/pane.rs`, `src/terminal/state.rs`, `src/terminal/runtime.rs`, `src/app/api.rs` — not in persist), so it does not survive handoff. Post-restart, screen detection is active for Claude Code panes until their hooks re-assert authority via `AppEvent::HookStateReported`. Therefore the seeded state gives the immediate spinner and the sweep's screen rescan verifies it, same as any other agent. Verify this assumption during implementation; if authority turns out to be re-established before the sweep reaches a pane, the sweep must skip screen-forcing for that pane (authority suppression already handles this inside the detection loop).

## Risks / Trade-offs

- [Stale spinner window] A pane whose agent finished during the restart shows Working until the sweep verifies it → bounded by sweep latency (~150ms × pane index); accepted in the direction decision.
- [Nudge repaint is best-effort] Some agent TUIs may not fully repaint their status line on SIGWINCH → the forced rescan then reads an unchanged grid and keeps the seeded state; the next real PTY output corrects it. No worse than today, and the seeded state is usually right.
- [Old-server manifests] The first handoff into this version carries no state field → seeds Idle (today's behavior) but the sweep still runs, so those panes recover via rescan instead of seeding. Degraded gracefully.
- [Repaint burst on slow machines] Even staggered, dozens of agent TUIs repainting can spike CPU → stagger interval is a named constant; sequential ordering (one pane at a time) already serializes the work.
- [Refactor risk surfaces] This touches handoff/restore (persisted-ish state) and detection — classified refactor-risk per project rules → characterization tests for manifest roundtrip and restore seeding before changing behavior.

## Migration Plan

1. Additive optional manifest field with serde default; old → new handoff degrades to today's Idle seeding plus sweep recovery.
2. Check `src/protocol/wire.rs::PROTOCOL_VERSION` against the latest released tag per project convention; the handoff manifest is a separate channel from the client wire protocol, so a bump is expected to be unnecessary — verify.
3. No integration-asset version markers are expected to change (no `HERDR_INTEGRATION_VERSION` assets touched); re-check at implementation time.
4. Rollback: revert the commit; the manifest field is ignored by older builds.

## Open Questions

- Exact insertion point for the sweep task relative to `unpause_handoff_readers()` and restore completion — must run after actors are `Running` and app state (workspace/tab/pane order) is available.
- Whether `mark_detection_content_changed` + `detect_reset` are reachable from the server/headless layer with current visibility, or need a small `PaneRuntime` method (e.g. `force_detection_rescan()`).
