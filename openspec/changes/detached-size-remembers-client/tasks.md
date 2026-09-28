Planned 2026-09-28, not started: to be built in a batch that ac chooses.
Issue: https://gitea.cat-bluegill.ts.net/AC-forks/herdr/issues/102

## 1. Tests first (red)

- [ ] 1.1 headless: `detach_keeps_the_last_client_size` (310×56 attach → detach → effective size 310×56, no resize to 120×40)
- [ ] 1.2 headless: `last_client_resize_wins` (200×50 → resize 310×56 → detach → 310×56)
- [ ] 1.3 headless: `tiny_client_is_not_remembered` (310×56, then 60×20 attach/detach → 310×56)
- [ ] 1.4 headless: `background_client_size_is_ignored` (foreground 310×56, background 100×30)
- [ ] 1.5 headless: `handoff_size_beats_remembered_size` (handoff 200×60, remembered 310×56 → 200×60 until attach)
- [ ] 1.6 headless: `remember_client_size_false_keeps_todays_behaviour`
- [ ] 1.7 persist: `snapshot_round_trips_last_client_size`, `snapshot_without_last_client_size_loads`, `zero_last_client_size_is_ignored`
- [ ] 1.8 startup: `cold_start_spawns_restored_panes_at_remembered_size`
- [ ] 1.9 creation: split while detached still divides the target's real size (existing test, re-run at a remembered size)

## 2. Build (green)

- [ ] 2.1 `last_client_size` on the server + AppState; set from the foreground client with the 80×24 floor; mark the session dirty on change
- [ ] 2.2 `detached_size()` precedence: handoff, remembered, headless
- [ ] 2.3 `SessionSnapshot.last_client_size` (serde default); save and restore; seed before the first `detached_pane_size`
- [ ] 2.4 `server.remember_client_size` (default true), wired at startup and in live reload (`apply_live_config`); `--default-config` template entry
- [ ] 2.5 docs/next configuration.mdx (`[server]`), no CHANGELOG edit

## 3. Ship (with the batch)

- [ ] 3.1 `just check` green; `openspec validate detached-size-remembers-client --strict`
- [ ] 3.2 Beta; live proof on m4m: attach at full size, detach, `herdr-beta pane list` shows real widths; restart the server detached, panes come back at the remembered size; the nightly reads an unclipped footer
- [ ] 3.3 Comment the SHAs, beta and implementation notes on the issue; close it after the live check
