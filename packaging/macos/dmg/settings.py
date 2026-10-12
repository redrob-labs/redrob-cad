# dmgbuild settings for the Redrob CAD disk image (packaging/macos/package.sh runs it).
#
#   dmgbuild -s settings.py -D app=<Redrob CAD.app> -D background=<background.tiff> "Redrob CAD 1.2.3" out.dmg
#
# The window matches background.html: 660x400, the app at (170, 230), Applications at (490, 230),
# 96px icons, no toolbar or sidebar. dmgbuild writes the .DS_Store itself, so no Finder scripting.
import os.path

application = defines["app"]  # noqa: F821 -- injected by dmgbuild
appname = os.path.basename(application)

format = "UDZO"
compression_level = 9
filesystem = "HFS+"
size = None

files = [application]
symlinks = {"Applications": "/Applications"}
hide_extensions = [appname]

background = defines["background"]  # noqa: F821 -- a multi-resolution TIFF (1x + 2x)
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
sidebar_width = 0
window_rect = ((200, 120), (660, 400))
default_view = "icon-view"
show_icon_preview = False
arrange_by = None
grid_spacing = 100
scroll_position = (0, 0)
label_pos = "bottom"
text_size = 13
icon_size = 96
icon_locations = {appname: (170, 230), "Applications": (490, 230)}
