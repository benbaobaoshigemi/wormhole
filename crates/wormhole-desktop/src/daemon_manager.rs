use anyhow::{anyhow, Context, Result};
use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
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
        let config_path = config_path(&product_dir);
        ensure_config(&config_path)?;
        let config = AppConfig::load(&config_path)?;
        let client = LocalApiClient::new(config.port);
        let log_dir = product_dir.join("logs");
        fs::create_dir_all(&log_dir)?;

        let mut manager = Self {
            config_path,
            log_dir,
            child: None,
            client,
        };

        // 1. 检查是否有已运行的 daemon 实例。
        let target_daemon_exe = daemon_path()?;
        let mut resolved_target_daemon = std::fs::canonicalize(&target_daemon_exe)
            .unwrap_or(target_daemon_exe.clone());

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

                // 等待让它退出
                std::thread::sleep(Duration::from_millis(1500));
            }
        }

        // 2. 检查防火墙规则（仅在 Windows 上）
        #[cfg(windows)]
        {
            let check_result = query_firewall_status_sync(&resolved_target_daemon);
            if check_result == "missing_rule"
                || check_result == "stale_program_path"
                || check_result == "blocked_by_rule"
            {
                // 需要修复防火墙
                let description = format!(
                    "Wormhole 需要允许局域网内另一台电脑连接本机 daemon。\n\n我们将请求管理员权限添加仅限专用网络和本地子网的入站规则。\n\n这不会关闭防火墙，也不会开放公用网络。"
                );

                let confirm = rfd::MessageDialog::new()
                    .set_title("Wormhole 防火墙网络授权")
                    .set_description(&description)
                    .set_buttons(rfd::MessageButtons::OkCancel)
                    .set_level(rfd::MessageLevel::Warning)
                    .show();

                if matches!(
                    confirm,
                    rfd::MessageDialogResult::Ok | rfd::MessageDialogResult::Yes
                ) {
                    // 用户确认了，用 UAC 提权执行防火墙脚本
                    let ps_script = product_dir
                        .join("scripts")
                        .join("install-windows-firewall-rule.ps1");
                    let ps_script = if ps_script.is_file() {
                        ps_script
                    } else {
                        // 备用开发路径
                        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("../../scripts/install-windows-firewall-rule.ps1")
                    };

                    let daemon_arg = resolved_target_daemon.to_string_lossy();
                    let args = format!(
                        "-NoProfile -ExecutionPolicy Bypass -File \"{}\" -DaemonPath \"{}\"",
                        ps_script.to_string_lossy(),
                        daemon_arg
                    );

                    let _ = std::process::Command::new("powershell")
                        .args(&[
                            "-NoProfile",
                            "-Command",
                            &format!(
                                "Start-Process powershell -Verb RunAs -Wait -ArgumentList '{}'",
                                args.replace('\'', "''")
                            ),
                        ])
                        .output();

                    // 等待修复生效（最多轮询 8 秒）
                    let deadline = std::time::Instant::now() + Duration::from_secs(8);
                    while std::time::Instant::now() < deadline {
                        let current_status = query_firewall_status_sync(&resolved_target_daemon);
                        if current_status == "ok" {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(500));
                    }
                } else {
                    eprintln!("User cancelled firewall UAC repair.");
                }
            }
        }

        if manager.client.state().is_err() {
            manager.launch_daemon()?;
        }
        manager.wait_until_ready()?;
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
    }

    fn launch_daemon(&mut self) -> Result<()> {
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

fn config_path(product_dir: &Path) -> PathBuf {
    std::env::var_os("WORMHOLE_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| product_dir.join("config").join("config.json"))
}

fn ensure_config(path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    let config = AppConfig::default_at(path, 53_000 + 317, "127.0.0.1".to_string(), 53318)?;
    config.save(path)
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
