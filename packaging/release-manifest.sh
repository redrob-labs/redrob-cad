#!/usr/bin/env bash
# Write the Redrob Launcher release manifest into a release asset directory.
#
# Usage: packaging/release-manifest.sh [<dir>]      (default: dist/release)
#
# Every GitHub Release carries `redrob-launcher.json` beside the installers and SHA256SUMS.txt, so the
# launcher learns from the release itself what it can do. Contract: Redrob Launcher's
# docs/release-manifest.md (schema 1). `sharedAccount` is true because this app reads the launcher's
# shared account file since v0.0.3. Run it BEFORE the checksum step so the file is listed in
# SHA256SUMS.txt; packaging/check-release-dir.sh verifies both.
set -euo pipefail

dir="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/dist/release}"
mkdir -p "$dir"
printf '{ "schema": 1, "sharedAccount": true }\n' >"$dir/redrob-launcher.json"
echo "wrote $dir/redrob-launcher.json"
