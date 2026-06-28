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
Copy-Item (Join-Path $Root "target/$BinDir/wormhole-desktop.exe") (Join-Path $OutDir "Wormhole.exe")
Copy-Item (Join-Path $Root "target/$BinDir/wormhole-daemon.exe") (Join-Path $OutDir "wormhole-daemon.exe")
Copy-Item (Join-Path $UiDir "dist") (Join-Path $OutDir "web") -Recurse
if (Test-Path (Join-Path $Root ".wormhole/windows/config.json")) {
  Copy-Item (Join-Path $Root ".wormhole/windows/config.json") (Join-Path $OutDir "config/config.json")
}
Set-Content -LiteralPath (Join-Path $OutDir "README_START.txt") -Value @"
Double-click Wormhole.exe to start Wormhole.
The launcher starts wormhole-daemon.exe, shows the system tray menu, and opens http://127.0.0.1:<local_port>/.
Send files and folders from the tray menu so Wormhole receives real local paths.
If the peer stays offline from another computer, run this once in an Administrator PowerShell:
powershell -ExecutionPolicy Bypass -File scripts/install-windows-firewall-rule.ps1 -DaemonPath "$OutDir\wormhole-daemon.exe"
"@

Write-Host "Wormhole product output: $OutDir"
