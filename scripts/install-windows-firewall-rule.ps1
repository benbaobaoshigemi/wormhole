param(
  [string]$DaemonPath = ""
)

$ErrorActionPreference = "Stop"

if (-not $DaemonPath) {
  $DaemonPath = Join-Path (Resolve-Path (Join-Path $PSScriptRoot "..")).Path "target/product/windows/Wormhole/wormhole-daemon.exe"
}

$resolved = (Resolve-Path -LiteralPath $DaemonPath).Path

Get-NetFirewallApplicationFilter |
  Where-Object { $_.Program -and ((Resolve-Path -LiteralPath $_.Program -ErrorAction SilentlyContinue).Path -eq $resolved -or $_.Program -like "*wormhole-daemon.exe") } |
  ForEach-Object {
    $rule = Get-NetFirewallRule -AssociatedNetFirewallApplicationFilter $_
    if ($rule.Direction -eq "Inbound" -and $rule.Action -eq "Block") {
      Disable-NetFirewallRule -Name $rule.Name
    }
  }

if (-not (Get-NetFirewallRule -DisplayName "Wormhole daemon inbound" -ErrorAction SilentlyContinue)) {
  New-NetFirewallRule `
    -DisplayName "Wormhole daemon inbound" `
    -Direction Inbound `
    -Action Allow `
    -Program $resolved `
    -Profile Private `
    -Protocol TCP `
    -LocalPort Any | Out-Null
}

Write-Host "Wormhole firewall rule installed for $resolved"
