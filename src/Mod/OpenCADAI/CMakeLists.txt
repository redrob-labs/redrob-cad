# SPDX-License-Identifier: LGPL-2.1-or-later

set(OpenCADAI_SRCS
    Init.py
    InitGui.py
    RedRobAgent.py
)

source_group("" FILES ${OpenCADAI_SRCS})

add_custom_target(OpenCADAI ALL SOURCES ${OpenCADAI_SRCS})
fc_copy_sources(OpenCADAI "${CMAKE_BINARY_DIR}/Mod/OpenCADAI" ${OpenCADAI_SRCS})

install(FILES ${OpenCADAI_SRCS} DESTINATION Mod/OpenCADAI)
