#!/usr/bin/env bash
# Derive every app icon under assets/app-icon/ from the pinned design-system sources in
# assets/brand/ and nothing else. Needs ImageMagick 7 (`magick`), python3 and cargo. Runs on any
# OS; the outputs are committed, so builds and packaging never need these tools.
#
#   packaging/brand-assets.sh
#
# What is derived, and from what (assets/brand/README.md has the rules):
#   hicolor/{16,32,48,64}            the design system's per-size favicons, as is
#   hicolor/512, redrob-cad-512.png  the 512 tile, as is
#   hicolor/{24,128,256}             the next larger tile resized DOWN (32, 192, 512), never up
#   redrob-cad-256.png               the window icon: the 256 above
#   redrob-cad.ico                   16, 32, 48, 64 as is + 256 resized down from 512
#   redrob-cad.icns                  16, 32, 128, 256, 512
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
B="$ROOT/assets/brand"
A="$ROOT/assets/app-icon"
ID="ai.redrob.cad"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
command -v magick >/dev/null || { echo "error: magick (ImageMagick 7) not found" >&2; exit 1; }

(cd "$B" && sha256sum -c --quiet SHA256SUMS)

down() { magick "$1" -filter Lanczos -resize "$2x$2" "PNG32:$3"; }

rm -rf "$A"
mkdir -p "$A"
for s in 16 24 32 48 64 128 256 512; do mkdir -p "$A/hicolor/${s}x${s}/apps"; done
for s in 16 32 48 64; do cp "$B/favicon-${s}x${s}.png" "$A/hicolor/${s}x${s}/apps/$ID.png"; done
cp "$B/icon-512.png" "$A/hicolor/512x512/apps/$ID.png"
down "$B/favicon-32x32.png" 24 "$A/hicolor/24x24/apps/$ID.png"
down "$B/icon-192.png" 128 "$A/hicolor/128x128/apps/$ID.png"
down "$B/icon-512.png" 256 "$A/hicolor/256x256/apps/$ID.png"

cp "$A/hicolor/256x256/apps/$ID.png" "$A/redrob-cad-256.png"
cp "$B/icon-512.png" "$A/redrob-cad-512.png"

(cd "$ROOT" && cargo run -q -p xtask -- ico "$A/redrob-cad.ico" \
  "$B/favicon-16x16.png" "$B/favicon-32x32.png" "$B/favicon-48x48.png" "$B/favicon-64x64.png" \
  "$A/redrob-cad-256.png")

python3 "$ROOT/packaging/make-icns.py" "$A/redrob-cad.icns" \
  16="$B/favicon-16x16.png" 32="$B/favicon-32x32.png" \
  128="$A/hicolor/128x128/apps/$ID.png" 256="$A/redrob-cad-256.png" 512="$B/icon-512.png"

echo "brand assets written to $A"
