---
name: herdr-release
description: Cut an -ac suffixed release of the herdr fork — promotes staged docs, runs just release-ac, publishes binaries to GitHub Releases and the colangelo/homebrew-tap formula. Use when the user wants to release, ship, publish, tag, or cut a new version of herdr.
---

# Releasing herdr (fork, -ac channel)

Fork versioning keeps the upstream base visible: release `X.Y.Z-ac` where
`X.Y.Z` is the upstream version the tree is based on (`grep '^version' Cargo.toml`).
Re-releases on the same base: `X.Y.Z-ac.2`, `X.Y.Z-ac.3`.

Why this shape: `Cargo.toml` MUST stay plain `X.Y.Z` — `src/update.rs
Version::parse` panics on suffixes. The `-ac` suffix reaches the binary via
`HERDR_BUILD_CHANNEL=ac` (+ `HERDR_BUILD_ID` for `.N`) set in the release
workflow, so `herdr --version` prints `X.Y.Z-ac` with zero Rust patches.
Upstream's own preview builds use the same mechanism.

## Pre-flight

```bash
git status --short                      # must be clean (untracked .env is fine)
grep '^version' Cargo.toml | head -1    # upstream base = the X.Y.Z for the tag
just release-docs-check                 # shows which docs/next files need promoting
gh run list --repo colangelo/herdr --branch master --limit 3   # CI green?
```

Also confirm:
- `docs/next/CHANGELOG.md` has a non-empty `## Unreleased` section (prepare
  aborts on empty; upstream syncs it at their releases, fork content accrues
  between).
- Local `just check` needs Zig 0.15: `brew install zig@0.15` (keg-only) and
  `export PATH="$(brew --prefix zig@0.15)/bin:$PATH"` for the session.

## 1. Promote staged docs

For every file `release-docs-check` flags (`README.md CONFIGURATION.md
INTEGRATIONS.md SOCKET_API.md CHANGELOG.md` + whatever upstream added to the
gate — read the recipe): `cp docs/next/$f $f`, then commit:

```bash
git commit -m "docs: promote docs/next to public for next release"
```

## 2. Release

```bash
export PATH="$(brew --prefix zig@0.15)/bin:$PATH"
just release-ac 0.7.1-ac        # full fork version, no v prefix
```

The fork recipe (end of `justfile`): validates format, requires clean tree +
new tag, runs `release-docs-check`, `changelog.py prepare` (writes the
`[X.Y.Z-ac]` section), keeps `Cargo.toml` at the BASE version, runs
`just check`, commits `release: vX.Y.Z-ac`, tags, pushes master + tag to
origin.

## 3. Watch CI

Tag push triggers `.github/workflows/release.yml`:
- **flake-check** (nix), **validate-release-inputs**, **build** ×4 targets
  (with `HERDR_BUILD_CHANNEL=ac`) gate → **release** (creates the GitHub
  Release with notes extracted from the `[X.Y.Z-ac]` changelog section)
- **update-homebrew** → writes `Formula/herdr.rb` in colangelo/homebrew-tap
  (needs repo secret `HOMEBREW_TAP_TOKEN`, a PAT with write to the tap)
- **close-released-issues** is `continue-on-error` and fails harmlessly
  (upstream bot secret); upstream's `update-latest-json` is removed in the fork.

```bash
gh run watch
```

## 4. Verify

```bash
gh api repos/colangelo/homebrew-tap/contents/Formula/herdr.rb -H "Accept: application/vnd.github.raw" | head -6
brew update && brew install colangelo/tap/herdr   # or brew upgrade herdr
herdr --version                                    # must print herdr X.Y.Z-ac
```

Also push master to the internal mirror: `git push internal master`.

## Failure recovery

| Error | Fix |
|---|---|
| `version must look like 0.7.1-ac` | Pass the FULL fork version to `release-ac`, no `v` prefix. |
| `commit your changes first` | Commit or stash; untracked-only state is fine. |
| `<file> differs from docs/next/<file>` | Step 1. |
| `Unreleased section is empty` | Nothing accrued in `docs/next/CHANGELOG.md`; add at least one entry (e.g. the upstream sync summary) before releasing. |
| `cargo update -p herdr --offline` fails | Run `cargo update -p herdr` without `--offline`, then finish the recipe steps manually (check → commit → tag → push). Do NOT rerun release-ac: CHANGELOG is already prepared. |
| `just check` fails: `failed to execute zig build ... No such file or directory` | Zig missing/not on PATH — see pre-flight. For metadata-only releases, skipping local check and trusting tag CI is acceptable with user sign-off. |
| Tag CI: `tag ... doesn't match Cargo.toml version` | Cargo.toml must equal the tag's base (strip `-ac*`). The recipe does this; manual releases must too. |
| `update-homebrew` fails on `git push` | `HOMEBREW_TAP_TOKEN` missing/expired on colangelo/herdr → `gh secret set HOMEBREW_TAP_TOKEN --repo colangelo/herdr`. |
| Build failed mid-matrix after tag push | Fix, then re-run the workflow for the same tag (`gh run rerun <id>`). If the fix needs a commit, cut `-ac.2`. |
| `brew install` gets old version | `brew update` first; formula lives in colangelo/homebrew-tap Formula/herdr.rb. |

## Known behavior (not bugs)

- The fork binary's self-update checker compares against upstream's herdr.dev
  manifest; `0.7.1-ac` binaries treat upstream `0.7.2` as an update and would
  self-update to an UPSTREAM binary. Brew users should update via
  `brew upgrade`; direct installs can set `[update] version_check = false`.
- Release tags point at commits that become orphaned after the next upstream
  sync (history is rebased). Tags and their releases stay valid.

Related: `.claude/skills/herdr-sync-upstream/SKILL.md` (sync before releasing),
`~/_sync/dev/CONTEXT/SKILLS/fork-maintenance/SKILL.md` (general pattern).
