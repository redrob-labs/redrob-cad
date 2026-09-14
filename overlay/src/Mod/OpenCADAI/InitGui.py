# SPDX-License-Identifier: LGPL-2.1-or-later
"""Register the Redrob CAD AI workbench."""

from pathlib import Path

import FreeCADGui as Gui

_ICON = str(Path(__file__).with_name("Resources") / "icons" / "redrob-cad.png")


class ShowAgentCommand:
    def GetResources(self):
        return {
            "Pixmap": _ICON,
            "MenuText": "Redrob CAD AI",
            "ToolTip": "Open the Redrob AI agent for the active CAD document",
            "Accel": "A, I",
        }

    def Activated(self):
        import RedRobAgent

        RedRobAgent.show_panel()

    def IsActive(self):
        return True


class StopAgentCommand:
    def GetResources(self):
        return {
            "Pixmap": _ICON,
            "MenuText": "Stop Redrob AI",
            "ToolTip": "Interrupt the active Redrob AI request",
        }

    def Activated(self):
        import RedRobAgent

        RedRobAgent.stop_active_request()

    def IsActive(self):
        return True


class OpenCADAIWorkbench(Gui.Workbench):
    MenuText = "Redrob CAD AI"
    ToolTip = "Redrob-powered CAD planning and Python automation"
    # FreeCAD may execute InitGui.py with distinct globals and locals. Resolve
    # the module-local resource path after the class body instead.
    Icon = ""

    def Initialize(self):
        Gui.addCommand("OpenCAD_AI_Show", ShowAgentCommand())
        Gui.addCommand("OpenCAD_AI_Stop", StopAgentCommand())
        commands = ["OpenCAD_AI_Show", "OpenCAD_AI_Stop"]
        self.appendToolbar("Redrob CAD AI", commands)
        self.appendMenu("Redrob CAD AI", commands)

    def Activated(self):
        import RedRobAgent

        RedRobAgent.show_panel()

    def GetClassName(self):
        return "Gui::PythonWorkbench"


OpenCADAIWorkbench.Icon = _ICON
Gui.addWorkbench(OpenCADAIWorkbench())
