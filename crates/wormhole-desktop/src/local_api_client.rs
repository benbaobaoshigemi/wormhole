use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::{json, Value};
#[cfg(target_os = "macos")]
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct LocalApiClient {
    base: String,
}

#[derive(Debug, Deserialize)]
pub struct LocalState {
    pub status: String,
    pub peer: Option<Device>,
    pub settings: Settings,
    pub diagnostics: Option<DiagnosticsLocal>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DiagnosticsLocal {
    pub daemon_path: String,
    pub config_path: String,
    pub network_profile: String,
    pub firewall_status: String,
    pub last_handshake_error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Device {
    pub device_name: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Settings {
    pub receive_dir: String,
    pub peer_host: String,
    pub peer_port: u16,
    pub clipboard: ClipboardSettings,
}

#[derive(Debug, Deserialize)]
pub struct ClipboardSettings {
    pub enabled: bool,
}

impl LocalApiClient {
    pub fn new(port: u16) -> Self {
        Self {
            base: format!("http://127.0.0.1:{port}"),
        }
    }

    pub fn control_center_url(&self) -> String {
        format!("{}/", self.base)
    }

    pub fn state(&self) -> Result<LocalState> {
        self.get("/local/state")
    }

    pub fn send_paths(&self, paths: &[PathBuf]) -> Result<()> {
        if paths.is_empty() {
            return Err(anyhow!("no selected paths"));
        }
        let send_paths = prepare_send_paths(paths)?;
        let payload = json!({ "paths": send_paths });
        self.post_value("/local/transfer/send", payload)?;
        Ok(())
    }

    pub fn connect(&self) -> Result<Value> {
        self.post_value("/local/connect", json!({}))
    }

    pub fn enable_clipboard(&self) -> Result<()> {
        self.post_value("/local/clipboard/enable", json!({}))?;
        Ok(())
    }

    pub fn disable_clipboard(&self) -> Result<()> {
        self.post_value("/local/clipboard/disable", json!({}))?;
        Ok(())
    }

    pub fn open_receive_dir(&self) -> Result<()> {
        let state = self.state()?;
        open::that(Path::new(&state.settings.receive_dir))?;
        Ok(())
    }

    fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let response = ureq::get(&format!("{}{}", self.base, path)).call()?;
        Ok(response.into_json::<T>()?)
    }

    fn post_value(&self, path: &str, value: Value) -> Result<Value> {
        let response = ureq::post(&format!("{}{}", self.base, path)).send_json(value)?;
        Ok(response.into_json::<Value>()?)
    }
}

fn prepare_send_paths(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    #[cfg(target_os = "macos")]
    {
        stage_paths_for_daemon(paths)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(paths.to_vec())
    }
}

#[cfg(target_os = "macos")]
fn stage_paths_for_daemon(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let root = macos_app_support_dir()?.join("Outgoing").join(format!(
        "{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    fs::create_dir_all(&root)?;
    let mut staged = Vec::with_capacity(paths.len());
    for path in paths {
        let name = path
            .file_name()
            .ok_or_else(|| anyhow!("selected path has no file name: {}", path.display()))?;
        let target = root.join(name);
        copy_path(path, &target)?;
        staged.push(target);
    }
    Ok(staged)
}

#[cfg(target_os = "macos")]
fn macos_app_support_dir() -> Result<PathBuf> {
    Ok(std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("HOME is not set"))?
        .join("Library")
        .join("Application Support")
        .join("Wormhole"))
}

#[cfg(target_os = "macos")]
fn copy_path(source: &Path, target: &Path) -> Result<()> {
    let meta = fs::metadata(source)?;
    if meta.is_file() {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, target)?;
        return Ok(());
    }
    if meta.is_dir() {
        copy_dir_recursive(source, target)?;
        return Ok(());
    }
    Err(anyhow!("unsupported selected path: {}", source.display()))
}

#[cfg(target_os = "macos")]
fn copy_dir_recursive(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let child_source = entry.path();
        let child_target = target.join(entry.file_name());
        copy_path(&child_source, &child_target)?;
    }
    Ok(())
}
