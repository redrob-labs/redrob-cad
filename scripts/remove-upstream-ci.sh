#!/usr/bin/env bash
#
# Removes upstream FreeCAD's own CI from the fetched checkout.
#
# WHY THIS IS A SCRIPT AND NOT A PATCH: it deletes 39 files. As a patch that is 39 unreadable
# deletion hunks that conflict every time upstream touches its own workflows -- which it does
# often. As a list it is reviewable, and adding or dropping an entry is a one-line diff.
#
# WHY IT IS REMOVED AT ALL: upstream's pipeline is built for upstream's own infrastructure --
# its LibPack hosting, its caches, its repository secrets and its label automation. Running all
# 39 workflows unchanged on a fork produces a wall of red that nobody reads, which is worse than
# no signal.
#
# The previous shape of this repository achieved the same thing by deleting the files in the
# import commit, which is how it ended up with no CI AT ALL and no record that upstream's had
# ever existed -- including no way to run the Windows signing filter test this repository itself
# added. Our own CI lives in .github/workflows and is not affected by this script.
#
# Re-enabling a specific upstream workflow is now a deliberate act: drop its line from
# scripts/upstream-ci-files.txt and it comes back on the next build.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

target="build/freecad"
list="scripts/upstream-ci-files.txt"
[ -d "$target" ] || { echo "error: $target missing; run fetch-upstream.sh first" >&2; exit 1; }
[ -f "$list" ] || { echo "error: $list missing" >&2; exit 1; }

removed=0
absent=0
while IFS= read -r rel; do
  [ -n "$rel" ] || continue
  case "$rel" in \#*) continue ;; esac
  if [ -e "$target/$rel" ]; then
    rm -f "$target/$rel"
    removed=$((removed + 1))
  else
    # Upstream deleted or renamed it since the pin. Not fatal -- the goal is absence -- but
    # reported, because a list that has drifted is a list nobody is maintaining.
    echo "  note: $rel is already absent upstream"
    absent=$((absent + 1))
  fi
done < "$list"

echo "upstream CI removed: $removed file(s)${absent:+, $absent already absent}"
