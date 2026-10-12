# Upstream: storytold/cadcraft

This repository is a **hard fork** of [storytold/cadcraft](https://github.com/storytold/cadcraft),
carrying upstream's full history up to one of its release tags. It is not a patch stack and it is not a
mirror.

한국어: [UPSTREAM.ko.md](./UPSTREAM.ko.md)

## Where the fork was cut

| | |
|---|---|
| Upstream tag | `v0.3.0` |
| Upstream commit | `59631c8` — 2026-10-08, `Bump version to 0.3.0 (from 0.2.0) for release` |
| Upstream default branch | `main` |
| Our long-lived branches | `develop` (default), `main` (release) |

The machine-readable record is [`upstream-base.json`](./upstream-base.json). Because the history is
shared, `git merge-base develop upstream/main` is real, and an upstream sync is an ordinary three-way
merge, not a replay.

## Earlier base

Until 2026-10-09 this repository was a patch set over FreeCAD. That work is kept, not deleted:

- branch `archive/freecad` and tag `freecad-final` point at `a9a11cd`, the last FreeCAD-based `develop`;
- branch `archive/freecad-stage3-wip` holds unfinished work that was in progress at the switch.

## What this fork does not own

Upstream's `NOTICE` states that the ArtCraft name, wordmark and logos in `docs/brand/` are trademarks of
the ArtCraft Team, not open source, and not covered by the code licence. `upstream-base.json` lists that
prefix under `proprietaryPrefixes`. The rebrand removed it; `cargo xtask brand` fails if it comes back
on a sync, or if an upstream product name or link reaches a surface a user sees.

## Absorbing upstream

Follow upstream **tags**. Sync branches are `sync/upstream-<tag>`, opened as a pull request into `develop`,
and **merged as a merge commit, never a squash** — a squash destroys the merge base and the next sync
replays work already taken.

```sh
git remote add upstream https://github.com/storytold/cadcraft.git
git fetch upstream --tags
git checkout -b sync/upstream-vX.Y.Z develop
git merge vX.Y.Z          # resolve conflicts by hand; keep ours where we renamed, take theirs around it
cargo xtask ci
```

Then update `base`, `baseTag`, `baseDate`, `baseSubject` and `lastSyncedUpstream` in
`upstream-base.json` in the same pull request.

## Attribution

Upstream's copyright notice stays in `LICENSE-MIT` and `NOTICE`; ours is added below it, never in its
place. The code is available under MIT OR Apache-2.0, as upstream's is.
