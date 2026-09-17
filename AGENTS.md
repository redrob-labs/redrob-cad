# AGENTS.md

Operating notes for an agent working in `redrob-labs/redrob-cad`. Everything here was verified by
reading the file or running the command in this repository. Nothing is inherited from a sibling
repository's conventions.

## What this repository is

Redrob CAD is a parametric 3D modeller with an AI agent sidecar, an LGPL-2.1-or-later fork of
FreeCAD. This repository holds only the changes: upstream FreeCAD is fetched at build time from a
pinned commit and is never committed, so the tracked tree is 28 files rather than the 13,265 of the
squashed import it replaced. The customization is 6 patches, 11 overlay files, and a list of 39
upstream files to delete, applied to a 13,000-file upstream by scripts in `scripts/`.

## Branch model, verified against the live repository

`develop` is the default branch and the base of every pull request. `main` is released state and
moves by merging `develop` into it. Release tags are cut from `main`.

A hotfix branches from `main`, merges into `main`, releases, and then `main` is merged back into
`develop`. Skipping that back-merge is a real failure mode, not a formality: the sibling repository
`redrob-code` spent a month with a lockfile its own default branch could not install from because
that merge back was missed.

Working branches are `<type>/<short-slug>`. The types are `feat`, `fix`, `docs`, `chore`, `test`,
`refactor`, `perf`, `hotfix`, `release`, and `sync`. Nothing on `develop` declared a set before this
paragraph, so this one was chosen rather than inherited: it is a superset of the seven types in the
`CONTRIBUTING.md` that exists only on pull request #7, plus names for the three long-lived motions
the model above already describes. `.github/workflows/gitflow.yml` enforces it on every pull request,
because a rule nothing checks is a preference: this repository already carries four `kiro/` branches
that no document defines. The same workflow checks after every push to `main` that `main` is
contained in `develop`, which is the back-merge described above.

Both branches are protected identically, read from the branch protection API: pull requests only,
force pushes blocked, deletions blocked, 0 required approving reviews, admin enforcement off. The
one required status check on both is `Patches apply to pinned upstream`.

`.github/workflows/verify-patches.yml` has a bare `pull_request:` trigger with no branch filter, so
that required check does report on a pull request targeting `develop`. This is worth stating because
the sibling repository `redrob-design` had the opposite: its required check ran only on
`workflow_dispatch` and a nightly schedule, so every pull request sat blocked on a check that could
never report. `gitflow.yml` uses a bare `pull_request:` trigger for the same reason, so neither
workflow can be silently filtered away from `develop`.

The same workflow's `push:` trigger names only `main`. Pushes to `develop` therefore run nothing,
and a merged pull request produces no post-merge run on the default branch. That is a gap, not a
break: the check still gates every pull request before the merge.

Two traps a newcomer hits:

- The local remote HEAD symref is stale. `git symbolic-ref refs/remotes/origin/HEAD` returns
  `refs/remotes/origin/main` in an existing clone, so a `git checkout -b x origin/HEAD` cuts from the
  wrong branch. Name `origin/develop` explicitly. The GitHub default really is `develop`.
- `CONTRIBUTING.md` is not on `develop`. It exists only on the branch of open pull request #7, and
  its text states a single-trunk model with no `develop` branch and tells you to cut from `main`.
  That contradicts the protection settings above. If #7 merges unchanged, the repository will carry
  written instructions that disagree with its own configuration.

Caveat as of this writing: pull request #7 targets `main`, not `develop`. When it merges, `main` will
be ahead of `develop` and needs a back-merge. Right now
`git rev-list --count origin/develop..origin/main` is **0**, and the reverse direction is 0 as well,
so the two branches are identical. Re-run that command before assuming it still is.

## The patch and pin mechanism, which is what to get right first

The pin lives in one file: `upstream.json`, whose `repo` is
`https://github.com/FreeCAD/FreeCAD.git` and whose `commit` is
`d1424838c2cf9858f7b35cf6b774cfa07f7ba515`. It also carries `version` (`26.3.0-dev`). Nothing else
in the repository names a baseline, so a bump is a one-line reviewable change.

The customization is split by the nature of each change, and the split is deliberate:

- `patches/` holds 6 numbered patches, our edits to files that already exist upstream. They touch
  `src/Mod/CMakeLists.txt`, `src/Gui/Application.cpp`, `src/Gui/StartupProcess.cpp`,
  `src/Main/CMakeLists.txt`, the preference pack and stylesheet CMake files,
  `package/libpack-windows/create_bundle.sh`, `package/rattler-build/windows/create_bundle.sh`, and
  a large sixth patch that rewrites user-visible strings across `src/Gui`.
- `overlay/` holds 11 files that exist only in this product: the `src/Mod/OpenCADAI` workbench,
  `src/Main/branding.xml`, the OpenCAD preference pack and stylesheet parameters, the Windows signing
  filter test, and two design documents. A patch that creates a whole new file is a less readable
  copy of that file and conflicts on nothing, so these are copied rather than patched.
- `scripts/upstream-ci-files.txt` lists 39 upstream files to delete, which is upstream FreeCAD's
  entire CI pipeline. As a patch that would be 39 deletion hunks conflicting every time upstream
  touches its own workflows. As a list, re-enabling one workflow is a one-line diff.

Four scripts run in a fixed order, and `scripts/verify-patches.sh` is the gate that runs all of them:

1. `scripts/fetch-upstream.sh` reads `repo` and `commit` from `upstream.json`, shallow-clones into
   `build/freecad` with `core.autocrlf false` and `core.eol lf`, and asserts the checked-out SHA
   equals the pin. The line-ending settings are load-bearing: a Windows runner with default
   `autocrlf` gives a CRLF tree that a Linux-generated patch will not apply to.
2. `scripts/apply-patches.sh` applies `patches/*.patch` in numeric order with
   `git apply --3way --whitespace=nowarn`, and exits on the first failure with the conflicting hunk
   printed. `--3way` is why a drifted patch reports a conflict instead of quietly doing nothing.
3. `scripts/apply-overlay.sh` copies every `overlay/` file into the checkout, then re-walks the
   overlay and `cmp`s each file against its copy. A silent copy failure would otherwise produce a
   build that is upstream but claims to be ours.
4. `scripts/remove-upstream-ci.sh` deletes the 39 listed files, and prints a note for any entry
   already absent upstream rather than failing, because absence is the goal.

The gate then asserts the result rather than trusting the steps. It checks that
`src/Mod/CMakeLists.txt` registers `OpenCADAI` and that the workbench's four files and its icon are
present and installed by CMake, that `branding.xml` exists and is installed and that both
`setApplicationDisplayName` and the `Redrob CAD` window title are in place, that the preference pack
and stylesheet are installed, that the shipped `version.json` matches the pin, that the libpack
bundler still names `vcruntime140.dll`, that upstream's workflows are gone while ours survives, that
the signing filter test is both present and wired into CI, and finally that
`scripts/sweep-strings.py --check` finds zero user-visible strings still naming FreeCAD and that one
single product name is used throughout.

The gate begins with `rm -rf build/freecad` and refetches. That is the single most important line in
it: verifying against a half-patched tree proves nothing, because a stale patch reports success when
its changes are already present. Do not skip the refetch to save time.

The required status check `Patches apply to pinned upstream` is the `name:` of the single job in
`verify-patches.yml`. It runs `./scripts/verify-patches.sh` and then
`bash build/freecad/package/windows-sign-filter.test.sh`, so the required check and the local gate
are the same command.

What breaks when the pin moves: the whole gate is a chain and step 2 is where the pin is felt. A
patch whose context no longer matches the new upstream fails `git apply --3way` and the run stops
with that patch named. Bumping is therefore three steps: edit `commit` and `version` in
`upstream.json`, run `./scripts/verify-patches.sh`, and regenerate any failing patch against the new
upstream, committing the regenerated patch and the pin bump together. Two failure shapes are not
patch conflicts and will confuse you if you expect one: if upstream edits `version.json`, the pin
and the built tree disagree and the gate fails on the version assertion instead; and if upstream adds
new user-visible strings naming FreeCAD, `sweep-strings.py --check` fails, which is fixed by running
that script without `--check` and regenerating patch 0006, not by editing the checkout.

Never edit `build/freecad` and stop there. It is gitignored, and the gate deletes it on every run.

`scripts/sweep-strings.py` is narrow on purpose and should stay narrow. It rewrites only Qt
`tr("...")` and `QT_TR_NOOP("...")` first arguments in `.cpp`, and `<string>` and `<title>` text in
`.ui` files. Identifiers, includes, `FreeCADGui` and `FreeCADCmd`, Python module names, the `.FCStd`
extension, preference keys and URLs are left alone because they are load-bearing names, not labels.
It also holds back phrases naming upstream's own resources, its forum, wiki, website, developers
handbook and donation page, because renaming those would point our product name at somebody else's
resources. It takes the product name from `<Application>` in `overlay/src/Main/branding.xml`, so
that file is the single place the name is decided.

## Layout

```
upstream.json                    the pin: upstream repo, commit, version. The only baseline.
patches/                         6 patches, our edits to files that exist upstream
overlay/                         11 files that exist only in this product, copied not patched
  src/Mod/OpenCADAI/             the AI workbench (CMakeLists, Init.py, InitGui.py, RedRobAgent.py, icon)
  src/Main/branding.xml          product identity, and the source of the swept name
  src/Gui/PreferencePacks/       OpenCAD preference pack
  src/Gui/Stylesheets/           OpenCAD stylesheet parameters
  package/windows-sign-filter.test.sh
  docs/                          OpenCAD-architecture.md, feature-parity.md
scripts/                         fetch, apply, sweep, and the gate
  upstream-ci-files.txt          39 upstream CI files to delete
.github/workflows/               one workflow: verify-patches.yml
build/                           gitignored. The fetched upstream lands in build/freecad.
```

Own code against pinned upstream: everything tracked in this repository is ours. Upstream appears
only inside `build/freecad`, which is gitignored, produced by a script, and destroyed by the gate.
The previous tree, the squashed import, is preserved on the `archive/opencad-snapshot` branch.

## Build, test, lint

There is no `package.json`, `Cargo.toml`, `Makefile`, `justfile`, `pixi.toml` or root
`CMakeLists.txt` in this repository. The toolchain at this level is bash plus `python3`, both used
directly by the scripts. The actual application build is upstream FreeCAD's own CMake plus pixi
toolchain, run against the tree the gate leaves in `build/freecad`; that build was not run here.

Commands, with the exit code observed on this host:

| Command | Purpose | Exit code |
|---|---|---|
| `./scripts/verify-patches.sh` | the gate, identical to the required check | 0 |
| `bash build/freecad/package/windows-sign-filter.test.sh` | the second CI step, needs the gate to have run first | 0 |
| `bash -n scripts/*.sh` | shell syntax check, each of the 5 scripts | 0 |
| `python3 -m py_compile scripts/sweep-strings.py` | Python syntax check | 0 |

The gate's own output on a passing run: 6 patches applied, 11 overlay files verified, 39 upstream CI
files removed, `upstream version matches the pin: 26.3.0-dev`, `string sweep would rewrite 0
user-visible string(s)`, `one product name throughout: Redrob CAD`. The signing filter test prints
`matched 8, skipped 3, to sign 5` and `PASS`.

`bash -n` and `py_compile` are the fastest checks and catch nothing the gate does not, so use them
only as a quick smoke test after editing a script. There is no configured linter and no formatter.

## Workflows

Two workflows exist: `.github/workflows/verify-patches.yml` and `.github/workflows/gitflow.yml`.

| Workflow | Triggers | Runs on a pull request |
|---|---|---|
| `verify patches` | `pull_request` (no branch filter), `push` to `main`, `workflow_dispatch` | yes, on every pull request regardless of base |
| `gitflow` | `pull_request` (no branch filter), `push` to `main`, `workflow_dispatch` | yes, the branch name job only; the back-merge job runs on the other two triggers |

Its single job is named `Patches apply to pinned upstream`, which is the required status check on
both protected branches. It runs on `ubuntu-latest`, has `contents: read` permission only, pins
`actions/checkout` by commit SHA, and cancels in-progress runs per pull request.

## Not verified here

The FreeCAD CMake and pixi application build was not run, so nothing in this file claims the
customized tree compiles or that the workbench loads at run time. `overlay/docs/feature-parity.md`
labels its own entries "Source retained" and says runtime verification is pending, so treat parity
claims as source-level, not behavioural.
