Planned 2026-09-29 (JOB 14 from herdr-helper; ac: "solve it thoroughly and add
to current build"). Issues:
https://gitea.cat-bluegill.ts.net/AC-forks/herdr/issues/123 (D),
https://gitea.cat-bluegill.ts.net/AC-forks/herdr/issues/127 (E).

## 0. Probe

- [ ] 0.1 throwaway session: hand-started stand-in codex with the Codex hook; log whether the SessionStart report arrives and why it is refused

## 1. Tests first (red)

- [ ] 1.1 carry tables (claude, codex): kept / dropped flags, `=` forms, relative paths
- [ ] 1.2 composer: claude hook argv + launch flags, codex built-in + flags, daemon codex keeps `--remote`
- [ ] 1.3 guards: agent change drops the old session; nested codex keeps claude's; snapshot skips a mismatch; codex report replaces a claude session
- [ ] 1.4 persistence: old session.json loads, new field round-trips; restore uses the composed command per kind
- [ ] 1.5 `restore_argv` in pane list/get equals the restore plan

## 2. Build (green)

- [ ] 2.1 launch record: argv read on agent change, carry filter, state + snapshot field
- [ ] 2.2 composer in `src/agent_resume.rs`, used by restore and by `restore_argv`
- [ ] 2.3 guards in `src/terminal/state.rs` + snapshot; refusal logging
- [ ] 2.4 codex id from `resume <id>` argv when no report
- [ ] 2.5 schema field, regenerate `docs/next/api/herdr-api.schema.json`

## 3. Docs

- [ ] 3.1 integrations.mdx: what a restore keeps, per agent
- [ ] 3.2 socket-api.mdx / cli-reference.mdx: `restore_argv`

## 4. Verify and ship

- [ ] 4.1 proof: one pane per kind, server stop + restart, restored argv read back with `ps`
- [ ] 4.2 `just check`, push origin and internal, notes on #123 and #127
