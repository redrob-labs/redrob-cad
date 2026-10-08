# SPDX-License-Identifier: LGPL-2.1-or-later
"""Register the Redrob CAD AI workbench.

FreeCAD does not import this file, it exec()s it inside its own init function
(src/Gui/FreeCADGuiInit.py). Two things follow, and each one broke this workbench before:

* there is no ``__file__``, so a path cannot be built from it here. The workbench failed to load
  with "name '__file__' is not defined" and was missing from the workbench list;
* names defined here land in exec's locals, while a method defined here looks names up in the
  init module's globals, so a method cannot see them either.

So the methods only import real modules, which behave normally, and the one value needed when
the class is built -- the icon path -- is resolved from a real module's location and attached
after the class body.
"""

import importlib.util
from pathlib import Path

import FreeCAD as App
import FreeCADGui as Gui


class RedrobCADWorkbench(Gui.Workbench):
    MenuText = "Redrob CAD AI"
    ToolTip = "Plan and automate CAD work with Redrob Code"
    Icon = ""

    def Initialize(self):
        import RedrobCADCommands

        RedrobCADCommands.register(self)

    def Activated(self):
        import RedrobAgent

        RedrobAgent.show_panel()

    def GetClassName(self):
        return "Gui::PythonWorkbench"


_spec = importlib.util.find_spec("RedrobAgent")
_module_dir = (
    Path(_spec.origin).parent
    if _spec and _spec.origin
    else Path(App.getHomePath()) / "Mod" / "RedrobCAD"
)
RedrobCADWorkbench.Icon = str(_module_dir / "Resources" / "icons" / "redrob-cad.png")
Gui.addWorkbench(RedrobCADWorkbench())
