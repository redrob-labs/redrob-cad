#!/usr/bin/env bash
# Derive every app icon under assets/app-icon/ from the pinned design-system source in
# assets/brand/ and nothing else. Needs a Chromium (headless shell is enough; set CHROME to its
# path), ImageMagick 7 (`magick`), python3 and cargo. The outputs are committed, so builds and
# packaging never need these tools.
#
#   CHROME=/path/to/chrome-headless-shell packaging/brand-assets.sh
#
# What is derived, and from what (assets/brand/README.md has the rules):
#   1024 master                      redrob-cad-icon-web.svg rendered by Chromium at 1024 px
#                                    (ImageMagick's SVG reader drops the gradients, so never it)
#   hicolor/{16..512}, -256, -512    the master resized DOWN, never up
#   redrob-cad.ico                   16, 24, 32, 48, 64, 256
#   redrob-cad.icns                  16, 32, 64, 128, 256, 512, 1024
#   favicon.ico                      16, 32, 48, 64 (the web app's favicon)
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
B="$ROOT/assets/brand"
A="$ROOT/assets/app-icon"
ID="ai.redrob.cad"
SVG="$B/redrob-cad-icon-web.svg"
CHROME="${CHROME:-$HOME/.cache/ms-playwright/chromium_headless_shell-1243/chrome-headless-shell-linux64/chrome-headless-shell}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
command -v magick >/dev/null || { echo "error: magick (ImageMagick 7) not found" >&2; exit 1; }
[ -x "$CHROME" ] || { echo "error: Chromium not found; set CHROME" >&2; exit 1; }

(cd "$B" && sha256sum -c --quiet SHA256SUMS)

cp "$SVG" "$TMP/icon.svg"
printf '%s\n' '<!DOCTYPE html><html><head><style>html,body{margin:0;background:transparent}img{display:block;width:1024px;height:1024px}</style></head><body><img src="icon.svg"></body></html>' > "$TMP/icon.html"
"$CHROME" --headless --no-sandbox --hide-scrollbars --default-background-color=00000000 \
  --window-size=1024,1024 --screenshot="$TMP/1024.png" "file://$TMP/icon.html" >/dev/null 2>&1
magick "$TMP/1024.png" PNG32:"$TMP/1024.png"

down() { magick "$TMP/1024.png" -filter Lanczos -resize "$1x$1" "PNG32:$2"; }

rm -rf "$A"
mkdir -p "$A"
for s in 16 24 32 48 64 128 256 512; do
  mkdir -p "$A/hicolor/${s}x${s}/apps"
  down "$s" "$A/hicolor/${s}x${s}/apps/$ID.png"
done
H="$A/hicolor"
cp "$H/256x256/apps/$ID.png" "$A/redrob-cad-256.png"
cp "$H/512x512/apps/$ID.png" "$A/redrob-cad-512.png"
cp "$TMP/1024.png" "$A/redrob-cad-1024.png"

(cd "$ROOT" && cargo run -q -p xtask -- ico "$A/redrob-cad.ico" \
  "$H/16x16/apps/$ID.png" "$H/24x24/apps/$ID.png" "$H/32x32/apps/$ID.png" \
  "$H/48x48/apps/$ID.png" "$H/64x64/apps/$ID.png" "$A/redrob-cad-256.png")
(cd "$ROOT" && cargo run -q -p xtask -- ico "$A/favicon.ico" \
  "$H/16x16/apps/$ID.png" "$H/32x32/apps/$ID.png" "$H/48x48/apps/$ID.png" "$H/64x64/apps/$ID.png")

python3 "$ROOT/packaging/make-icns.py" "$A/redrob-cad.icns" \
  16="$H/16x16/apps/$ID.png" 32="$H/32x32/apps/$ID.png" 64="$H/64x64/apps/$ID.png" \
  128="$H/128x128/apps/$ID.png" 256="$A/redrob-cad-256.png" 512="$A/redrob-cad-512.png" \
  1024="$A/redrob-cad-1024.png"

echo "brand assets written to $A"
