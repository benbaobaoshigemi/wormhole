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
