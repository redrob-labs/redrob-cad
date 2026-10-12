#!/usr/bin/env bash
# CI check for the macOS disk image: it carries the branded window (a .DS_Store and the background
# art), and a screenshot of that window as Finder opens it.
#
# Usage: packaging/macos/dmg/ci-check.sh <image.dmg> <screens-dir>
set -euo pipefail
DMG="$1"
OUT="$2"
mkdir -p "$OUT"
MNT="$(mktemp -d)"
hdiutil attach -readonly -nobrowse -mountpoint "$MNT" "$DMG" >/dev/null
trap 'hdiutil detach "$MNT" -force >/dev/null 2>&1 || true' EXIT
ls -la "$MNT"
test -d "$MNT/Redrob CAD.app" || { echo "no Redrob CAD.app in the image" >&2; exit 1; }
test -L "$MNT/Applications" || { echo "no Applications link in the image" >&2; exit 1; }
test -s "$MNT/.DS_Store" || { echo "no .DS_Store: the window is not branded" >&2; exit 1; }
# dmgbuild stores the art at the volume root as .background.tiff (older layouts used .background/).
BG="$(ls -d "$MNT"/.background.* "$MNT"/.background/* 2>/dev/null | head -1)"
test -n "$BG" && test -s "$BG" || { echo "no background art on the volume" >&2; exit 1; }
echo "branded: .DS_Store and ${BG#"$MNT"/}"

# The window as a person sees it. Best effort: a runner without a GUI session skips it.
hdiutil detach "$MNT" -force >/dev/null
trap - EXIT
VOL="$(hdiutil attach -readonly "$DMG" | awk -F'\t' '/\/Volumes\//{print $NF}' | tail -1)"
if [ -n "$VOL" ]; then
  open "$VOL" || true
  sleep 4
  screencapture -x "$OUT/dmg-window.png" || echo "screencapture unavailable"
  hdiutil detach "$VOL" -force >/dev/null || true
fi
