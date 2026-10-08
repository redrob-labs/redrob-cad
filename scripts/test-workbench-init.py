#!/usr/bin/env python3
"""Loads the AI workbench the way FreeCAD does, without FreeCAD.

FreeCAD does not import a module's InitGui.py. It exec()s the file inside a function of its own
init module (src/Gui/FreeCADGuiInit.py), so the file has no __file__, and the names it defines at
top level are invisible to the methods it defines. The workbench once died on the first of those
-- "name '__file__' is not defined" -- and simply vanished from the workbench list. Nothing caught
it, because nothing ever ran the file.

This runs InitGui.py under the same rules, with stand-ins for FreeCAD's two modules, then drives
the workbench the way FreeCAD would: register it, initialize it, and ask every command for its
resources. It does not import the agent panel itself, which needs a real Qt.

Usage: test-workbench-init.py <module-dir>
"""

from __future__ import annotations

import sys
import types
from pathlib import Path

# Importing the module under test must not leave a __pycache__ in the source tree.
sys.dont_write_bytecode = True

module_dir = Path(sys.argv[1]).resolve()
failures: list[str] = []

workbenches: list = []
commands: dict = {}


class Workbench:
    def __init__(self):
        self.toolbars: list = []
        self.menus: list = []

    def appendToolbar(self, name, items):
        self.toolbars.append((name, list(items)))

    def appendMenu(self, name, items):
        self.menus.append((name, list(items)))


gui = types.ModuleType("FreeCADGui")
gui.Workbench = Workbench
gui.addWorkbench = workbenches.append
gui.addCommand = commands.__setitem__
app = types.ModuleType("FreeCAD")
app.getHomePath = lambda: str(module_dir.parent.parent)
sys.modules["FreeCADGui"] = gui
sys.modules["FreeCAD"] = app

# FreeCAD puts every module directory on sys.path before it runs InitGui.py.
sys.path.insert(0, str(module_dir))

# The runner lives in a namespace like FreeCADGuiInit's -- no __file__, and the same handful of
# names that module defines at top level, which an InitGui method CAN see -- and exec()s the file
# from inside a function, exactly as FreeCADGuiInit.InitGuiFile.run_init_gui does. Anything else an
# InitGui method names is invisible to it in FreeCAD, and so it is here.
import importlib
import re
import traceback
import typing

runner: dict = {
    "__name__": "FreeCADGuiInit",
    "__builtins__": __builtins__,
    "path": module_dir / "InitGui.py",
    "FreeCAD": app,
    "FreeCADGui": gui,
    "App": app,
    "Gui": gui,
    "Path": Path,
    "importlib": importlib,
    "re": re,
    "traceback": traceback,
    "typing": typing,
}
exec(
    "def run_init_gui():\n"
    "    source = path.read_text(encoding='utf-8')\n"
    "    code = compile(source, str(path), 'exec')\n"
    "    exec(code)\n",
    runner,
)

try:
    runner["run_init_gui"]()
except Exception as exc:  # noqa: BLE001 -- the point is to report whatever FreeCAD would hit
    failures.append(f"InitGui.py fails under FreeCAD's exec rules: {type(exc).__name__}: {exc}")

if not failures:
    if len(workbenches) != 1:
        failures.append(f"InitGui.py registered {len(workbenches)} workbench(es), expected 1")
    else:
        wb = workbenches[0]
        if not Path(wb.Icon).is_file():
            failures.append(f"workbench icon does not exist: {wb.Icon}")
        if wb.GetClassName() != "Gui::PythonWorkbench":
            failures.append(f"unexpected class name {wb.GetClassName()!r}")
        try:
            wb.Initialize()
        except Exception as exc:  # noqa: BLE001
            failures.append(f"Initialize() fails: {type(exc).__name__}: {exc}")
        if not commands:
            failures.append("Initialize() registered no commands")
        for name, command in commands.items():
            try:
                resources = command.GetResources()
            except Exception as exc:  # noqa: BLE001
                failures.append(f"{name}.GetResources() fails: {type(exc).__name__}: {exc}")
                continue
            if not Path(resources.get("Pixmap", "")).is_file():
                failures.append(f"{name}: pixmap does not exist: {resources.get('Pixmap')!r}")
            if not resources.get("MenuText"):
                failures.append(f"{name}: no MenuText")
        listed = {c for _, items in wb.toolbars + wb.menus for c in items}
        if listed != set(commands):
            failures.append(f"toolbar/menu list {sorted(listed)} != registered {sorted(commands)}")

if failures:
    for line in failures:
        print(f"error: {line}", file=sys.stderr)
    sys.exit(1)
print(f"workbench loads under FreeCAD's exec rules: {workbenches[0].MenuText!r}, {len(commands)} command(s)")
