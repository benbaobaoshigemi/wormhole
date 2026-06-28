use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::{json, Value};
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
pub struct DiagnosticsLocal {
    pub daemon_path: String,
    pub config_path: String,
    pub network_profile: String,
    pub firewall_status: String,
}

#[derive(Debug, Deserialize)]
pub struct Device {
    pub device_name: String,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub receive_dir: String,
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
        let payload = json!({ "paths": paths });
        self.post_value("/local/transfer/send", payload)?;
        Ok(())
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
