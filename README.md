# OpenCAD — patch-based customization of FreeCAD

This repository holds **only our changes**. Upstream FreeCAD is fetched at build time, never
vendored, so `git log` here shows OpenCAD work rather than being buried under upstream history.

```
upstream.json     the pinned upstream repo + commit -- the only place a baseline appears
overlay/          files that exist only in this product (copied in, never patched)
patches/          our modifications to upstream files, one patch per logical change
scripts/          fetch, apply, and THE GATE
```

Build:

```
./scripts/verify-patches.sh     # fetch upstream, apply everything, assert the result
```

That leaves a complete, customized FreeCAD tree in `build/freecad`, which is then built with
FreeCAD's own CMake + pixi toolchain.

## Why this shape

The previous shape was a **single squashed import commit** — 13,265 tracked files, no upstream
remote, no tags, and no merge base with FreeCAD. Nothing recorded which upstream commit it came
from, so upstream releases could never be pulled: the fork could only diverge.

Recovering that baseline was the first task, and it was done by measurement rather than by
guessing. Diffing the snapshot against current upstream `main` showed **1,430 differing files**,
which looks like a heavily modified fork. It is not: of 120 randomly sampled differing files,
**every single one** had its snapshot content present verbatim somewhere in upstream's history.
The divergence was upstream moving on, not us editing. Scoring the snapshot against all 1,143
first-parent commits on `main` put the minimum at `d1424838c2` (2026-09-07) — **59 files** — and
all 59 are now accounted for:

| Kind | Count | Where it lives now |
|---|---|---|
| Files only we have | 10 | `overlay/` |
| Upstream files we modify | 10 | `patches/` (5 patches, grouped by intent) |
| Upstream files we delete | 39 | `scripts/upstream-ci-files.txt` |

So the customization is 20 files against a 13,000-file upstream. Storing the other ~13,200 was
what made upstream unpullable.

## The 39 deletions were upstream's entire CI

The import commit deleted **all of FreeCAD's own pipeline** — `CI_primary.yml`, the Linux, macOS
and Windows build actions, CodeQL, the release builds, the LibPack jobs. That is why this
repository had no CI: not an omission, a deletion, with no record that upstream's had ever
existed.

It is still removed, because upstream's pipeline is built for upstream's infrastructure — its
LibPack hosting, its caches, its repository secrets — and running all 39 unchanged on a fork
produces a wall of red nobody reads. The difference is that it is now a **reviewable list** in
`scripts/upstream-ci-files.txt` rather than an invisible deletion, and re-enabling one workflow is
a one-line diff.

Meanwhile this repository now has CI of its own, which it did not before. It runs the gate, and it
runs `package/windows-sign-filter.test.sh` — a test this repository added in PR #1 and then had no
way to execute.

## Versioning

`version.json` is upstream's, unmodified. FreeCAD adopted CalVer (`YY.N`) in FEP-0003, so
`26.3.0-dev` is upstream's own numbering for the third 2026 cycle — not a fork renumbering. The
gate asserts the shipped version matches `upstream.json`, so the pin and the build cannot drift
apart silently.

The `26.3` branch is scheduled for 2026-09-30. When it exists, prefer pinning that tag over a
`main`-branch commit.

## Bumping upstream

1. Edit `commit` (and `version`) in `upstream.json`.
2. Run `./scripts/verify-patches.sh`.
3. A patch that no longer applies fails with the conflicting hunk named. Regenerate it against the
   new upstream and commit both together.

The gate discards and refetches the checkout before applying anything. Verifying against a tree
that is already half-patched proves nothing — a stale patch would report success because its
changes were already there. The sibling `redrob-browser` repository carries 258 Firefox patches
with no gate; one went stale and its CI trigger named a branch that does not exist, so nothing ran
and nobody found out.

## The previous tree

Preserved on `archive/opencad-snapshot`. Nothing was force-pushed and no history was discarded.
