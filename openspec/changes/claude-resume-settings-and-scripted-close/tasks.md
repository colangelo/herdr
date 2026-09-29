Planned 2026-09-29 (JOB 13 from herdr-helper; ac said "build").
Issues: https://gitea.cat-bluegill.ts.net/AC-forks/herdr/issues/123, /120,
/121; base picks in /124.

## 1. Tests first (red)

- [ ] 1.1 #121: manifest tests for `· 2 shells` at end of line and `· 1 shell`, plus the existing `· 2 shells · ← 1 agent` (`src/detect/manifest/tests.rs`)
- [ ] 1.2 #123 hook: a Python-level test harness in `src/integration/tests.rs` (or `tests/cli/hooks.rs`) that runs the real `herdr-agent-state.sh` against a fake socket with a fixture transcript and hook input, and checks the reported `resume_argv` for: auto + launched-with-bypass, bypass, plan, nothing known, `<synthetic>` model skipped, input mode beats transcript mode
- [ ] 1.3 #123 installer: install writes SessionStart, UserPromptSubmit and Stop entries; install on a canonical file is a byte-exact no-op; install migrates a v8 file (SessionStart only); uninstall removes all three and keeps user hooks
- [ ] 1.4 #123 restore: a Claude pane whose reported resume carries the flags restores with exactly that command (`src/persist/restore.rs`)
- [ ] 1.5 #120: an API `pane.close` with todos leaves client mode and tokens alone and lists the todos; `force` closes and returns `closed_todos`; `tab.close` and `workspace.close` refuse with todos and accept `force`; the TUI modal flow is unchanged; a ConfirmClose whose pane was closed leaves the mode (`src/app/api/panes.rs`, `tabs.rs`, `workspaces.rs`, `src/app/input/modal.rs`)
- [ ] 1.6 #120 CLI: `--force` parses for pane, tab and workspace close (`src/cli/`)

## 2. Build (green)

- [ ] 2.1 #121 manifest regex, version bump
- [ ] 2.2 #123 hook script (.sh), then the .ps1 counterpart without the process walk
- [ ] 2.3 #123 installer: list of canonical entries, removal table
- [ ] 2.4 #120 schema params, origin marking in `dispatch_runtime_mutation`, pure gates, forced result, orphan cleanup
- [ ] 2.5 #120 CLI flags and help text

## 3. Docs

- [ ] 3.1 integrations.mdx: what the Claude hook saves and its limits
- [ ] 3.2 cli-reference.mdx and socket-api.mdx: `--force` / `force`, `closed_todos`
- [ ] 3.3 regenerate `docs/next/api/herdr-api.schema.json`

## 4. Ship

- [ ] 4.1 `just check`, push origin and internal
- [ ] 4.2 one beta; throwaway proof on m4m (stand-in claude, no paid tokens): restore types the reported command; API close with todos shows no modal on an attached client
- [ ] 4.3 comments on #120, #121, #123, #124; report to herdr-helper
