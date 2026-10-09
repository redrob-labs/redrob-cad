# SPDX-License-Identifier: LGPL-2.1-or-later
"""Offers, once, to bring a FreeCAD user's setup into Redrob CAD.

Redrob CAD keeps its own settings folder (branding.xml names the application "Redrob CAD", and
FreeCAD derives every user folder from that name). That is what stops the two from overwriting each
other when both are installed. The cost is that a FreeCAD user starts with nothing: no preferences,
no macros, no add-ons. So on the first start of a profile, if FreeCAD's settings are on this
computer, the user is asked once whether to copy them.

Copying is one-way. FreeCAD's own files are only read, never written, and nothing that already
exists in Redrob CAD's folders is overwritten. Paths inside the copied preferences that point into
FreeCAD's folders are rewritten to point into ours, or the two would share a macro folder again
through the back door.

The functions above the GUI section take plain paths and do not import FreeCAD, so
scripts/test-freecad-import.py can exercise them without the application.
"""

from __future__ import annotations

import os
import re
import shutil
import sys
from dataclasses import dataclass, field
from pathlib import Path
from xml.etree import ElementTree

UPSTREAM_NAME = "FreeCAD"
OFFERED_FLAG = "FreeCADImportOffered"
_VERSION_DIR = re.compile(r"^v(\d+)-(\d+)$")


# --- finding FreeCAD's profile ------------------------------------------------------------------


def standard_homes() -> tuple[Path, Path]:
    """The (config, data) base folders FreeCAD uses on this platform, as ApplicationDirectories
    computes them: Windows keeps both under %APPDATA%; elsewhere they are Qt's generic locations."""
    if sys.platform == "win32":
        appdata = Path(os.environ.get("APPDATA", Path.home() / "AppData" / "Roaming"))
        return appdata, appdata
    if sys.platform == "darwin":
        return Path.home() / "Library" / "Preferences", Path.home() / "Library" / "Application Support"
    config = Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")
    data = Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local" / "share")
    return config, data


def newest_version_dir(base: Path) -> Path | None:
    """FreeCAD 1.1 and later keep one folder per version (v1-1, v26-3, ...); the newest one wins."""
    if not base.is_dir():
        return None
    found = []
    for child in base.iterdir():
        match = _VERSION_DIR.match(child.name)
        if match and child.is_dir():
            found.append(((int(match.group(1)), int(match.group(2))), child))
    return max(found)[1] if found else None


@dataclass
class Profile:
    config: Path  # holds user.cfg
    data: Path  # holds Macro/ and Mod/
    config_base: Path  # FreeCAD's folders without a version, which old paths may still name
    data_base: Path
    macro: Path | None = None  # where FreeCAD keeps macros, if they are FreeCAD's to copy
    macros: list[Path] = field(default_factory=list)
    addons: list[Path] = field(default_factory=list)


def find_profile(config_home: Path, data_home: Path, name: str = UPSTREAM_NAME) -> Profile | None:
    """FreeCAD's profile under the given homes, or None when there is nothing worth offering."""
    config_base, data_base = config_home / name, data_home / name
    config = newest_version_dir(config_base) or config_base
    data = newest_version_dir(data_base) or data_base
    if not (config / "user.cfg").is_file():
        return None
    profile = Profile(config=config, data=data, config_base=config_base, data_base=data_base)

    macro = macro_path_from(config / "user.cfg") or data / "Macro"
    if is_inside(macro, data_base) or is_inside(macro, config_base):
        profile.macro = macro
        if macro.is_dir():
            profile.macros = sorted(p for p in macro.iterdir() if p.is_file())
    # A macro folder the user moved elsewhere is shared on purpose; it is not copied.

    mods = data / "Mod"
    if mods.is_dir():
        profile.addons = sorted(p for p in mods.iterdir() if p.is_dir() and not p.name.startswith("."))
    return profile


def macro_path_from(user_cfg: Path) -> Path | None:
    try:
        root = ElementTree.parse(user_cfg).getroot()
    except (ElementTree.ParseError, OSError):
        return None
    for group in root.iter("FCParamGroup"):
        if group.get("Name") == "Macro":
            for text in group.iter("FCText"):
                if text.get("Name") == "MacroPath" and (text.text or "").strip():
                    return Path(text.text.strip())
    return None


def is_inside(path: Path, base: Path) -> bool:
    try:
        Path(os.path.normpath(path)).relative_to(os.path.normpath(base))
        return True
    except ValueError:
        return False


# --- copying ------------------------------------------------------------------------------------


def rewrite_paths(text: str, moves: dict[str, str]) -> str:
    """Points every path in the preferences that was inside FreeCAD's folders into ours. Longest
    first, so a versioned folder is matched before the base folder that contains it."""
    for old in sorted(moves, key=len, reverse=True):
        text = text.replace(old, moves[old])
    return text


def path_moves(profile: Profile, config: Path, data: Path) -> dict[str, str]:
    moves = {}
    pairs = ((profile.data, data), (profile.config, config), (profile.data_base, data), (profile.config_base, config))
    for old, new in pairs:
        for sep in ("/", "\\"):
            moves[str(old).rstrip("/\\") + sep] = str(new).rstrip("/\\") + sep
        moves[str(old).rstrip("/\\")] = str(new).rstrip("/\\")
    return moves


def copy_new(source: Path, target: Path) -> bool:
    """Copies a file or folder unless the target already exists. Never writes to the source."""
    if target.exists():
        return False
    target.parent.mkdir(parents=True, exist_ok=True)
    if source.is_dir():
        shutil.copytree(source, target, symlinks=True)
    else:
        shutil.copy2(source, target)
    return True


@dataclass
class Copied:
    macros: int = 0
    addons: int = 0
    preferences: Path | None = None  # rewritten copy of user.cfg, ready to insert


def copy_profile(profile: Profile, config: Path, data: Path, macro: Path, scratch: Path) -> Copied:
    copied = Copied()
    for item in profile.macros:
        copied.macros += copy_new(item, macro / item.name)
    for item in profile.addons:
        copied.addons += copy_new(item, data / "Mod" / item.name)
    text = (profile.config / "user.cfg").read_text(encoding="utf-8", errors="surrogateescape")
    rewritten = scratch / "freecad-user.cfg"
    rewritten.write_text(
        rewrite_paths(text, path_moves(profile, config, data)), encoding="utf-8", errors="surrogateescape"
    )
    copied.preferences = rewritten
    return copied


# --- GUI ----------------------------------------------------------------------------------------


def offer_later(delay_ms: int = 2500) -> bool:
    """Schedules the offer for after the main window is up. False when there is no Qt to do it."""
    try:
        from PySide import QtCore
    except ImportError:  # headless, or the gate's stand-in loader
        return False
    QtCore.QTimer.singleShot(delay_ms, offer)
    return True


def _custom_folders() -> bool:
    # A run pointed at its own folders (tests, portable installs) is not a user's first start.
    return any(os.environ.get(v) for v in ("FREECAD_USER_HOME", "FREECAD_USER_DATA"))


def offer():
    import FreeCAD as App
    import FreeCADGui as Gui
    from PySide import QtCore, QtWidgets

    prefs = App.ParamGet("User parameter:BaseApp/Preferences/RedrobCAD")
    if prefs.GetBool(OFFERED_FLAG, False) or _custom_folders():
        return
    profile = find_profile(*standard_homes())
    ours_config, ours_data = Path(App.getUserConfigDir()), Path(App.getUserAppDataDir())
    if profile is None or is_inside(ours_data, profile.data) or is_inside(profile.data, ours_data):
        return
    prefs.SetBool(OFFERED_FLAG, True)

    window = Gui.getMainWindow()
    box = QtWidgets.QMessageBox(window)
    box.setObjectName("RedrobCADImportOffer")
    box.setWindowTitle("Bring over your FreeCAD setup?")
    box.setText(
        "Redrob CAD keeps its own settings, separate from FreeCAD.\n\n"
        f"FreeCAD's settings are on this computer: your preferences, {len(profile.macros)} "
        f"macro(s) and {len(profile.addons)} add-on(s). Copy them into Redrob CAD?"
    )
    box.setInformativeText("FreeCAD keeps its own copy, unchanged.")
    copy_button = box.addButton("Copy them", QtWidgets.QMessageBox.AcceptRole)
    box.addButton("Start fresh", QtWidgets.QMessageBox.RejectRole)
    box.setDefaultButton(copy_button)
    box.exec()
    if box.clickedButton() is not copy_button:
        return

    QtWidgets.QApplication.setOverrideCursor(QtCore.Qt.WaitCursor)
    try:
        copied = copy_profile(
            profile,
            ours_config,
            ours_data,
            Path(App.getUserMacroDir(True)),
            Path(App.getTempPath()),
        )
        # Insert merges FreeCAD's preferences into the live tree, which is saved on exit;
        # writing user.cfg directly would be overwritten by the running application.
        App.ParamGet("User parameter:").Insert(str(copied.preferences))
        prefs.SetBool(OFFERED_FLAG, True)
        App.saveParameter()
    finally:
        QtWidgets.QApplication.restoreOverrideCursor()

    done = QtWidgets.QMessageBox(window)
    done.setObjectName("RedrobCADImportDone")
    done.setWindowTitle("FreeCAD setup copied")
    done.setText(
        f"Copied your preferences, {copied.macros} macro(s) and {copied.addons} add-on(s).\n\n"
        "Add-ons load the next time Redrob CAD starts."
    )
    restart_button = done.addButton("Restart now", QtWidgets.QMessageBox.AcceptRole)
    done.addButton("Later", QtWidgets.QMessageBox.RejectRole)
    done.exec()
    if done.clickedButton() is restart_button:
        _restart(window)


def _restart(window):
    from PySide import QtCore, QtWidgets

    app = QtWidgets.QApplication.instance()

    def relaunch():
        args = list(app.arguments())[1:]
        QtCore.QProcess.startDetached(app.applicationFilePath(), args, app.applicationDirPath())

    app.aboutToQuit.connect(relaunch)
    window.close()  # asks about unsaved documents; cancelling it cancels the restart
