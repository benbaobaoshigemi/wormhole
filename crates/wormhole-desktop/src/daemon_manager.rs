use anyhow::{anyhow, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};
#[cfg(not(target_os = "macos"))]
use std::{fs::OpenOptions, process::Stdio};
use wormhole_core::AppConfig;

use crate::local_api_client::LocalApiClient;

pub struct DaemonManager {
    config_path: PathBuf,
    log_dir: PathBuf,
    child: Option<Child>,
    client: LocalApiClient,
}

impl DaemonManager {
    pub fn start_or_attach() -> Result<Self> {
        let product_dir = product_dir()?;
        let config_path = config_path(&product_dir)?;
        ensure_config(&product_dir, &config_path)?;
        let config = AppConfig::load(&config_path)?;
        let client = LocalApiClient::new(config.port);
        let log_dir = log_dir(&product_dir)?;
        fs::create_dir_all(&log_dir)?;

        let mut manager = Self {
            config_path,
            log_dir,
            child: None,
            client,
        };

        // 1. 检查是否有已运行的 daemon 实例。
        let target_daemon_exe = daemon_path()?;
        let mut resolved_target_daemon =
            std::fs::canonicalize(&target_daemon_exe).unwrap_or(target_daemon_exe.clone());

        let r_str = resolved_target_daemon.to_string_lossy();
        if r_str.starts_with(r"\\?\") {
            resolved_target_daemon = std::path::PathBuf::from(&r_str[4..]);
        }

        if let Ok(state) = manager.client.state() {
            // 已有 daemon 运行！检查路径是否一致
            let is_same = if let Some(diag) = &state.diagnostics {
                let running_daemon = std::path::PathBuf::from(&diag.daemon_path);
                let resolved_running =
                    std::fs::canonicalize(&running_daemon).unwrap_or(running_daemon);
                resolved_running == resolved_target_daemon
            } else {
                false
            };

            if !is_same {
                // 不一致，必须停止/杀掉它以防冲突！
                #[cfg(windows)]
                {
                    // 在 Windows 上通过 Stop-Process
                    let _ = std::process::Command::new("powershell")
                        .args(&[
                            "-NoProfile",
                            "-Command",
                            &format!(
                                "Get-Process wormhole-daemon -ErrorAction SilentlyContinue | Where-Object {{ $_.Path -ne '{}' }} | Stop-Process -Force",
                                resolved_target_daemon.to_string_lossy().replace('\'', "''")
                            )
                        ])
                        .output();
                }
                #[cfg(target_os = "macos")]
                {
                    let _ = stop_running_daemon(&manager.client);
                }

                // 等待让它退出
                std::thread::sleep(Duration::from_millis(1500));
            }
        }

        if manager.client.state().is_err() {
            manager.launch_daemon()?;
        }
        manager.wait_until_ready()?;
        #[cfg(windows)]
        {
            let firewall_status = query_firewall_status_sync(&resolved_target_daemon);
            if firewall_status != "ok" && firewall_status != "unknown" {
                eprintln!(
                    "Wormhole firewall needs attention: status={firewall_status}, daemon={}",
                    resolved_target_daemon.display()
                );
            }
        }
        manager.check_peer_reachability();
        Ok(manager)
    }

    pub fn client(&self) -> LocalApiClient {
        self.client.clone()
    }

    pub fn control_center_url(&self) -> String {
        self.client.control_center_url()
    }

    pub fn log_dir(&self) -> &Path {
        &self.log_dir
    }

    pub fn restart(&mut self) -> Result<()> {
        self.stop();
        self.launch_daemon()?;
        self.wait_until_ready()
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = stop_running_daemon(&self.client);
    }

    pub fn quit_all(&mut self) {
        self.stop();
    }

    fn launch_daemon(&mut self) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            self.launch_daemon_with_launch_agent()?;
            return Ok(());
        }

        #[cfg(not(target_os = "macos"))]
        {
            let daemon = daemon_path()?;
            let stdout = OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.log_dir.join("daemon.out.log"))?;
            let stderr = OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.log_dir.join("daemon.err.log"))?;
            let child = Command::new(&daemon)
                .arg("--config")
                .arg(&self.config_path)
                .stdout(Stdio::from(stdout))
                .stderr(Stdio::from(stderr))
                .spawn()
                .with_context(|| format!("start daemon {}", daemon.display()))?;
            self.child = Some(child);
            Ok(())
        }
    }

    #[cfg(target_os = "macos")]
    fn launch_daemon_with_launch_agent(&mut self) -> Result<()> {
        let daemon = daemon_path()?;
        let plist = launch_agent_plist()?;
        if let Some(parent) = plist.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir_all(&self.log_dir)?;
        unload_launch_agent(&plist)?;

        let stdout = self.log_dir.join("daemon.out.log");
        let stderr = self.log_dir.join("daemon.err.log");
        let plist_body = launch_agent_plist_body(
            &daemon,
            &self.config_path,
            &product_dir()?,
            &stdout,
            &stderr,
        );
        fs::write(&plist, plist_body)?;
        let domain = launchctl_domain()?;
        let output = Command::new("launchctl")
            .args(["bootstrap", &domain, &plist.to_string_lossy()])
            .output()
            .context("bootstrap Wormhole daemon LaunchAgent")?;
        if !output.status.success() {
            return Err(anyhow!(
                "launchctl bootstrap failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(())
    }

    fn wait_until_ready(&self) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(12);
        while Instant::now() < deadline {
            if self.client.state().is_ok() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(250));
        }
        Err(anyhow!(
            "daemon did not become ready; port may be occupied or config is invalid"
        ))
    }

    fn check_peer_reachability(&self) {
        if self.client.connect().is_ok() {
            return;
        }

        let Ok(state) = self.client.state() else {
            return;
        };
        let Some(diagnostics) = state.diagnostics else {
            return;
        };
        let Some(error) = diagnostics.last_handshake_error else {
            return;
        };

        #[cfg(target_os = "macos")]
        if looks_like_local_network_privacy_denial(&error) {
            eprintln!(
                "macOS local network access appears blocked for Wormhole; peer={}:{} error={}",
                state.settings.peer_host, state.settings.peer_port, error
            );
            let _ = open::that(
                "x-apple.systempreferences:com.apple.preference.security?Privacy_LocalNetwork",
            );
        }
        #[cfg(not(target_os = "macos"))]
        let _ = error;
    }
}

impl Drop for DaemonManager {
    fn drop(&mut self) {
        self.stop();
    }
}

fn product_dir() -> Result<PathBuf> {
    Ok(std::env::current_exe()?
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or(std::env::current_dir()?))
}

fn config_path(_product_dir: &Path) -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("WORMHOLE_CONFIG") {
        return Ok(PathBuf::from(path));
    }
    #[cfg(target_os = "macos")]
    {
        return Ok(app_support_dir()?.join("config.json"));
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(_product_dir.join("config").join("config.json"))
    }
}

fn ensure_config(product_dir: &Path, path: &Path) -> Result<()> {
    if path.exists() {
        normalize_runtime_config(path)?;
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bundled = product_dir.join("config").join("config.json");
    if bundled.is_file() {
        fs::copy(&bundled, path).with_context(|| {
            format!(
                "copy bundled config {} to {}",
                bundled.display(),
                path.display()
            )
        })?;
        normalize_runtime_config(path)?;
        return Ok(());
    }
    let config = AppConfig::default_at(path, 53_000 + 317, "127.0.0.1".to_string(), 53318)?;
    config.save(path)?;
    normalize_runtime_config(path)
}

fn log_dir(_product_dir: &Path) -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        return Ok(home_dir()?.join("Library").join("Logs").join("Wormhole"));
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(_product_dir.join("logs"))
    }
}

fn normalize_runtime_config(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let support = app_support_dir()?;
        let mut config = AppConfig::load(path)?;
        let receive_dir = support.join("Received");
        let data_dir = support.join("Data");
        if config.receive_dir != receive_dir || config.data_dir != data_dir {
            config.receive_dir = receive_dir;
            config.data_dir = data_dir;
            config.save(path)?;
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
    }
    Ok(())
}

fn daemon_path() -> Result<PathBuf> {
    let exe_name = if cfg!(windows) {
        "wormhole-daemon.exe"
    } else {
        "wormhole-daemon"
    };
    let beside_launcher = product_dir()?.join(exe_name);
    if beside_launcher.is_file() {
        return Ok(beside_launcher);
    }
    let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/debug")
        .join(exe_name);
    if dev_path.is_file() {
        return Ok(dev_path);
    }
    Err(anyhow!("bundled daemon not found next to launcher"))
}

#[cfg(target_os = "macos")]
fn looks_like_local_network_privacy_denial(error: &str) -> bool {
    error.contains("No route to host") || error.contains("os error 65")
}

#[cfg(target_os = "macos")]
fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("HOME is not set"))
}

#[cfg(target_os = "macos")]
fn app_support_dir() -> Result<PathBuf> {
    Ok(home_dir()?
        .join("Library")
        .join("Application Support")
        .join("Wormhole"))
}

#[cfg(target_os = "macos")]
fn launch_agent_plist() -> Result<PathBuf> {
    Ok(home_dir()?
        .join("Library")
        .join("LaunchAgents")
        .join("dev.wormhole.daemon.plist"))
}

#[cfg(target_os = "macos")]
fn launch_agent_plist_body(
    daemon: &Path,
    config: &Path,
    working_dir: &Path,
    stdout: &Path,
    stderr: &Path,
) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>dev.wormhole.daemon</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
    <string>--config</string>
    <string>{}</string>
  </array>
  <key>WorkingDirectory</key><string>{}</string>
  <key>RunAtLoad</key><true/>
  <key>StandardOutPath</key><string>{}</string>
  <key>StandardErrorPath</key><string>{}</string>
</dict>
</plist>
"#,
        plist_escape(&daemon.to_string_lossy()),
        plist_escape(&config.to_string_lossy()),
        plist_escape(&working_dir.to_string_lossy()),
        plist_escape(&stdout.to_string_lossy()),
        plist_escape(&stderr.to_string_lossy())
    )
}

#[cfg(target_os = "macos")]
fn plist_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(target_os = "macos")]
fn unload_launch_agent(plist: &Path) -> Result<()> {
    if !plist.exists() {
        return Ok(());
    }
    let domain = launchctl_domain()?;
    let _ = Command::new("launchctl")
        .args(["bootout", &domain, &plist.to_string_lossy()])
        .output();
    Ok(())
}

#[cfg(target_os = "macos")]
fn stop_running_daemon(client: &LocalApiClient) -> Result<()> {
    let _ = unload_launch_agent(&launch_agent_plist()?);

    let daemon_path = client
        .state()
        .ok()
        .and_then(|state| state.diagnostics.map(|diag| diag.daemon_path))
        .unwrap_or_else(|| {
            daemon_path()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default()
        });
    if !daemon_path.is_empty() {
        let _ = Command::new("pkill").args(["-f", &daemon_path]).output();
    }
    Ok(())
}

#[cfg(windows)]
fn stop_running_daemon(client: &LocalApiClient) -> Result<()> {
    let daemon_path = client
        .state()
        .ok()
        .and_then(|state| state.diagnostics.map(|diag| diag.daemon_path))
        .unwrap_or_else(|| {
            daemon_path()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default()
        });
    if daemon_path.is_empty() {
        return Ok(());
    }
    let escaped = daemon_path.replace('\'', "''");
    let script = format!(
        "Get-Process wormhole-daemon -ErrorAction SilentlyContinue | Where-Object {{ $_.Path -eq '{}' }} | Stop-Process -Force",
        escaped
    );
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output();
    Ok(())
}

#[cfg(all(not(target_os = "macos"), not(windows)))]
fn stop_running_daemon(_client: &LocalApiClient) -> Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn launchctl_domain() -> Result<String> {
    let output = Command::new("id")
        .arg("-u")
        .output()
        .context("read current user id")?;
    if !output.status.success() {
        return Err(anyhow!("id -u failed"));
    }
    let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(format!("gui/{uid}"))
}

#[cfg(windows)]
fn query_firewall_status_sync(daemon_path: &std::path::Path) -> String {
    let daemon_str = daemon_path.to_string_lossy();
    let script = format!(
        r#"
        $daemonPath = "{}"
        $blockRules = Get-NetFirewallRule -AssociatedNetFirewallApplicationFilter (Get-NetFirewallApplicationFilter -ErrorAction SilentlyContinue | Where-Object {{ $_.Program -like "*wormhole-daemon.exe" }}) -ErrorAction SilentlyContinue | Where-Object {{ $_.Direction -eq "Inbound" -and $_.Action -eq "Block" -and $_.Enabled -eq "True" }}
        if ($blockRules) {{ Write-Output "blocked_by_rule"; exit }}

        $allowRule = Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object {{ $_.DisplayName -like "*Wormhole*" -and $_.Direction -eq "Inbound" -and $_.Action -eq "Allow" -and $_.Enabled -eq "True" }} | Select-Object -First 1
        if (-not $allowRule) {{ Write-Output "missing_rule"; exit }}

        $allowProgram = Get-NetFirewallApplicationFilter -AssociatedNetFirewallRule $allowRule -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Program -First 1
        if (-not $allowProgram) {{ Write-Output "missing_rule"; exit }}

        $resolvedAllow = (Resolve-Path -LiteralPath $allowProgram -ErrorAction SilentlyContinue).Path
        if ($resolvedAllow) {{ $resolvedAllow = $resolvedAllow.Replace("\\?\", "") }}
        $resolvedDaemon = (Resolve-Path -LiteralPath $daemonPath -ErrorAction SilentlyContinue).Path
        if ($resolvedDaemon) {{ $resolvedDaemon = $resolvedDaemon.Replace("\\?\", "") }}
        if ($resolvedAllow -ne $resolvedDaemon) {{ Write-Output "stale_program_path"; exit }}

        Write-Output "ok"
        "#,
        daemon_str.replace('"', "\\\"")
    );

    let output = std::process::Command::new("powershell")
        .args(&["-NoProfile", "-Command", &script])
        .output();

    match output {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Err(_) => "unknown".to_string(),
    }
}
