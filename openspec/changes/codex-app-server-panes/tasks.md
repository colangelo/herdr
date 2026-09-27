## 1. Layer A

- [ ] 1.1 `[agents.codex]` config with `app_server`, `app_server_socket`, `name_threads`
- [ ] 1.2 Launch arguments helper (`--remote unix://…`, `-C cwd`), caller arguments win
- [ ] 1.3 Apply to `agent start --kind codex` and to the deferred `codex resume`

## 2. Layer B

- [ ] 2.1 Minimal JSON-RPC-over-WebSocket client for a unix socket
- [ ] 2.2 Name jobs: resume by id, start by discovery, session report, rename; off the app loop
- [ ] 2.3 Skip ephemeral and sub-agent threads; ambiguous discovery names none

## 3. Tests and docs

- [ ] 3.1 Unit tests: argv for start and resume, caller precedence, off by default
- [ ] 3.2 Client tests against a fake WebSocket daemon on a unix socket
- [ ] 3.3 Discovery selection tests (one, none, ambiguous, ephemeral, sub-agent)
- [ ] 3.4 Config reference + configuration.mdx

## 4. Verification

- [ ] 4.1 `just check` green
- [ ] 4.2 Live, no Codex turn: thread in `thread/loaded/list`; `thread/read` shows pane cwd and name; `agent-bell who` lists it as codex; `agent-bell send` arrives queued
