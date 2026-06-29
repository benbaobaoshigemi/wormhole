# Windows Network Diagnosis for Wormhole Product

$ErrorActionPreference = "Continue"

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "     Wormhole Windows Network Diagnosis   " -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan

# 1. 当前 NetworkCategory
Write-Host "`n[1/10] Network Categories:" -ForegroundColor Yellow
$profiles = Get-NetConnectionProfile -ErrorAction SilentlyContinue
if ($profiles) {
  foreach ($p in $profiles) {
    Write-Host "Profile Name: $($p.Name) | NetworkCategory: $($p.NetworkCategory)"
  }
} else {
  Write-Host "No active network profiles detected." -ForegroundColor Red
}

# 2. Wormhole.exe 路径
Write-Host "`n[2/10] Wormhole Launcher Path:" -ForegroundColor Yellow
$launcher = Get-Command "Wormhole.exe" -ErrorAction SilentlyContinue
if ($launcher) {
  Write-Host "Launcher found in PATH: $($launcher.Source)"
} else {
  $prodLauncher = "$PSScriptRoot/../target/product/windows/Wormhole/Wormhole.exe"
  if (Test-Path $prodLauncher) {
    Write-Host "Product Launcher path: $((Resolve-Path $prodLauncher).Path)"
  } else {
    Write-Host "Wormhole.exe not found in environment." -ForegroundColor Red
  }
}

# 3. wormhole-daemon.exe 实际运行路径 & command line
Write-Host "`n[3/10] Running wormhole-daemon.exe processes:" -ForegroundColor Yellow
$processes = Get-CimInstance Win32_Process -Filter "name='wormhole-daemon.exe'" -ErrorAction SilentlyContinue
if ($processes) {
  foreach ($proc in $processes) {
    Write-Host "Process ID      : $($proc.ProcessId)"
    Write-Host "Executable Path : $($proc.ExecutablePath)"
    Write-Host "Command Line    : $($proc.CommandLine)"
    Write-Host "----------------------------------"
  }
} else {
  Write-Host "No running wormhole-daemon.exe processes found." -ForegroundColor Red
}

# 4. Config & daemon settings
Write-Host "`n[4/10] Config File & Host Settings:" -ForegroundColor Yellow
$configPath = $null
if ($processes) {
  $cmdLine = $processes[0].CommandLine
  if ($cmdLine -match '--config\s+"?([^"]+)"?') {
    $configPath = $Matches[1]
  }
}

if (-not $configPath) {
  $defaultConfig = "$PSScriptRoot/../target/product/windows/Wormhole/config/config.json"
  if ($defaultConfig -and (Test-Path $defaultConfig)) {
    $configPath = (Resolve-Path $defaultConfig).Path
  }
}

if ($configPath -and (Test-Path $configPath)) {
  Write-Host "Using config path: $configPath"
  try {
    $json = Get-Content $configPath -Raw | ConvertFrom-Json
    Write-Host "Device ID      : $($json.device_id)"
    Write-Host "Device Name    : $($json.device_name)"
    Write-Host "Bind Host      : $($json.bind_host)"
    Write-Host "Local Port     : $($json.port)"
    Write-Host "Peer Host      : $($json.peer.host)"
    Write-Host "Peer Port      : $($json.peer.port)"
    Write-Host "Receive Dir    : $($json.receive_dir)"
    $localPort = $json.port
  } catch {
    Write-Host "Failed to parse JSON config: $_" -ForegroundColor Red
    $localPort = $null
  }
} else {
  Write-Host "Config file not found or could not be determined." -ForegroundColor Red
  $localPort = $null
}

# 5. netstat -ano 监听情况
Write-Host "`n[5/10] Netstat Port Listening Status:" -ForegroundColor Yellow
if ($localPort) {
  $netstat = netstat -ano | findstr LISTENING | findstr "$localPort"
  if ($netstat) {
    Write-Host $netstat
  } else {
    Write-Host "No processes detected listening on local port $localPort." -ForegroundColor Red
  }
} else {
  Write-Host "Local port is unknown because no readable config was found." -ForegroundColor Red
}

# 6. Windows 防火墙规则 - Wormhole 规则
Write-Host "`n[6/10] Wormhole Firewall Rules:" -ForegroundColor Yellow
$whRules = Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object {
  $_.DisplayName -like "*Wormhole*" -or $_.DisplayName -like "*wormhole*"
}
if ($whRules) {
  foreach ($rule in $whRules) {
    $filter = Get-NetFirewallApplicationFilter -AssociatedNetFirewallRule $rule -ErrorAction SilentlyContinue
    $addr = Get-NetFirewallAddressFilter -AssociatedNetFirewallRule $rule -ErrorAction SilentlyContinue
    $port = Get-NetFirewallPortFilter -AssociatedNetFirewallRule $rule -ErrorAction SilentlyContinue
    Write-Host "DisplayName   : $($rule.DisplayName)"
    Write-Host "Enabled       : $($rule.Enabled)"
    Write-Host "Direction     : $($rule.Direction)"
    Write-Host "Action        : $($rule.Action)"
    Write-Host "Profile       : $($rule.Profile)"
    Write-Host "Program       : $($filter.Program)"
    Write-Host "Protocol      : $($port.Protocol)"
    Write-Host "LocalPort     : $($port.LocalPort)"
    Write-Host "RemoteAddress : $($addr.RemoteAddress)"
    Write-Host "----------------------------------"
  }
} else {
  Write-Host "No specific Wormhole firewall rules found." -ForegroundColor Red
}

# 7. 所有包含 wormhole-daemon.exe 的 Inbound Allow/Block 规则
Write-Host "`n[7/10] All Inbound Rules for wormhole-daemon.exe (Allow & Block):" -ForegroundColor Yellow
$allInboundFilters = Get-NetFirewallApplicationFilter -ErrorAction SilentlyContinue
$foundDaemonRule = $false
foreach ($filter in $allInboundFilters) {
  if ($filter.Program -and $filter.Program -like "*wormhole-daemon.exe") {
    $rule = Get-NetFirewallRule -AssociatedNetFirewallApplicationFilter $filter -ErrorAction SilentlyContinue
    if ($rule -and $rule.Direction -eq "Inbound") {
      $foundDaemonRule = $true
      $addr = Get-NetFirewallAddressFilter -AssociatedNetFirewallRule $rule -ErrorAction SilentlyContinue
      $port = Get-NetFirewallPortFilter -AssociatedNetFirewallRule $rule -ErrorAction SilentlyContinue
      Write-Host "DisplayName   : $($rule.DisplayName) ($($rule.Name))"
      Write-Host "Action        : $($rule.Action)"
      Write-Host "Enabled       : $($rule.Enabled)"
      Write-Host "Profile       : $($rule.Profile)"
      Write-Host "Program       : $($filter.Program)"
      Write-Host "Protocol      : $($port.Protocol)"
      Write-Host "RemoteAddress : $($addr.RemoteAddress)"
      Write-Host "----------------------------------"
    }
  }
}
if (-not $foundDaemonRule) {
  Write-Host "No inbound firewall rules found referencing wormhole-daemon.exe." -ForegroundColor Red
}

# 8. http://127.0.0.1:<local_port>/local/state 测试
Write-Host "`n[8/10] Local API State Test (http://127.0.0.1:$localPort/local/state):" -ForegroundColor Yellow
if ($localPort) {
  try {
    $response = Invoke-RestMethod -Uri "http://127.0.0.1:$localPort/local/state" -Method Get -TimeoutSec 3
    Write-Host "Local API Status: Online (200 OK)"
    Write-Host "Device ID       : $($response.device.device_id)"
    Write-Host "Device Name     : $($response.device.device_name)"
    Write-Host "Status          : $($response.status)"
    if ($response.peer) {
      Write-Host "Peer Connected  : $($response.peer.device_name) ($($response.peer.host):$($response.peer.port))"
    } else {
      Write-Host "Peer Connected  : None"
    }
  } catch {
    Write-Host "Failed to request local state API: $_" -ForegroundColor Red
  }
} else {
  Write-Host "Skipped because local port is unknown." -ForegroundColor Red
}

# 9. 本机 LAN IP
Write-Host "`n[9/10] Local LAN IP Addresses:" -ForegroundColor Yellow
$lanIps = Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object {
  $_.IPAddress -notlike "127.*" -and $_.IPAddress -notlike "169.254.*"
}
if ($lanIps) {
  foreach ($ip in $lanIps) {
    Write-Host "Interface: $($ip.InterfaceAlias) | IPAddress: $($ip.IPAddress)"
  }
} else {
  Write-Host "Failed to get local IPv4 via Get-NetIPAddress." -ForegroundColor Red
}

# 10. 建议从 Mac 执行的 curl
Write-Host "`n[10/10] Recommended Command to Run on Mac Peer:" -ForegroundColor Yellow
if ($lanIps) {
  $primaryIp = $lanIps[0].IPAddress
  Write-Host "Run the following command on your Mac to test connection to this Windows machine:"
  if ($localPort) {
    Write-Host "curl -v http://$primaryIp`:$localPort/peer/handshake" -ForegroundColor Green
  } else {
    Write-Host "Local port is unknown; read config/config.json first." -ForegroundColor Red
  }
} else {
  Write-Host "No active LAN IP address found to suggest test curl command." -ForegroundColor Red
}

Write-Host "`n==========================================" -ForegroundColor Cyan
Write-Host "          Diagnosis Complete              " -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan
