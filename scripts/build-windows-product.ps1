param(
  [string]$Configuration = "release"
)

$ErrorActionPreference = "Stop"
$Root = Resolve-Path (Join-Path $PSScriptRoot "..")
$UiDir = Join-Path $Root "apps/desktop-ui"
$OutDir = Join-Path $Root "target/product/windows/Wormhole"
$CargoProfileFlag = if ($Configuration -eq "release") { "--release" } else { "" }
$BinDir = if ($Configuration -eq "release") { "release" } else { "debug" }

Push-Location $UiDir
npm install
npm run build
Pop-Location

Push-Location $Root
if ($CargoProfileFlag) {
  cargo build --package wormhole-daemon --package wormhole-desktop $CargoProfileFlag
} else {
  cargo build --package wormhole-daemon --package wormhole-desktop
}
Pop-Location

Remove-Item -LiteralPath $OutDir -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $OutDir | Out-Null
New-Item -ItemType Directory -Force (Join-Path $OutDir "config") | Out-Null
New-Item -ItemType Directory -Force (Join-Path $OutDir "assets") | Out-Null
Copy-Item (Join-Path $Root "target/$BinDir/wormhole-desktop.exe") (Join-Path $OutDir "Wormhole.exe")
Copy-Item (Join-Path $Root "target/$BinDir/wormhole-daemon.exe") (Join-Path $OutDir "wormhole-daemon.exe")
Copy-Item (Join-Path $UiDir "dist") (Join-Path $OutDir "web") -Recurse
Copy-Item (Join-Path $Root "assets/wormhole/wormhole.ico") (Join-Path $OutDir "assets/wormhole.ico")
New-Item -ItemType Directory -Force (Join-Path $OutDir "scripts") | Out-Null
Copy-Item (Join-Path $Root "scripts/install-windows-firewall-rule.ps1") (Join-Path $OutDir "scripts/install-windows-firewall-rule.ps1")
if (Test-Path (Join-Path $Root ".wormhole/windows/config.json")) {
  Copy-Item (Join-Path $Root ".wormhole/windows/config.json") (Join-Path $OutDir "config/config.json")
}
Set-Content -LiteralPath (Join-Path $OutDir "README_START.txt") -Value @"
Double-click Wormhole.exe to start Wormhole.
The launcher starts wormhole-daemon.exe, shows the system tray menu, and opens the control center.

Wormhole starts the daemon first and keeps firewall repair as a manual diagnostic action.
- The recommended rule only allows connections from the Local Subnet (LocalSubnet) on Private or Domain-authenticated networks.
- It will NOT turn off your Windows firewall, and it will NOT allow connections from Public networks.
- If the peer Mac computer still shows "peer_offline", ensure that your current Windows network category is set to "Private" (专用网络) instead of "Public".

If you ever need to manually install the firewall rule, run this in an Administrator PowerShell:
powershell -ExecutionPolicy Bypass -File scripts/install-windows-firewall-rule.ps1 -DaemonPath "$OutDir\wormhole-daemon.exe"
"@

$Desktop = [Environment]::GetFolderPath("Desktop")
if ($Desktop) {
  $ShortcutPath = Join-Path $Desktop "Wormhole.lnk"
  $Shell = New-Object -ComObject WScript.Shell
  $Shortcut = $Shell.CreateShortcut($ShortcutPath)
  $Shortcut.TargetPath = Join-Path $OutDir "Wormhole.exe"
  $Shortcut.WorkingDirectory = $OutDir
  $Shortcut.IconLocation = "$($Shortcut.TargetPath),0"
  $Shortcut.Save()
  Write-Host "Desktop shortcut: $ShortcutPath"
}

Write-Host "Wormhole product output: $OutDir"
