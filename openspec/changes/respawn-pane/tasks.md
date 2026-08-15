Ordered so each group lands green on its own. Group 1 is the runtime primitive,
group 2 the gate, group 3 the surfaces that call it.

## 1. Generalise the respawn primitive

- [ ] 1.1 Introduce a respawn target (`LaunchArgv` | `Shell`) and rename
      `respawn_shell_for_launch_pane` to `respawn_pane_runtime(pane_id, target)`
      (`src/app/api.rs`), reusing the existing cwd/size/launch-env/spawn body
- [ ] 1.2 `LaunchArgv` respawns `TerminalState.launch_argv` when present and
      falls back to the shell when absent; `Shell` keeps today's behaviour
- [ ] 1.3 Point the existing `RuntimeExitAction::RespawnShell` caller at
      `Shell` so the agent-exit path is byte-identical
- [ ] 1.4 Tests: a command pane respawns its argv; a shell pane respawns a
      shell; pane id, terminal id, label, and todos survive; agent runtime
      identity is cleared

## 2. The confirmation gate

- [ ] 2.1 Add `confirm_respawn_pane: Option<PaneId>` (`src/app/state.rs`),
      mutually exclusive with `confirm_close_pane`, and assert pane liveness for
      it in `assert_invariants_for_test`
- [ ] 2.2 `confirm_pane_respawn(ws_idx, pane_id)` (`src/app/actions.rs`)
      mirroring `confirm_pane_close_with_todos`: consume a pending token,
      otherwise prompt when `child_pid().is_some()` or outstanding todos exist
- [ ] 2.3 Dialog wording branch for the respawn token (`src/ui/dialogs.rs`) and
      the accept path re-issuing the respawn mutation (`src/app/input/modal.rs`)
- [ ] 2.4 Tests: live child prompts; exited child with no todos does not;
      todos prompt regardless; confirm respawns; cancel leaves the pane;
      the two tokens never coexist

## 3. Surfaces

- [ ] 3.1 `Method::PaneRespawn(PaneTarget)` (`src/api/schema.rs`) plus dispatch
      and `runtime_pane_respawn` (`src/app/runtime_mutations.rs`); confirm
      `PROTOCOL_VERSION` needs no bump since the change is additive
- [ ] 3.2 `herdr pane respawn <pane_id>` (`src/cli/pane.rs`, `src/cli/spec.rs`)
- [ ] 3.3 `KeysConfig.respawn_pane` defaulting to `prefix+ctrl+x`
      (`src/config/model.rs`, `keybinds.rs`), `NavigateAction::RespawnPane`
      (`src/app/input/navigate.rs`), `help_entry` in `src/ui/keybind_help.rs`,
      commented entry in the `src/main.rs` config template
- [ ] 3.4 Docs: keyboard page and the pane CLI reference under
      `docs/next/website/src/content/docs/`
- [ ] 3.5 `just check` green; dogfood on `-ac-beta` against a real wedged pane
