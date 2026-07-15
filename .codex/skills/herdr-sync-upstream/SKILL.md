---
name: herdr-sync-upstream
description: Sync the herdr fork with upstream ogulcancelik/herdr — scratch-branch rebase of the fork patches onto upstream/master, force-push to origin and internal, disable new upstream bot workflows. Use when the user wants to pull/merge/integrate/sync upstream changes into the fork.
---

# Syncing the herdr fork with upstream

Fork model: `master` = `upstream/master` + a small linear patch set (kept
minimal on purpose). Never merge — rebase the patch set onto the new upstream
tip and force-push. History SHAs change on every sync; that is expected.

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

The last command previews conflict risk: those are the fork-patched files.
Expect the patch set to be ~4-6 commits touching `.github/workflows/release.yml`,
`.github/workflows/ci.yml`, `justfile`, `.gitignore`, `.claude/skills/`,
`.codex/skills/`, `.pi/skills/`.

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

Must show ONLY the fork files. Then check the load-bearing hunks in
`.github/workflows/release.yml`:

- `update-homebrew` job present (publishes Formula/herdr.rb to colangelo/homebrew-tap)
- build job env has `HERDR_BUILD_CHANNEL: ac` (+ the `Set fork build id from tag` step)
- tag-verify compares `TAG_BASE="${TAG_VERSION%%-ac*}"` against Cargo.toml
- upstream's `update-latest-json` job stays REMOVED (needs upstream deploy keys)

And in `.github/workflows/ci.yml`: the conventional-commits force-push guard
(`git cat-file -e "$BEFORE_SHA"` fallback). And in `justfile`: the `release-ac`
recipe at the end. Then:

```bash
uv run --with pyyaml python -c "import yaml; [yaml.safe_load(open(f)) for f in ['.github/workflows/release.yml','.github/workflows/ci.yml']]"
just --list >/dev/null
```

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
  Close pending-release issues, PR Gate, Preview) and
  `gh workflow disable <name>` any new ones. Keep: CI, Nix, Release,
  Build artifacts (manual).
- **CI on the pushed master** must go green (`gh run list --repo colangelo/herdr
  --branch master --limit 3`). The conventional-commits job tolerates the
  force-push via the fork guard.
- **Drift check**: skim upstream changes to `justfile` release recipes,
  `scripts/changelog.py`, and `release.yml` — if the release flow moved,
  update `.codex/skills/herdr-release/SKILL.md` and the `release-ac` recipe
  to match.
- Keep the fork's PROJECTS entry honest:
  `~/_sync/dev/CONTEXT/PROJECTS/herdr.md`.

Related: `.codex/skills/herdr-release/SKILL.md` (cutting the -ac release
after a sync), `~/_sync/dev/CONTEXT/SKILLS/fork-maintenance/SKILL.md`
(the general pattern).
