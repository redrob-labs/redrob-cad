# SPDX-License-Identifier: LGPL-2.1-or-later

set(OpenCADAI_SRCS
    Init.py
    InitGui.py
    RedRobAgent.py
)

set(OpenCADAI_RESOURCES
    Resources/icons/redrob-cad.png
)

source_group("" FILES ${OpenCADAI_SRCS})
source_group("Resources\\icons" FILES ${OpenCADAI_RESOURCES})

add_custom_target(OpenCADAI ALL SOURCES ${OpenCADAI_SRCS} ${OpenCADAI_RESOURCES})
fc_copy_sources(
    OpenCADAI
    "${CMAKE_BINARY_DIR}/Mod/OpenCADAI"
    ${OpenCADAI_SRCS}
    ${OpenCADAI_RESOURCES}
)

install(FILES ${OpenCADAI_SRCS} DESTINATION Mod/OpenCADAI)
install(FILES ${OpenCADAI_RESOURCES} DESTINATION Mod/OpenCADAI/Resources/icons)
