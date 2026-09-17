# Contributing

Thanks for helping. This is the working agreement for the repository: how branches are named, what
has to be green before a merge, and the rule that follows from this being a **patch set over upstream
FreeCAD** rather than a fork of its history.

## The rule that shapes everything

**This repository holds only our changes.** Upstream FreeCAD is fetched at build time from the commit
pinned in `upstream.json` and is never committed: the shape this replaced was a single squashed import
of 13,265 tracked files with no upstream remote, which left no merge base and so no way to take an
upstream release.

- **`upstream.json` is the only place a baseline appears.**
- **`overlay/`** holds files that exist only in this product, copied in, never patched.
- **`patches/`** holds our modifications to upstream files, one patch per logical change.
- **Never edit the fetched checkout and stop there.** The edit disappears on the next fetch.

Upstream's own CI pipeline is deliberately not carried: it is built for upstream's infrastructure
(its LibPack hosting, its caches, its repository secrets) and running all 39 workflows unchanged on a
fork fails for reasons that have nothing to do with the change under review. What we do run is listed
below.

Version numbers like `26.3.0-dev` are upstream's own numbering for its cycle, not a fork
renumbering. Do not "fix" them.

## Branch model

One long-lived branch, `main`. Short-lived branches off it, merged by pull request.

- **`main`** is the trunk: no direct pushes, no force pushes, no deletion.
- **Working branches** are `<type>/<short-slug>`, e.g. `fix/windows-sign-filter`,
  `feat/agent-sidecar`. Types: `feat`, `fix`, `chore`, `docs`, `test`, `refactor`, `perf`.
- **Upstream bumps** land on `sync/upstream-<date>`.
- Release tags are cut from `main`, formatted `v<major>.<minor>.<patch>`.

[redrob-code](https://github.com/redrob-labs/redrob-code) and
[redrob-console](https://github.com/mckinley-and-rice/redrob-console) run Git Flow with a `develop`
branch. This one does not, so cut from `main`.

## Day to day

```bash
git switch main && git pull
git switch -c fix/short-description

./scripts/fetch-upstream.sh
./scripts/apply-patches.sh
# change the tree, then capture it as a patch
./scripts/verify-patches.sh      # what CI runs

# open a pull request into main
```

If you touch the Windows signing scripts, run `package/windows-sign-filter.test.sh`. It exists
because the signing step was re-signing hundreds of vendor DLLs that were already signed, and the
test is what keeps the filter narrow.

### Commits

Subject in the imperative. Use the body to say _why_: a patch's reason is what a future rebase needs.

## What CI checks

**`.github/workflows/verify-patches.yml`** applies every patch to the pinned upstream from a clean
tree and asserts the result. It starts clean on purpose: applying patches to a tree that is already
half-patched proves nothing, because a stale patch reports success when its changes are already there.

Run `./scripts/verify-patches.sh` before pushing.
