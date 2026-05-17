---
name: herdr-release
description: Cut a release of the herdr fork — bumps version, promotes staged docs, tags, pushes binaries to GitHub Releases, and publishes to colangelo/homebrew-tap. Use when the user wants to release, ship, publish, tag, or cut a new version of herdr.
---

# Releasing herdr

This fork releases via `just release X.Y.Z`. The recipe has three gotchas that catch first-time runs:

1. Working tree must be clean
2. Version arg must NOT have a `v` prefix (recipe prepends `v` for the tag)
3. `docs/next/*.md` must be promoted to top-level for 5 gated files

## Pre-flight (must all be green before running)

Run these in parallel and read all output before proceeding:

```bash
git status --short
grep '^version' Cargo.toml | head -1
ls docs/next/
```

Then for each of `README.md CONFIGURATION.md INTEGRATIONS.md SOCKET_API.md CHANGELOG.md`:

```bash
diff -u "$f" "docs/next/$f"
```

## Procedure

### 1. Pick the version

- Drop any `v` prefix the user typed. `v0.5.10` → `0.5.10`.
- Must be strictly greater than the current `version` in `Cargo.toml`. SemVer bump (patch/minor/major) based on what shipped since the last tag.

### 2. Promote staged docs

For each file in `README.md CONFIGURATION.md INTEGRATIONS.md SOCKET_API.md CHANGELOG.md` where `diff` shows a difference, copy `docs/next/$f` → `$f`. Then commit:

```bash
git add README.md CONFIGURATION.md INTEGRATIONS.md SOCKET_API.md CHANGELOG.md
git commit -m "docs: promote docs/next to public for next release"
```

The `release-docs-check` recipe will diff these and abort if they don't match.

### 3. Ensure clean tree

If anything else is dirty, commit or stash first. `just release` aborts with `error: commit your changes first`.

### 4. Run the release

```bash
just release 0.5.10   # no v prefix
```

The recipe does the rest:
- `python3 scripts/changelog.py prepare --version X` (renames `## Unreleased` → `## [X] - <date>`)
- Mirrors the new CHANGELOG.md back into `docs/next/`
- Bumps `Cargo.toml` version + `cargo update -p herdr --offline` (refreshes `Cargo.lock`)
- Runs `just check` (lint + tests)
- Commits as `release: vX.Y.Z`
- Annotated tag `vX.Y.Z`
- `git push --follow-tags`

### 5. Watch GitHub Actions

The `v*` tag triggers `.github/workflows/release.yml`, which runs these jobs in order:

- **build** (matrix, 4 targets): linux x86_64/aarch64 + macos x86_64/aarch64. Uses Zig + libghostty-vt vendored build. Each binary uploaded as an artifact.
- **release**: downloads all 4 artifacts, extracts release notes from `CHANGELOG.md` via `scripts/changelog.py extract`, creates the GH Release.
- **update-homebrew**: shas the 4 binaries, clones `colangelo/homebrew-tap`, writes `Formula/herdr.rb`, pushes. Requires `HOMEBREW_TAP_TOKEN` repo secret.
- **update-latest-json**: regenerates `website/latest.json`, commits to `master`.
- **close-released-issues**: closes issues referenced via `refs #N` lines in commits between the previous tag and this one.

Watch:

```bash
command gh run watch
```

## After the release

Verify the formula landed and the install works:

```bash
command gh api repos/colangelo/homebrew-tap/contents/Formula/herdr.rb -H "Accept: application/vnd.github.raw" | head -10
brew update && brew install colangelo/tap/herdr
herdr --version
```

## Prerequisite: HOMEBREW_TAP_TOKEN

The `update-homebrew` job needs a PAT with write access to `colangelo/homebrew-tap`. It is repo-scoped (not org-level — there is no `colangelo` org). Set once:

```bash
command gh secret set HOMEBREW_TAP_TOKEN --repo colangelo/herdr --body 'ghp_...'
```

Reuse the value set on `colangelo/HittyPing` if still valid. If not, mint a fine-grained PAT at https://github.com/settings/tokens with `Contents: Read and write` on `colangelo/homebrew-tap`.

## Failure recovery

| Error | Fix |
|---|---|
| `commit your changes first` | Commit or stash anything dirty. The check ignores untracked-only state. |
| `tag vX.Y.Z already exists` | Pick the next version. Do NOT delete and re-push tags — releases are already cut. |
| `README.md differs from docs/next/README.md` | Step 2 above. |
| `cargo update -p herdr --offline` fails | Run `cargo update -p herdr` (without `--offline`) once locally, commit `Cargo.lock`, retry. |
| `update-homebrew` job fails on `git push` | `HOMEBREW_TAP_TOKEN` missing/expired on `colangelo/herdr` repo secrets. |
| Tag pushed but build failed mid-matrix | Push a fix commit, then re-trigger via `command gh workflow run release.yml -f tag=vX.Y.Z`. The workflow supports `workflow_dispatch` with a tag input. |

## Files that matter

- `justfile` — `release` recipe (line ~45)
- `.github/workflows/release.yml` — CI matrix + tap update
- `scripts/changelog.py` — `prepare` / `extract` / `sync-latest-json` subcommands
- `docs/next/` — staging area for unreleased doc changes
- `website/latest.json` — bumped by `update-latest-json` job, not the local release recipe
