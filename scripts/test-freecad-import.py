#!/usr/bin/env python3
"""Tests the FreeCAD-setup import without the application.

Builds a fake FreeCAD profile and a fresh Redrob CAD profile in a temporary home, then checks the
three promises RedrobCADImport makes: it finds the newest versioned profile (and the old unversioned
layout), it copies macros and add-ons without overwriting anything that already exists, and it never
writes to FreeCAD's files -- every byte under FreeCAD's folders is hashed before and after.

Usage: test-freecad-import.py <module-dir>
"""

from __future__ import annotations

import hashlib
import sys
import tempfile
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(sys.argv[1]).resolve()))
import RedrobCADImport as imp  # noqa: E402

failures: list[str] = []


def check(ok: bool, what: str):
    if not ok:
        failures.append(what)


def tree_hash(root: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(root.rglob("*")):
        digest.update(str(path.relative_to(root)).encode())
        if path.is_file():
            digest.update(path.read_bytes())
    return digest.hexdigest()


def user_cfg(macro_path: str) -> str:
    return (
        '<?xml version="1.0" encoding="utf-8"?>\n<FCParameters>\n'
        '<FCParamGroup Name="Root"><FCParamGroup Name="BaseApp"><FCParamGroup Name="Preferences">\n'
        f'<FCParamGroup Name="Macro"><FCText Name="MacroPath">{macro_path}</FCText></FCParamGroup>\n'
        '<FCParamGroup Name="General"><FCText Name="Language">Korean</FCText></FCParamGroup>\n'
        "</FCParamGroup></FCParamGroup></FCParamGroup>\n</FCParameters>\n"
    )


with tempfile.TemporaryDirectory() as tmp:
    home = Path(tmp)
    config_home, data_home = home / ".config", home / ".local" / "share"

    check(imp.find_profile(config_home, data_home) is None, "an empty home must offer nothing")

    # FreeCAD 1.0 left an unversioned profile; 1.1 added v1-1. The newest one must win.
    old_cfg = config_home / "FreeCAD"
    old_cfg.mkdir(parents=True)
    (old_cfg / "user.cfg").write_text(user_cfg("/nowhere"), encoding="utf-8")
    fc_config = config_home / "FreeCAD" / "v1-1"
    fc_data = data_home / "FreeCAD" / "v1-1"
    fc_config.mkdir(parents=True)
    (fc_data / "Macro").mkdir(parents=True)
    (fc_data / "Mod" / "DemoAddon").mkdir(parents=True)
    (fc_data / "Mod" / ".git").mkdir()
    (fc_config / "user.cfg").write_text(user_cfg(f"{fc_data}/Macro/"), encoding="utf-8")
    (fc_data / "Macro" / "hello.FCMacro").write_text("print('hello')\n", encoding="utf-8")
    (fc_data / "Macro" / "kept.FCMacro").write_text("print('freecad')\n", encoding="utf-8")
    (fc_data / "Mod" / "DemoAddon" / "Init.py").write_text("# addon\n", encoding="utf-8")

    profile = imp.find_profile(config_home, data_home)
    check(profile is not None, "a FreeCAD profile was not found")
    if profile:
        check(profile.config == fc_config, f"config should be the newest version, got {profile.config}")
        check(profile.data == fc_data, f"data should be the newest version, got {profile.data}")
        check([p.name for p in profile.macros] == ["hello.FCMacro", "kept.FCMacro"], "macros not listed")
        check([p.name for p in profile.addons] == ["DemoAddon"], "hidden folders must not count as add-ons")

        ours_config = config_home / "Redrob CAD" / "v26-3"
        ours_data = data_home / "Redrob CAD" / "v26-3"
        ours_macro = ours_data / "Macro"
        ours_macro.mkdir(parents=True)
        ours_config.mkdir(parents=True)
        (ours_macro / "kept.FCMacro").write_text("print('redrob')\n", encoding="utf-8")

        before = tree_hash(home / ".config" / "FreeCAD") + tree_hash(data_home / "FreeCAD")
        copied = imp.copy_profile(profile, ours_config, ours_data, ours_macro, home)
        after = tree_hash(home / ".config" / "FreeCAD") + tree_hash(data_home / "FreeCAD")

        check(before == after, "FreeCAD's files changed during the copy")
        check(copied.macros == 1 and copied.addons == 1, f"copied {copied.macros} macro(s), {copied.addons} add-on(s)")
        check((ours_macro / "hello.FCMacro").is_file(), "hello.FCMacro was not copied")
        check(
            (ours_macro / "kept.FCMacro").read_text(encoding="utf-8") == "print('redrob')\n",
            "an existing Redrob CAD macro was overwritten",
        )
        check((ours_data / "Mod" / "DemoAddon" / "Init.py").is_file(), "the add-on was not copied")
        prefs = copied.preferences.read_text(encoding="utf-8")
        check(f"{ours_data}/Macro/" in prefs, "MacroPath was not moved into Redrob CAD's folder")
        check(str(fc_data) not in prefs, "a path into FreeCAD's folder survived in the preferences")
        check("Korean" in prefs, "other preferences must come across untouched")

    # A macro folder the user moved out of FreeCAD's folders is shared on purpose: not copied.
    shared = home / "Documents" / "macros"
    shared.mkdir(parents=True)
    (fc_config / "user.cfg").write_text(user_cfg(f"{shared}/"), encoding="utf-8")
    profile = imp.find_profile(config_home, data_home)
    check(profile is not None and profile.macro is None and not profile.macros, "a custom macro folder must not be copied")

if failures:
    for line in failures:
        print(f"error: {line}", file=sys.stderr)
    sys.exit(1)
print("FreeCAD setup import: finds the newest profile, copies without overwriting, leaves FreeCAD's files alone")
