#!/usr/bin/env bash
# Check a release asset directory before it is uploaded.
#
# Usage: packaging/check-release-dir.sh <dir>
#
# Fails when:
#   - redrob-launcher.json is missing, is not valid JSON, has schema != 1, or sharedAccount != true
#     (Redrob Launcher's docs/release-manifest.md; written by packaging/release-manifest.sh)
#   - SHA256SUMS.txt exists but does not list redrob-launcher.json, or does not verify
#
# The release workflow runs it on the merged artifacts right after the checksum step; the packaging
# lint runs it on a rehearsal directory so a broken manifest never waits for a tag to be noticed.
set -euo pipefail

dir="${1:?usage: check-release-dir.sh <dir>}"
manifest="$dir/redrob-launcher.json"
fail() { echo "::error::$*" >&2; exit 1; }

[ -f "$manifest" ] || fail "$manifest missing: run packaging/release-manifest.sh before the checksum step"
# jq -e exits 1 when the expression is false or null, and 2+ on a parse error.
jq -e '.schema == 1 and .sharedAccount == true' "$manifest" >/dev/null \
  || fail "$manifest must be valid JSON with schema 1 and sharedAccount true; got: $(cat "$manifest")"
[ "$(tail -c1 "$manifest" | od -An -c | tr -d ' ')" = '\n' ] || fail "$manifest must end with a newline"

if [ -f "$dir/SHA256SUMS.txt" ]; then
  grep -qE '^[0-9a-f]{64} [ *]redrob-launcher\.json$' "$dir/SHA256SUMS.txt" \
    || fail "$dir/SHA256SUMS.txt does not list redrob-launcher.json; write the manifest before the checksum step"
  (cd "$dir" && sha256sum --check --quiet SHA256SUMS.txt) || fail "$dir/SHA256SUMS.txt does not verify"
fi
echo "release dir ok: $dir (redrob-launcher.json schema 1, sharedAccount true)"
