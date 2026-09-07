# SPDX-License-Identifier: LGPL-2.1-or-later
"""Register the OpenCAD AI workbench backed by RedRob Code."""

import FreeCADGui as Gui

_ICON = """
/* XPM */
static const char * opencad_ai_xpm[] = {
"16 16 4 1",
"  c None",
". c #5EEAD4",
"+ c #10151D",
"@ c #E8EEF7",
"      ....      ",
"    ........    ",
"   ..++++++..   ",
"  ..++@@@@++..  ",
" ..++@++++@++.. ",
" ..+@++..++@+.. ",
"..++@+.  .+@++..",
"..++@+.  .+@++..",
"..++@+.  .+@++..",
"..++@+.  .+@++..",
" ..+@++..++@+.. ",
" ..++@++++@++.. ",
"  ..++@@@@++..  ",
"   ..++++++..   ",
"    ........    ",
"      ....      "};
"""


class ShowAgentCommand:
    def GetResources(self):
        return {
            "Pixmap": _ICON,
            "MenuText": "RedRob AI Agent",
            "ToolTip": "Open the RedRob Code agent for the active CAD document",
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
            "MenuText": "Stop RedRob Agent",
            "ToolTip": "Interrupt the active RedRob Code request",
        }

    def Activated(self):
        import RedRobAgent

        RedRobAgent.stop_active_request()

    def IsActive(self):
        return True


class OpenCADAIWorkbench(Gui.Workbench):
    MenuText = "OpenCAD AI"
    ToolTip = "RedRob-powered CAD planning and Python automation"
    Icon = _ICON

    def Initialize(self):
        Gui.addCommand("OpenCAD_AI_Show", ShowAgentCommand())
        Gui.addCommand("OpenCAD_AI_Stop", StopAgentCommand())
        commands = ["OpenCAD_AI_Show", "OpenCAD_AI_Stop"]
        self.appendToolbar("OpenCAD AI", commands)
        self.appendMenu("OpenCAD AI", commands)

    def Activated(self):
        import RedRobAgent

        RedRobAgent.show_panel()

    def GetClassName(self):
        return "Gui::PythonWorkbench"


Gui.addWorkbench(OpenCADAIWorkbench())
