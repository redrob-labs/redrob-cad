# SPDX-License-Identifier: LGPL-2.1-or-later

set(RedrobCAD_SRCS
    Init.py
    InitGui.py
    RedrobAgent.py
    RedrobCADCommands.py
)

set(RedrobCAD_RESOURCES
    Resources/icons/redrob-cad.png
)

source_group("" FILES ${RedrobCAD_SRCS})
source_group("Resources\\icons" FILES ${RedrobCAD_RESOURCES})

add_custom_target(RedrobCAD ALL SOURCES ${RedrobCAD_SRCS} ${RedrobCAD_RESOURCES})
fc_copy_sources(
    RedrobCAD
    "${CMAKE_BINARY_DIR}/Mod/RedrobCAD"
    ${RedrobCAD_SRCS}
    ${RedrobCAD_RESOURCES}
)

install(FILES ${RedrobCAD_SRCS} DESTINATION Mod/RedrobCAD)
install(FILES ${RedrobCAD_RESOURCES} DESTINATION Mod/RedrobCAD/Resources/icons)
