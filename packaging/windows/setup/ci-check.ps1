<#
.SYNOPSIS
  CI check for the Windows installers: screenshots of the branded setup, and a real per-user
  MSI install and removal.

.DESCRIPTION
  1. Starts the setup's demo screens (welcome in English and Korean, the uninstall question) and
     saves a screenshot of each to $Out. Demo mode installs nothing.
  2. Installs the MSI the way the setup does (msiexec /i ALLUSERS=2 MSIINSTALLPERUSER=1 /qn), then
     checks the files, the App Paths entry and the Settings > Apps entry that opens the branded
     uninstaller, then removes it with msiexec /x and checks that everything is gone.

.EXAMPLE
  pwsh packaging/windows/setup/ci-check.ps1 -Setup dist/release/redrob-cad-0.0.2-windows-x64-setup.exe `
    -Msi dist/release/redrob-cad-0.0.2-windows-x64.msi -Out dist/screens
#>
param(
  [Parameter(Mandatory = $true)] [string] $Setup,
  [Parameter(Mandatory = $true)] [string] $Msi,
  [Parameter(Mandatory = $true)] [string] $Out
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $Out | Out-Null
Add-Type -AssemblyName System.Drawing, System.Windows.Forms

function Save-Screen([string] $Name) {
  $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
  $bmp.Save((Join-Path $Out $Name), [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
  Write-Output "screenshot $Name"
}

# ---- 1. the window --------------------------------------------------------------------------
foreach ($shot in @(
    @{ Name = 'setup-welcome-en.png'; Args = @('--demo', '--lang', 'en') },
    @{ Name = 'setup-welcome-ko.png'; Args = @('--demo', '--lang', 'ko') },
    @{ Name = 'setup-uninstall-en.png'; Args = @('--demo-uninstall', '--lang', 'en') },
    @{ Name = 'setup-uninstall-ko.png'; Args = @('--demo-uninstall', '--lang', 'ko') },
    @{ Name = 'setup-error-en.png'; Args = @('--demo-error', '--lang', 'en') })) {
  $p = Start-Process -FilePath $Setup -ArgumentList $shot.Args -PassThru
  Start-Sleep -Seconds 3
  if ($p.HasExited) { throw "$($shot.Name): the setup exited at once (code $($p.ExitCode))" }
  Save-Screen $shot.Name
  Stop-Process -Id $p.Id -Force
  Start-Sleep -Milliseconds 500
}

# ---- 2. per-user install and removal ----------------------------------------------------------
$log = Join-Path $Out 'msi-install.log'
$p = Start-Process msiexec.exe -ArgumentList @('/i', "`"$((Resolve-Path $Msi).Path)`"", '/qn', '/norestart',
    'ALLUSERS=2', 'MSIINSTALLPERUSER=1', '/l*v', "`"$log`"") -Wait -PassThru
if ($p.ExitCode -ne 0) { throw "msiexec /i exited $($p.ExitCode); see msi-install.log" }

$dir = Join-Path $env:LOCALAPPDATA 'Programs\Redrob CAD'
foreach ($f in 'redrob-cad.exe', 'redrob-cad-cli.exe', 'redrob-cad-uninstall.exe') {
  if (-not (Test-Path (Join-Path $dir $f))) { throw "not installed per-user: $dir\$f" }
}
$app = (Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\App Paths\redrob-cad.exe').'(default)'
if ($app -ne (Join-Path $dir 'redrob-cad.exe')) { throw "App Paths points at '$app'" }
$arp = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\RedrobCAD'
if ($arp.DisplayName -ne 'Redrob CAD') { throw "Settings > Apps entry: DisplayName '$($arp.DisplayName)'" }
if ($arp.UninstallString -notlike '*redrob-cad-uninstall.exe" --uninstall') { throw "UninstallString '$($arp.UninstallString)'" }
Write-Output "installed per-user: $dir; Settings > Apps opens: $($arp.UninstallString)"
& (Join-Path $dir 'redrob-cad-cli.exe') --version
if ($LASTEXITCODE -ne 0) { throw 'the installed CLI did not run' }

$code = (Get-CimInstance -ClassName Win32_Product -Filter "Name = 'Redrob CAD'" -ErrorAction SilentlyContinue).IdentifyingNumber
if (-not $code) {
  # Win32_Product sees per-machine products only on some images; read the code from the install log.
  $code = (Select-String -Path $log -Pattern 'ProductCode = (\{[0-9A-F-]+\})' | Select-Object -First 1).Matches.Groups[1].Value
}
if (-not $code) { throw 'could not find the installed ProductCode' }
$p = Start-Process msiexec.exe -ArgumentList @('/x', $code, '/qn', '/norestart') -Wait -PassThru
if ($p.ExitCode -ne 0) { throw "msiexec /x exited $($p.ExitCode)" }
if (Test-Path (Join-Path $dir 'redrob-cad.exe')) { throw 'redrob-cad.exe is still there after removal' }
if (Test-Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\RedrobCAD') { throw 'the Settings > Apps entry is still there' }
Write-Output 'removed: files and the Settings > Apps entry are gone'
