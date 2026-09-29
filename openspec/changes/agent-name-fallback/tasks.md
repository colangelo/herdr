Planned 2026-09-29 (JOB 14; ac: "put in next beta" on
https://gitea.cat-bluegill.ts.net/AC-forks/herdr/issues/130 and
https://gitea.cat-bluegill.ts.net/AC-forks/herdr/issues/131).

## 1. Tests first (red)

- [ ] 1.1 name rule: names, generic titles, sentences, paths, shells, kinds
- [ ] 1.2 resolution: explicit wins, fallback unique, duplicates dropped, follows title
- [ ] 1.3 API/targets: list shows `name_source`, `agent read <fallback>` resolves, rename conflicts ignore fallbacks
- [ ] 1.4 notifications: named, fallback-named and unnamed titles

## 2. Build (green)

- [ ] 2.1 rule in `src/terminal/title.rs`, resolution on `AppState`
- [ ] 2.2 `AgentInfo.name_source`, list, targets, rename conflicts
- [ ] 2.3 notification titles (toast, notification center, forwarded message)

## 3. Verify

- [ ] 3.1 docs: agents.mdx, socket-api.mdx
- [ ] 3.2 `just check`, push, notes on #130 and #131
