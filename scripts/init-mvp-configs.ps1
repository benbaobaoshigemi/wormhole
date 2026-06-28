param(
  [string]$MacHost = "192.168.1.180",
  [int]$WindowsPort = (53000 + 317),
  [int]$MacPort = 53318
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$WinRoot = Join-Path $Root ".wormhole\windows"
$MacMirrorRoot = Join-Path $Root ".wormhole\macos"
New-Item -ItemType Directory -Force -Path $WinRoot, $MacMirrorRoot | Out-Null
$SharedToken = [guid]::NewGuid().ToString()

$winConfig = @{
  device_id = [guid]::NewGuid().ToString()
  device_name = "Windows Wormhole"
  platform = "windows"
  bind_host = "0.0.0.0"
  port = $WindowsPort
  peer = @{ name = "macOS Air"; host = $MacHost; port = $MacPort }
  receive_dir = (Join-Path $WinRoot "received")
  data_dir = (Join-Path $WinRoot "data")
  auto_connect = $true
  clipboard = @{ enabled = $true; text_enabled = $true; image_enabled = $true; max_image_bytes = 20971520; poll_millis = 750; remote_hash_window = 128 }
  shared_token = $SharedToken
  transfer = @{ max_concurrent_tasks = 2; conflict_strategy = "rename"; min_free_space_bytes = 67108864; verify_hash = $true; resume_enabled = $true }
  connection = @{ heartbeat_millis = 5000; reconnect_millis = 3000 }
  history_retention_days = 30
  min_peer_protocol_version = 1
  max_peer_protocol_version = 1
  retry_limit = 3
}
$macConfig = @{
  device_id = [guid]::NewGuid().ToString()
  device_name = "macOS Air Wormhole"
  platform = "macos"
  bind_host = "0.0.0.0"
  port = $MacPort
  peer = @{ name = "Windows Wormhole"; host = "WINDOWS_HOST_PLACEHOLDER"; port = $WindowsPort }
  receive_dir = "/Users/benbaobaoshigemi/Desktop/hole/.wormhole/macos/received"
  data_dir = "/Users/benbaobaoshigemi/Desktop/hole/.wormhole/macos/data"
  auto_connect = $true
  clipboard = @{ enabled = $true; text_enabled = $true; image_enabled = $true; max_image_bytes = 20971520; poll_millis = 750; remote_hash_window = 128 }
  shared_token = $SharedToken
  transfer = @{ max_concurrent_tasks = 2; conflict_strategy = "rename"; min_free_space_bytes = 67108864; verify_hash = $true; resume_enabled = $true }
  connection = @{ heartbeat_millis = 5000; reconnect_millis = 3000 }
  history_retention_days = 30
  min_peer_protocol_version = 1
  max_peer_protocol_version = 1
  retry_limit = 3
}

$winConfig | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $WinRoot "config.json") -Encoding UTF8
$macConfig | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $MacMirrorRoot "config.json") -Encoding UTF8

Write-Host "Windows config: $(Join-Path $WinRoot 'config.json')"
Write-Host "macOS config template: $(Join-Path $MacMirrorRoot 'config.json')"
