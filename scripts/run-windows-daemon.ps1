$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Config = Join-Path $Root ".wormhole\windows\config.json"
if (-not (Test-Path -LiteralPath $Config)) {
  & (Join-Path $PSScriptRoot "init-mvp-configs.ps1")
}
$VsDevCmd = "C:\Program Files\Microsoft Visual Studio\18\Community\Common7\Tools\VsDevCmd.bat"
$Cargo = "C:\Users\zhang\.cargo\bin\cargo.exe"
cmd /c "`"$VsDevCmd`" -arch=x64 -host_arch=x64 && `"$Cargo`" run -p wormhole-daemon -- --config `"$Config`""

