---
name: herdr-sync-upstream
description: Sync the herdr fork with upstream ogulcancelik/herdr — scratch-branch rebase of the fork patches onto upstream/master, force-push to origin and internal, disable new upstream bot workflows. Use when the user wants to pull/merge/integrate/sync upstream changes into the fork.
---

# Syncing the herdr fork with upstream

Fork model: `master` = `upstream/master` + a linear patch set. Never merge —
rebase the patch set onto the new upstream tip and force-push. History SHAs
change on every sync; that is expected.

The patch set is no longer small: it was ~150 commits as of the 2026-07-25
sync, mostly sidebar/UI features, and it touches `src/` broadly (`src/app/`,
`src/api/`, `src/ui/`, `src/cli/`, `src/config/`). Budget for real source
conflicts, not just workflow-file ones.

Remotes: `origin` = github.com/colangelo/herdr, `internal` = Gitea
(ac/herdr), `upstream` = github.com/ogulcancelik/herdr.

## 1. Assess

```bash
git fetch upstream
git log upstream/master..master --oneline        # the fork patch set
git merge-base master upstream/master            # old upstream base
git rev-list --left-right --count upstream/master...master
git log master..upstream/master --oneline -- .github/workflows justfile scripts/changelog.py .gitignore
```

The last command previews conflict risk on the fork's infrastructure files:
`.github/workflows/release.yml`, `.github/workflows/ci.yml`, `justfile`,
`.gitignore`, `.claude/skills/`, `.codex/skills/`, `.pi/skills/`. Also diff
the fork-touched source files against upstream's churn:

```bash
git diff --name-only $(git merge-base master upstream/master) master | grep '^src/'
git log master..upstream/master --oneline -- src/
```

## 2. Rebase on a scratch branch

```bash
BASE=$(git merge-base master upstream/master)
git branch -f reconcile-test master
git checkout reconcile-test
git rebase --onto upstream/master "$BASE" reconcile-test
```

Known conflict patterns:
- `.gitignore` — upstream appends entries where the fork `.env` block sits: keep both.
- `.github/workflows/release.yml` — upstream restructures jobs; re-apply the fork
  hunks (see §4 checklist) rather than fighting the diff.
- `docs/next/CHANGELOG.md` — conflicts on nearly every fork docs commit, because
  both sides edit `## Unreleased`. Resolve by merging per `###` heading (upstream
  entries first, then fork entries), keeping ONE block per heading in the
  `Added` / `Changed` / `Fixed` order the released sections already use.
  **Automate this carefully** — two distinct ways it has gone wrong:
  1. A naive merger keying only on `###` merges across a `## [X.Y.Z]` release
     boundary and folds upstream's Unreleased entries into a released section.
  2. A resolution that keeps *both* sides' blocks leaves two `### Fixed` blocks
     in one section with the first entries duplicated between them. Hit on the
     2026-07-25 sync and only caught 2026-07-26, by eye: `## Unreleased` had
     `Added / Fixed / Changed / Fixed`, with two entries appearing twice.

  This is not cosmetic. `SECTION_RE` in `scripts/changelog.py` matches only
  `##`, so `prepare_release` copies the Unreleased body **verbatim** into the new
  version section — duplicate headings and repeated entries ship in the real
  release notes. `just check` does not catch it (`scripts.test_changelog` tests
  the tooling, not the document).

  After the rebase, verify BOTH:

  ```bash
  # 1. no duplicate ## version headings; sections reverse-chronological
  grep '^## ' docs/next/CHANGELOG.md | sort | uniq -d     # must be empty
  # 2. no repeated ### block or entry inside any one section — silence is pass
  awk '/^## /{s=$0; split("",h); split("",e); next} /^### /{if (h[$0]++) print "dup heading in " s ": " $0; next} /^- /{if (e[$0]++) print "dup entry in " s ": " substr($0,1,50)}' docs/next/CHANGELOG.md
  ```

  (One line on purpose — verified on macOS stock awk 20200816. A heredoc here
  breaks on paste, because its closing delimiter ends up indented.)

  When repairing an already-duplicated section, rebuild it programmatically with
  assertions on every line index rather than editing by eye, then prove nothing
  was lost:
  `diff <(grep '^- ' OLD | sort -u) <(grep '^- ' NEW | sort -u)` must be empty.
- `justfile` `test`/`check` recipes — both sides add `scripts.test_*` modules to
  the same `python3 -m unittest` line: take the union.
- `tests/cli/sessions.rs` — asserts a hardcoded protocol number. Keep the fork's
  value (fork protocol is intentionally ahead of upstream's) and make sure it
  matches `src/protocol/wire.rs::PROTOCOL_VERSION`.
- If a rebase goes sideways, `git rebase --abort` and retry; master is untouched
  until §5.

## 3. Never rebase orphaned release commits

Fork release commits (`release: vX.Y.Z-ac`, doc promotions) become orphaned
after each sync — do NOT carry them forward. Only the durable patch set
rebases. Release tags stay valid pointing at orphaned commits.

## 4. Verify the fork surface survived

```bash
git diff upstream/master reconcile-test --stat
```

Expect the fork's feature surface plus its infrastructure files; what must NOT
appear is deletions of upstream files
(`git diff upstream/master reconcile-test --diff-filter=D --name-only` should
be empty). Then check the load-bearing hunks in
`.github/workflows/release.yml`:

- `update-homebrew` job present (publishes Formula/herdr.rb to colangelo/homebrew-tap)
- build job env has `HERDR_BUILD_CHANNEL: ac` (+ the `Set fork build id from tag` step)
- tag-verify compares `TAG_BASE="${TAG_VERSION%%-ac*}"` against Cargo.toml
- upstream's `update-latest-json` job stays REMOVED (it needs upstream deploy
  keys). Note the fork has its OWN `update-latest-json` job — one job with that
  name is correct; tell them apart by `secrets.RELEASE_DEPLOY_KEY` (upstream's)
  vs `--repo colangelo/herdr` + `--tag`/`--force` (the fork's).

And in `.github/workflows/ci.yml`: the conventional-commits force-push guard
(`git cat-file -e "$BEFORE_SHA"` fallback). And in `justfile`: the `release-ac`
recipe at the end. Then:

```bash
uv run --with pyyaml python -c "import yaml; [yaml.safe_load(open(f)) for f in ['.github/workflows/release.yml','.github/workflows/ci.yml']]"
just --list >/dev/null
```

Then run `just check`. Use `cargo nextest run --locked --no-fail-fast` for the
test stage — nextest cancels the run on first failure by default, so a single
failure hides the other ~1600 tests and you cannot tell a one-off from a broad
breakage.

`just check` does NOT validate the changelog document, so run the two
`docs/next/CHANGELOG.md` duplicate checks from §2 here as well — a bad merge
resolution there is silent until it reaches published release notes. Note also
that `just check` omits `release-docs-check`; run that separately before any
release.

**Before blaming the rebase for a test failure, get an upstream baseline.**
Build clean `upstream/master` in a scratch worktree and run the same test
there:

```bash
wt switch --create upstream-baseline --base upstream/master
cargo nextest run --locked <test_name>          # in that worktree
wt remove upstream-baseline                     # when done
```

Known upstream-baseline failure (macOS, verified 2026-07-25 at upstream
`d4e0dd3d`): `live_handoff_keeps_unmanaged_agent_name_bound_to_saved_session`
fails with `agent_not_found` — it fails identically with zero fork patches
applied, so it is not a rebase regression. Re-verify against the baseline each
sync rather than assuming it is still upstream's.

## 5. Adopt and push

```bash
git checkout master
OLD=$(git rev-parse master)
git reset --hard reconcile-test && git branch -D reconcile-test
git push --force-with-lease=master:$OLD origin master
git push --force-with-lease=master:$OLD internal master
```

If the internal push times out: Tailscale may be stopped (`tailscale status`;
`tailscale up`), or see the m4m MagicDNS caveat in global CLAUDE.md.

## 6. Post-sync checks

- **New upstream bot workflows**: upstream's maintainer automation needs their
  secrets (`KANGAL_GITHUB_TOKEN`, `RELEASE_DEPLOY_KEY`) and fails on the fork.
  Compare `gh workflow list --repo colangelo/herdr --all` against the disabled
  set (Approve Contributor, Approve Merged Contributor, Issue Gate,
  Close pending-release issues, PR Gate, Preview, **Website**) and
  `gh workflow disable <name>` any new ones. Keep: CI, Nix, Release,
  Build artifacts (manual).
- **CI on the pushed master** must go green (`gh run list --repo colangelo/herdr
  --branch master --limit 3`). The conventional-commits job tolerates the
  force-push via the fork guard.
- **`website/latest.json` survived the rebase**: `just latest-json-check`.
  The release workflow's `update-latest-json` job commits the manifest to
  master *after* the tag, so it is an ordinary master commit that §5's
  force-push of a replayed patch set drops — silently, because the release
  itself already succeeded and no job goes red. `src/update.rs` reads that file
  over raw GitHub, so a dropped commit pins every fork binary's update check to
  an older version: exactly what happened to v0.7.4-ac, which went unnoticed
  from 2026-07-18 until 2026-07-27 (AC-forks/herdr#38). Restore it with the
  same call CI makes, using the tag's protocol version, not master's:

  ```bash
  TAG=v0.7.4-ac; BASE=${TAG#v}; BASE=${BASE%%-ac*}
  PROTOCOL=$(git show "$TAG:src/protocol/wire.rs" | sed -n 's/^pub const PROTOCOL_VERSION: u32 = \([0-9]*\);/\1/p')
  python3 scripts/changelog.py sync-latest-json --repo colangelo/herdr \
      --tag "$TAG" --version "$BASE" --protocol "$PROTOCOL" --force \
      --output website/latest.json
  ```
- **Drift check**: skim upstream changes to `justfile` release recipes,
  `scripts/changelog.py`, and `release.yml` — if the release flow moved,
  update `.claude/skills/herdr-release/SKILL.md` and the `release-ac` recipe
  to match.
- **Versioned release docs stay upstream-only.** Upstream v0.7.5+ added
  `docs/versions/` + `website/scripts/docs-versions.mjs` + a `Website`
  workflow. `docs-versions.mjs check` asserts the docs manifest matches
  `website/latest.json`, which the fork keeps fork-scoped, so it always fails
  here. `release-docs-check` deliberately omits that line and the `Website`
  workflow is disabled — if a sync reintroduces either, drop it again rather
  than "fixing" the manifest. Full rationale: the "Versioned release docs are
  upstream-only" section of `.claude/skills/herdr-release/SKILL.md`.
- Keep the fork's PROJECTS entry honest:
  `~/_sync/dev/CONTEXT/PROJECTS/herdr.md`.

Related: `.claude/skills/herdr-release/SKILL.md` (cutting the -ac release
after a sync), `~/_sync/dev/CONTEXT/SKILLS/fork-maintenance/SKILL.md`
(the general pattern).
