// Resource identifiers shared between redrob_cad_setup.rc and the C++ sources.
#pragma once

// RCDATA
#define IDR_PAYLOAD          101  // embedded redrob-cad MSI (absent in the uninstaller build)
#define IDR_BACKGROUND_PNG   102  // pre-rendered brand background art (res/background.png)
#define IDR_LOCKUP_PNG       103  // Redrob lockup + "CAD", solid-mono-white (res/lockup.png)
#define IDR_FONT_SANS        104  // Pretendard SemiBold, embedded (res/Pretendard-SemiBold.ttf)
#define IDR_FONT_KR          107  // Wanted Sans SemiBold, embedded (res/WantedSans-SemiBold.ttf)

// Application icon
#define IDI_APPICON          105  // assets/app-icon/redrob-cad.ico

// Manifest
#define IDR_MANIFEST           1  // per-monitor-v2 DPI + common controls
