# Redrob CAD — branded Windows setup and uninstaller

`redrob-cad-<version>-windows-<arch>-setup.exe` is a native Win32 window that installs Redrob CAD
for the current Windows account. It is redrob-browser's setup (`installer/windows` in that
repository) with Redrob CAD's payload, so the two products install the same way.

- **Install:** the embedded MSI (`../redrob-cad.wxs`) runs silently with
  `msiexec /i … ALLUSERS=2 MSIINSTALLPERUSER=1`: `%LOCALAPPDATA%\Programs\Redrob CAD`, HKCU, no
  administrator rights. States: Install (with an opt-in "Also install Redrob Code") → Preparing →
  [Installing Redrob Code] → Installing → Opening Redrob CAD, plus an error state with Retry.
- **Uninstall:** the same app built without a payload, `redrob-cad-uninstall.exe`, ships in the
  install folder. Settings > Apps opens it (the MSI writes that entry and hides Windows
  Installer's own). It asks, optionally deletes the settings folder (`%APPDATA%\Redrob CAD`), and
  runs `msiexec /x`. Drawings are never touched.
- **Redrob CAD open:** install and removal stop and ask the person to save and close it, rather
  than closing it for them.
- **Language:** Korean when Windows is (Wanted Sans), English otherwise (Pretendard); `--lang`.
- **Design:** the design system's deep ground, Redrob Blue progress, the Redrob lockup with "CAD"
  after it (Redrob CAD has no product color). Art and fonts are committed in `res/`.

## Build

`package.ps1` builds both exes; by hand, from a machine with Visual Studio's C++ toolset:

```
python packaging\windows\setup\build.py --arch x64 --out out --name redrob-cad-uninstall.exe --version 1.2.3
python packaging\windows\setup\build.py --arch x64 --out out --name redrob-cad-1.2.3-windows-x64-setup.exe ^
    --version 1.2.3 --payload redrob-cad-1.2.3-windows-x64.msi
```

## Try it without installing

- `--demo` — the install screens on a timer after you press Install. Installs nothing.
- `--demo-error` — the error state.
- `--demo-code-error` — the demo, ending on "Redrob Code was not installed".
- `--demo-uninstall` — the uninstall screens on a timer. Removes nothing.

`ci-check.ps1` takes screenshots of those screens and runs a real per-user MSI install and
removal; the release workflow runs it on x64.
