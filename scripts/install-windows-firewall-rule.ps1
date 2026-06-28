param(
  [string]$DaemonPath = "",
  [string]$RuleName = "Wormhole Daemon (Private LAN)"
)

$ErrorActionPreference = "Stop"

# 1. Ensure run as Administrator
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
  Write-Error "This script must be run as Administrator."
  exit 1
}

# 2. Resolve Daemon Path
if (-not $DaemonPath) {
  $DaemonPath = Join-Path $PSScriptRoot "wormhole-daemon.exe"
}

if (-not (Test-Path -Path $DaemonPath -PathType Leaf)) {
  Write-Error "Daemon executable not found at path: $DaemonPath"
  exit 1
}

$resolved = (Resolve-Path -LiteralPath $DaemonPath).Path
Write-Host "Resolved Daemon Path: $resolved"

# 3. Clean up historical Wormhole inbound rules
Write-Host "Cleaning up historical Wormhole inbound firewall rules..."
$oldRules = Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object {
  ($_.DisplayName -like "*Wormhole*" -or $_.Name -like "*Wormhole*") -and $_.Direction -eq "Inbound"
}

foreach ($rule in $oldRules) {
  Write-Host "Removing old rule: $($rule.DisplayName) ($($rule.Name))"
  Remove-NetFirewallRule -Name $rule.Name
}

# 4. Remove Inbound Block rules for wormhole-daemon.exe
Write-Host "Checking for any Inbound Block rules for wormhole-daemon.exe..."
$allFilters = Get-NetFirewallApplicationFilter -ErrorAction SilentlyContinue
foreach ($filter in $allFilters) {
  if ($filter.Program -and $filter.Program -like "*wormhole-daemon.exe") {
    $rule = Get-NetFirewallRule -AssociatedNetFirewallApplicationFilter $filter -ErrorAction SilentlyContinue
    if ($rule -and $rule.Direction -eq "Inbound" -and $rule.Action -eq "Block") {
      Write-Host "Removing/Disabling Block rule: $($rule.DisplayName) associated with $($filter.Program)"
      Remove-NetFirewallRule -Name $rule.Name
    }
  }
}

# 5. Install new Allow rule
Write-Host "Installing new firewall rule: $RuleName"
New-NetFirewallRule `
  -DisplayName $RuleName `
  -Direction Inbound `
  -Action Allow `
  -Program $resolved `
  -Profile Private `
  -Protocol TCP `
  -RemoteAddress LocalSubnet `
  -ErrorAction Stop | Out-Null

# 6. Output new rule details
Write-Host "Installed Firewall Rule Details:"
$newRule = Get-NetFirewallRule -DisplayName $RuleName -ErrorAction SilentlyContinue
if ($newRule) {
  $filter = Get-NetFirewallApplicationFilter -AssociatedNetFirewallRule $newRule -ErrorAction SilentlyContinue
  $addressFilter = Get-NetFirewallAddressFilter -AssociatedNetFirewallRule $newRule -ErrorAction SilentlyContinue
  $portFilter = Get-NetFirewallPortFilter -AssociatedNetFirewallRule $newRule -ErrorAction SilentlyContinue

  [PSCustomObject]@{
    DisplayName   = $newRule.DisplayName
    Enabled       = $newRule.Enabled
    Direction     = $newRule.Direction
    Action        = $newRule.Action
    Profile       = $newRule.Profile
    Program       = $filter.Program
    Protocol      = $portFilter.Protocol
    LocalPort     = $portFilter.LocalPort
    RemoteAddress = $addressFilter.RemoteAddress
  } | Format-List
} else {
  Write-Error "Failed to verify new firewall rule installation."
  exit 1
}

# 7. Check NetworkCategory and warn
$profiles = Get-NetConnectionProfile -ErrorAction SilentlyContinue
$hasPrivate = $false
foreach ($p in $profiles) {
  Write-Host "Network connection profile: $($p.Name) -> $($p.NetworkCategory)"
  if ($p.NetworkCategory -eq "Private" -or $p.NetworkCategory -eq "DomainAuthenticated") {
    $hasPrivate = $true
  }
}

if (-not $hasPrivate) {
  Write-Warning "[WARNING] Current network category is not Private."
  Write-Warning "Wormhole firewall rule only allows Private profile and LocalSubnet, so the peer Mac may still fail to connect to this machine under the current network profile."
  Write-Warning "Please change your active network profile to 'Private' in Windows settings."
} else {
  Write-Host "Firewall rule successfully active on Private/Domain networks."
}

Write-Host "Wormhole firewall installation completed successfully."
