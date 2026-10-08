# SPDX-License-Identifier: LGPL-2.1-or-later
"""Commands of the Redrob CAD AI workbench.

They live in a real module, not in InitGui.py, because FreeCAD exec()s InitGui.py in a way that
hides its own top-level names from the methods defined in it (see InitGui.py). Here GetResources
can see ICON, and Activated can see everything it imports.
"""

from pathlib import Path

import FreeCADGui as Gui

ICON = str(Path(__file__).with_name("Resources") / "icons" / "redrob-cad.png")

SHOW = "RedrobCAD_Agent_Show"
STOP = "RedrobCAD_Agent_Stop"


class ShowAgentCommand:
    def GetResources(self):
        return {
            "Pixmap": ICON,
            "MenuText": "Redrob CAD AI",
            "ToolTip": "Open Redrob Code for the active CAD document",
            "Accel": "A, I",
        }

    def Activated(self):
        import RedrobAgent

        RedrobAgent.show_panel()

    def IsActive(self):
        return True


class StopAgentCommand:
    def GetResources(self):
        return {
            "Pixmap": ICON,
            "MenuText": "Stop Redrob Code",
            "ToolTip": "Interrupt the request Redrob Code is working on",
        }

    def Activated(self):
        import RedrobAgent

        RedrobAgent.stop_active_request()

    def IsActive(self):
        return True


_registered = False


def register(workbench):
    """Adds the commands once per session and puts them on the workbench's toolbar and menu."""
    global _registered
    if not _registered:
        Gui.addCommand(SHOW, ShowAgentCommand())
        Gui.addCommand(STOP, StopAgentCommand())
        _registered = True
    workbench.appendToolbar("Redrob CAD AI", [SHOW, STOP])
    workbench.appendMenu("Redrob CAD AI", [SHOW, STOP])
