Ordered so each group lands green on its own. Group 1 is the platform primitive
with no behaviour change; group 2 makes wrapped panes identify; group 3 corrects
the facts a wrapped pane reports.

There is a live reproduction on this machine — a Claude session behind
`atuin pty-proxy` — so group 5 can verify against a real wrapped pane rather
than only a constructed one.

## 1. Nested job lookup

- [x] 1.1 Add `nested_foreground_job(pid) -> Option<ForegroundJob>` to `src/platform/mod.rs`, documented as "the foreground job of the PTY this process owns", returning `None` when the process owns no PTY
- [x] 1.2 Implement it on macOS by finding children via `proc_bsdinfo`'s `pbi_ppid`, keeping only a child whose controlling terminal differs from the parent's, and delegating to the existing `foreground_job`
- [x] 1.3 Implement it on Linux over `/proc`, with the same different-controlling-terminal condition
- [x] 1.4 Stub it on Windows returning `None`, with a comment recording that ConPTY wrappers do not present this shape
- [x] 1.5 Tests: a child on the same controlling terminal is not followed; a process with no children returns `None`. Keep these to the testable contract in `src/platform/mod.rs` rather than asserting against live process tables

## 2. Identification descends into a recognised wrapper

- [x] 2.1 Add the recognised-wrapper set to `src/detect/mod.rs` as data beside the agent names, seeded with `atuin` and a comment saying why it is there
- [x] 2.2 In `probe_foreground_process_from_jobs` (`src/pane.rs`), when the foreground job yields no agent and its process group leader is a recognised wrapper, identify from `nested_foreground_job` of that leader
- [x] 2.3 Hold the descent to one level and to the process group leader only
- [x] 2.4 Tests on constructed jobs: a wrapper with an agent behind it identifies; an unrecognised wrapper does not descend; a wrapper with nothing behind it reports no agent; an agent in the pane's own job wins over one below it and skips the lookup entirely

## 3. Pane facts follow the identified job

- [x] 3.1 Read the reported process name and working directory from the job the agent was identified in, so a wrapped pane stops reporting the wrapper's cwd
- [x] 3.2 Leave panes with no nested identification reporting their own foreground job, unchanged
- [x] 3.3 Tests: a wrapped pane reports the nested job's cwd and process name; an unwrapped pane's reported facts are byte-identical to today's

## 4. Docs

- [x] 4.1 Troubleshooting entry in `docs/next/website/src/content/docs/troubleshooting.mdx` for an agent pane missing from the sidebar, naming Atuin's PTY proxy and saying which wrappers are recognised
- [x] 4.2 Changelog entry under `docs/next/CHANGELOG.md`

## 5. Verification

- [ ] 5.1 `just check` green
- [ ] 5.2 Confirm the unwrapped path is unchanged: with a wrapped and an unwrapped pane side by side, the unwrapped pane's probe performs no nested lookup
- [ ] 5.3 Dogfood on `-ac-beta` against the live wrapped Claude pane: it appears in the agents sidebar, resolves as an `herdr agent` target, and reports its own working directory rather than the wrapper's
- [ ] 5.4 Confirm the diff carries no fork-specific opinion, so it can be lifted upstream as-is
