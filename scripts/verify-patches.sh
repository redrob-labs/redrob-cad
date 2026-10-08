#!/usr/bin/env bash
#
# THE GATE. Proves the whole customization still produces the tree we expect from the pinned
# upstream, and fails loudly when it does not.
#
# It DISCARDS any existing checkout and refetches first. Verifying against a tree that is already
# half-patched proves nothing -- a patch that no longer applies would report success because its
# changes were already present. The sibling redrob-browser repository carries 258 Firefox patches
# with no gate at all; one went stale and its CI trigger named a branch that does not exist, so
# nothing ran and nobody knew.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

echo "=== verifying customization against pinned upstream ==="

rm -rf build/freecad
./scripts/fetch-upstream.sh
./scripts/apply-patches.sh
./scripts/apply-overlay.sh
./scripts/remove-upstream-ci.sh

# The product name is swept into user-visible strings at build time, translations included, and
# the result must equal the reviewed list. This used to be a frozen 1 MB patch: it rewrote nine
# CRLF files whole, and it never touched a .ts file, so 98 Korean strings fell back to English.
python3 scripts/sweep-strings.py build/freecad --expected scripts/sweep-strings.expected.tsv

target="build/freecad"
module="$target/src/Mod/RedrobCAD"

# --- 1. the AI workbench is registered, not merely present -------------------------
# A copied-in module that nothing adds to the build produces a green build with no workbench.
if ! grep -q "add_subdirectory(RedrobCAD)" "$target/src/Mod/CMakeLists.txt"; then
  echo "error: src/Mod/CMakeLists.txt does not add the RedrobCAD subdirectory" >&2
  exit 1
fi
for f in CMakeLists.txt Init.py InitGui.py RedrobAgent.py RedrobCADCommands.py; do
  if [ ! -f "$module/$f" ]; then
    echo "error: src/Mod/RedrobCAD/$f missing from the checkout" >&2
    exit 1
  fi
  if [ "$f" != "CMakeLists.txt" ] && ! grep -q "$f" "$module/CMakeLists.txt"; then
    echo "error: src/Mod/RedrobCAD/$f is not installed by its CMakeLists.txt" >&2
    exit 1
  fi
done
echo "Redrob CAD AI workbench present and registered"

# --- 1b. ... and it loads ---------------------------------------------------------
# Registered is not loaded. The workbench used Path(__file__) in InitGui.py, which FreeCAD runs
# with exec() and no __file__, so it never appeared in the workbench list -- with every check
# above green. This runs the file under FreeCAD's own exec rules.
python3 scripts/test-workbench-init.py "$module"
python3 -c 'import ast, sys; [ast.parse(open(p, encoding="utf-8").read(), p) for p in sys.argv[1:]]' "$module"/*.py

# --- 2. branding reached the executable -------------------------------------------
if [ ! -f "$target/src/Main/branding.xml" ]; then
  echo "error: src/Main/branding.xml missing -- the build would ship as FreeCAD" >&2
  exit 1
fi
if ! grep -q "branding.xml" "$target/src/Main/CMakeLists.txt"; then
  echo "error: src/Main/CMakeLists.txt does not install branding.xml" >&2
  exit 1
fi
if ! grep -qi "redrob" "$target/src/Main/branding.xml"; then
  echo "error: branding.xml does not carry the Redrob name" >&2
  exit 1
fi
# branding.xml alone does not set what the user sees on the window. Qt reads the display name from
# the application object, and FreeCAD sets the main window title separately -- so both have to be
# ours or the app presents itself as FreeCAD in the title bar and the task switcher.
if ! grep -q "setApplicationDisplayName" "$target/src/Gui/Application.cpp"; then
  echo "error: src/Gui/Application.cpp does not set an application display name" >&2
  exit 1
fi
if ! grep -q "Redrob CAD" "$target/src/Gui/StartupProcess.cpp"; then
  echo "error: the main window title is not branded" >&2
  exit 1
fi
echo "window title and display name branded"

# --- 2b. the workbench ships its icon --------------------------------------------
# The CMake target has to install it, not merely list it: a workbench whose icon is absent at run
# time falls back to a generic placeholder, which looks like a broken install.
if [ ! -f "$module/Resources/icons/redrob-cad.png" ]; then
  echo "error: the Redrob CAD AI workbench icon is missing from the checkout" >&2
  exit 1
fi
if ! grep -q "RedrobCAD_RESOURCES" "$module/CMakeLists.txt"; then
  echo "error: the workbench icon is present but CMake does not install it" >&2
  exit 1
fi
echo "workbench icon present and installed"
echo "branding applied: $(grep -oiE '<(Application|WindowTitle)>[^<]*' "$target/src/Main/branding.xml" | head -1 | cut -d'>' -f2)"

# --- 3. the preference pack and stylesheet are installed, and agree ----------------
# FreeCAD finds a theme's parameters at qss:parameters/<Theme>.yaml. A pack whose Theme names no
# installed file leaves every token unresolved, which is a wall of warnings and a half-styled UI.
pack="$target/src/Gui/PreferencePacks/Redrob CAD/Redrob CAD.cfg"
if [ ! -f "$pack" ]; then
  echo "error: the Redrob CAD preference pack is missing" >&2
  exit 1
fi
if ! grep -q '"Redrob CAD"' "$target/src/Gui/PreferencePacks/CMakeLists.txt"; then
  echo "error: the preference pack is present but not installed by CMake" >&2
  exit 1
fi
if ! grep -q "<name>Redrob CAD</name>" "$target/src/Gui/PreferencePacks/package.xml"; then
  echo "error: the preference pack is not declared in package.xml" >&2
  exit 1
fi
theme=$(grep -o 'Name="Theme">[^<]*' "$pack" | cut -d'>' -f2)
if [ ! -f "$target/src/Gui/Stylesheets/parameters/$theme.yaml" ]; then
  echo "error: the pack's theme '$theme' has no parameters/$theme.yaml" >&2
  exit 1
fi
if ! grep -q "\"parameters/$theme.yaml\"" "$target/src/Gui/Stylesheets/CMakeLists.txt"; then
  echo "error: parameters/$theme.yaml is present but not installed by CMake" >&2
  exit 1
fi
if ! grep -q "SetASCII(\"Theme\", \"$theme\")" "$target/src/Gui/StartupProcess.cpp"; then
  echo "error: new profiles do not default to the pack's theme '$theme'" >&2
  exit 1
fi
echo "preference pack and stylesheet installed: theme '$theme'"

# --- 4. upstream's own version is intact ------------------------------------------
# version.json is upstream's, unmodified. FreeCAD adopted CalVer in FEP-0003, so 26.3.0-dev is
# its own numbering for the third 2026 cycle. If a patch ever starts editing it, the pin in
# upstream.json and the shipped version would drift apart silently.
declared=$(python3 -c "
import json
print(json.load(open('upstream.json'))['version'])
")
built=$(python3 -c "
import json
d = json.load(open('$target/version.json'))
print(f\"{d['version_major']}.{d['version_minor']}.{d['version_patch']}-{d['version_suffix']}\")
")
if [ "$declared" != "$built" ]; then
  echo "error: upstream.json says $declared but the checkout says $built" >&2
  exit 1
fi
echo "upstream version matches the pin: $built"

# --- 5. vendor binaries are not re-signed -----------------------------------------
# The reason patch 0004 exists. MSVC redistributables arrive Microsoft-signed and re-signing them
# replaces a good signature with ours and bills us for it.
if ! grep -q "vcruntime140.dll" "$target/package/libpack-windows/create_bundle.sh"; then
  echo "error: the libpack bundler no longer names the vendor-signed redistributables" >&2
  exit 1
fi
echo "vendor re-signing filter present"

# --- 6. upstream's CI is gone and ours is not ------------------------------------
if [ -d "$target/.github/workflows" ] && [ -n "$(find "$target/.github/workflows" -maxdepth 1 -name '*.yml' -print -quit)" ]; then
  echo "error: upstream workflows survived removal" >&2
  find "$target/.github/workflows" -maxdepth 1 -name '*.yml' | sed 's/^/    /' >&2
  exit 1
fi
if [ ! -f ".github/workflows/verify-patches.yml" ]; then
  echo "error: this repository has no CI of its own" >&2
  exit 1
fi
echo "upstream CI removed, ours retained"

# --- 7. the signing filter test this repository owns actually runs ---------------
# It was added by PR #1 and, before this conversion, had no workflow to run it.
if [ ! -f "$target/package/windows-sign-filter.test.sh" ]; then
  echo "error: the signing filter test is not in the built tree" >&2
  exit 1
fi
if ! grep -q "windows-sign-filter.test.sh" .github/workflows/verify-patches.yml; then
  echo "error: the signing filter test exists but CI does not run it" >&2
  exit 1
fi
echo "signing filter test present and wired into CI"

# --- 8. our own names, spelled one way --------------------------------------------
# The product carried an earlier working name, and the engine was written "RedRob". Neither may
# survive in anything this repository controls -- file names included. The exceptions are lines
# marked legacy-name, which read a profile from before the rename so its settings carry over, and
# the name of the branch that preserves the old tree.
hits=$({
  git grep -n -I -i 'opencad' -- . ':!scripts/verify-patches.sh' || true
  git grep -n -I 'RedRob' -- . ':!scripts/verify-patches.sh' || true
  git ls-files | grep -i 'opencad' || true
  git ls-files | grep 'RedRob' || true
} | grep -v -e 'legacy-name' -e 'archive/opencad-snapshot' || true)
if [ -n "$hits" ]; then
  echo "error: an earlier name is still in the repository:" >&2
  echo "$hits" | sed 's/^/    /' >&2
  exit 1
fi
echo "no earlier product name and no 'RedRob' spelling left"

# --- no user-visible string names FreeCAD, and there is only ONE product name -------
python3 scripts/sweep-strings.py build/freecad --check

# The repository used to carry two names: branding.xml said one thing while the window title, set
# separately in src/Gui, said Redrob CAD -- so the app presented a different name depending on where
# you looked. And MaintainerUrl pointed at a third party's repository. One name, asserted here.
declared=$(python3 -c "
from xml.etree import ElementTree
root = ElementTree.parse('overlay/src/Main/branding.xml').getroot()
print((root.find('Application').text or '').strip())
")
if [ "$declared" != "Redrob CAD" ]; then
  echo "error: branding.xml declares '$declared', expected 'Redrob CAD'" >&2
  exit 1
fi
if ! grep -q "$declared" "$target/src/Gui/StartupProcess.cpp"; then
  echo "error: the window title does not match branding.xml ('$declared')" >&2
  exit 1
fi
if ! grep -q "redrob-labs/redrob-cad" overlay/src/Main/branding.xml; then
  echo "error: MaintainerUrl does not point at this repository" >&2
  exit 1
fi
echo "one product name throughout: $declared"

echo "=== customization verified ==="
