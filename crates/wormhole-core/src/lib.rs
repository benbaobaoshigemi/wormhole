use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};
use uuid::Uuid;
use walkdir::WalkDir;

pub const PROTOCOL_VERSION: u32 = 1;
pub const DEFAULT_IMAGE_LIMIT_BYTES: u64 = 20 * 1024 * 1024;
pub const DEFAULT_RETRY_LIMIT: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub bind_host: String,
    pub port: u16,
    pub peer: PeerConfig,
    pub receive_dir: PathBuf,
    pub data_dir: PathBuf,
    pub auto_connect: bool,
    pub clipboard: ClipboardSettings,
    #[serde(default)]
    pub shared_token: Option<String>,
    #[serde(default)]
    pub transfer: TransferSettings,
    #[serde(default)]
    pub connection: ConnectionSettings,
    #[serde(default = "default_history_retention_days")]
    pub history_retention_days: u32,
    #[serde(default = "default_protocol_min_version")]
    pub min_peer_protocol_version: u32,
    #[serde(default = "default_protocol_max_version")]
    pub max_peer_protocol_version: u32,
    pub retry_limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardSettings {
    pub enabled: bool,
    pub text_enabled: bool,
    pub image_enabled: bool,
    pub max_image_bytes: u64,
    #[serde(default = "default_clipboard_poll_millis")]
    pub poll_millis: u64,
    #[serde(default = "default_clipboard_remote_hash_window")]
    pub remote_hash_window: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferSettings {
    pub max_concurrent_tasks: usize,
    pub conflict_strategy: ConflictStrategy,
    pub min_free_space_bytes: u64,
    pub verify_hash: bool,
    pub resume_enabled: bool,
}

impl Default for TransferSettings {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 2,
            conflict_strategy: ConflictStrategy::Rename,
            min_free_space_bytes: 64 * 1024 * 1024,
            verify_hash: true,
            resume_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSettings {
    pub heartbeat_millis: u64,
    pub reconnect_millis: u64,
}

impl Default for ConnectionSettings {
    fn default() -> Self {
        Self {
            heartbeat_millis: 5_000,
            reconnect_millis: 3_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictStrategy {
    Rename,
    Overwrite,
    Skip,
}

fn default_clipboard_poll_millis() -> u64 {
    750
}

fn default_clipboard_remote_hash_window() -> usize {
    128
}

fn default_history_retention_days() -> u32 {
    30
}

fn default_protocol_min_version() -> u32 {
    1
}

fn default_protocol_max_version() -> u32 {
    PROTOCOL_VERSION
}

impl AppConfig {
    pub fn default_at(
        config_path: &Path,
        port: u16,
        peer_host: String,
        peer_port: u16,
    ) -> Result<Self> {
        let root = config_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let platform = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "macos"
        } else {
            "unknown"
        };
        let device_name = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| format!("Wormhole {}", platform));
        Ok(Self {
            device_id: Uuid::new_v4().to_string(),
            device_name,
            platform: platform.to_string(),
            bind_host: "0.0.0.0".to_string(),
            port,
            peer: PeerConfig {
                name: "Peer".to_string(),
                host: peer_host,
                port: peer_port,
            },
            receive_dir: root.join("received"),
            data_dir: root.join("data"),
            auto_connect: true,
            clipboard: ClipboardSettings {
                enabled: true,
                text_enabled: true,
                image_enabled: true,
                max_image_bytes: DEFAULT_IMAGE_LIMIT_BYTES,
                poll_millis: default_clipboard_poll_millis(),
                remote_hash_window: default_clipboard_remote_hash_window(),
            },
            shared_token: Some(Uuid::new_v4().to_string()),
            transfer: TransferSettings::default(),
            connection: ConnectionSettings::default(),
            history_retention_days: default_history_retention_days(),
            min_peer_protocol_version: default_protocol_min_version(),
            max_peer_protocol_version: default_protocol_max_version(),
            retry_limit: DEFAULT_RETRY_LIMIT,
        })
    }

    pub fn load(path: &Path) -> Result<Self> {
        let raw =
            fs::read_to_string(path).with_context(|| format!("read config {}", path.display()))?;
        serde_json::from_str(raw.trim_start_matches('\u{feff}'))
            .with_context(|| format!("parse config {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir_all(&self.receive_dir)?;
        fs::create_dir_all(&self.data_dir)?;
        fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    pub fn peer_base_url(&self) -> String {
        format!("http://{}:{}", self.peer.host, self.peer.port)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicDevice {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub port: u16,
    pub protocol_version: u32,
    pub capabilities: Vec<String>,
}

impl From<&AppConfig> for PublicDevice {
    fn from(value: &AppConfig) -> Self {
        Self {
            device_id: value.device_id.clone(),
            device_name: value.device_name.clone(),
            platform: value.platform.clone(),
            port: value.port,
            protocol_version: PROTOCOL_VERSION,
            capabilities: vec![
                "file_chunk_upload".to_string(),
                "file_resume".to_string(),
                "file_sha256".to_string(),
                "clipboard_text".to_string(),
                "clipboard_png_chunk".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    Unconfigured,
    Connecting,
    Connected,
    PeerOffline,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferDirection {
    Send,
    Receive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferStatus {
    Queued,
    Transferring,
    Completed,
    Failed,
    Cancelled,
    Retrying,
    Prepared,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferTask {
    pub task_id: String,
    pub direction: TransferDirection,
    pub peer_device_id: Option<String>,
    pub root_name: String,
    pub item_count: usize,
    pub total_size: u64,
    pub transferred_size: u64,
    pub status: TransferStatus,
    pub error: Option<String>,
    pub save_path: Option<PathBuf>,
    #[serde(default)]
    pub speed_bytes_per_sec: u64,
    #[serde(default)]
    pub eta_seconds: Option<u64>,
    #[serde(default)]
    pub retry_count: u32,
    #[serde(default)]
    pub error_code: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub source_paths: Vec<PathBuf>,
    #[serde(default)]
    pub parent_task_id: Option<String>,
    #[serde(default)]
    pub attempt_id: Option<String>,
    #[serde(default)]
    pub phase: Option<String>,
    #[serde(default)]
    pub current_file: Option<String>,
    #[serde(default)]
    pub preflight_bytes: u64,
    #[serde(default)]
    pub preflight_total_bytes: u64,
}

impl TransferTask {
    pub fn new_send(manifest: &LocalTransferManifest, paths: Vec<PathBuf>) -> Self {
        let now = Utc::now();
        Self {
            task_id: manifest.task_id.clone(),
            direction: TransferDirection::Send,
            peer_device_id: None,
            root_name: manifest.root_name.clone(),
            item_count: manifest.files.len(),
            total_size: manifest.total_size,
            transferred_size: 0,
            status: TransferStatus::Queued,
            error: None,
            save_path: None,
            speed_bytes_per_sec: 0,
            eta_seconds: None,
            retry_count: 0,
            error_code: None,
            created_at: now,
            updated_at: now,
            source_paths: paths,
            parent_task_id: None,
            attempt_id: None,
            phase: None,
            current_file: None,
            preflight_bytes: 0,
            preflight_total_bytes: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalTransferManifest {
    pub task_id: String,
    pub root_name: String,
    pub files: Vec<LocalTransferItem>,
    pub total_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalTransferItem {
    pub relative_path: String,
    pub size: u64,
    pub source_path: PathBuf,
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireTransferManifest {
    pub task_id: String,
    pub root_name: String,
    pub files: Vec<WireTransferItem>,
    pub total_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireTransferItem {
    pub relative_path: String,
    pub size: u64,
    #[serde(default)]
    pub sha256: Option<String>,
}

impl LocalTransferManifest {
    pub fn to_wire(&self) -> WireTransferManifest {
        WireTransferManifest {
            task_id: self.task_id.clone(),
            root_name: self.root_name.clone(),
            total_size: self.total_size,
            files: self
                .files
                .iter()
                .map(|item| WireTransferItem {
                    relative_path: item.relative_path.clone(),
                    size: item.size,
                    sha256: item.sha256.clone(),
                })
                .collect(),
        }
    }
}

pub type TransferManifest = LocalTransferManifest;
pub type TransferItem = LocalTransferItem;

pub fn scan_manifest(paths: &[PathBuf]) -> Result<LocalTransferManifest> {
    if paths.is_empty() {
        bail!("no paths supplied");
    }
    let mut files = Vec::new();
    for path in paths {
        let meta = fs::metadata(path).with_context(|| format!("metadata {}", path.display()))?;
        if meta.is_file() {
            let name = path
                .file_name()
                .ok_or_else(|| anyhow!("path has no file name: {}", path.display()))?
                .to_string_lossy()
                .to_string();
            files.push(LocalTransferItem {
                relative_path: name,
                size: meta.len(),
                source_path: path.clone(),
                sha256: None,
            });
        } else if meta.is_dir() {
            let root_name = path
                .file_name()
                .ok_or_else(|| anyhow!("folder has no name: {}", path.display()))?
                .to_string_lossy()
                .to_string();
            for entry in WalkDir::new(path).follow_links(false) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    let rel = entry.path().strip_prefix(path)?;
                    let relative_path = normalize_relative_path(Path::new(&root_name).join(rel))?;
                    files.push(LocalTransferItem {
                        relative_path,
                        size: entry.metadata()?.len(),
                        source_path: entry.path().to_path_buf(),
                        sha256: None,
                    });
                }
            }
        } else {
            bail!("unsupported path type: {}", path.display());
        }
    }
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    let total_size = files.iter().map(|f| f.size).sum();
    let root_name = if paths.len() == 1 {
        paths[0]
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Transfer".to_string())
    } else {
        format!("{} items", paths.len())
    };
    Ok(LocalTransferManifest {
        task_id: Uuid::new_v4().to_string(),
        root_name,
        files,
        total_size,
    })
}

pub fn normalize_relative_path(path: PathBuf) -> Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::CurDir => {}
            _ => bail!("unsafe relative path: {}", path.display()),
        }
    }
    if parts.is_empty() {
        bail!("empty relative path");
    }
    Ok(parts.join("/"))
}

pub fn safe_join(base: &Path, raw_relative: &str) -> Result<PathBuf> {
    normalize_relative_path(PathBuf::from(raw_relative))?;
    let mut out = base.to_path_buf();
    for part in raw_relative.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            bail!("unsafe relative path: {raw_relative}");
        }
        out.push(part);
    }
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub ts: DateTime<Utc>,
    #[serde(rename = "type")]
    pub event_type: String,
    pub data: serde_json::Value,
}

#[derive(Clone)]
pub struct EventLog {
    inner: Arc<Mutex<VecDeque<Event>>>,
    capacity: usize,
}

impl EventLog {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            capacity,
        }
    }

    pub fn push(&self, event_type: impl Into<String>, data: serde_json::Value) -> Event {
        let event = Event {
            ts: Utc::now(),
            event_type: event_type.into(),
            data,
        };
        let mut lock = self.inner.lock().expect("event log poisoned");
        if lock.len() >= self.capacity {
            lock.pop_front();
        }
        lock.push_back(event.clone());
        event
    }

    pub fn latest(&self, limit: usize) -> Vec<Event> {
        let lock = self.inner.lock().expect("event log poisoned");
        lock.iter()
            .rev()
            .take(limit)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ClipboardPayload {
    Text {
        text: String,
        hash: String,
        source_device_id: String,
    },
    Image {
        png: Vec<u8>,
        hash: String,
        source_device_id: String,
    },
}

impl ClipboardPayload {
    pub fn hash_text(text: &str) -> String {
        hex_hash(text.as_bytes())
    }

    pub fn hash_bytes(bytes: &[u8]) -> String {
        hex_hash(bytes)
    }

    pub fn hash(&self) -> &str {
        match self {
            ClipboardPayload::Text { hash, .. } => hash,
            ClipboardPayload::Image { hash, .. } => hash,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            ClipboardPayload::Text { .. } => "text",
            ClipboardPayload::Image { .. } => "image",
        }
    }
}

pub trait ClipboardPort: Send {
    fn read_text(&mut self) -> Result<Option<String>>;
    fn write_text(&mut self, text: &str) -> Result<()>;
    fn read_png(&mut self) -> Result<Option<Vec<u8>>>;
    fn write_png(&mut self, png: &[u8]) -> Result<()>;
}

pub fn hex_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn file_sha256(path: &Path) -> Result<String> {
    file_sha256_with_progress(path, |_| Ok(()))
}

pub fn file_sha256_with_progress(
    path: &Path,
    mut on_progress: impl FnMut(u64) -> Result<()>,
) -> Result<String> {
    let mut file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        on_progress(n as u64)?;
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[derive(Clone)]
pub struct HistoryDb {
    path: PathBuf,
}

impl HistoryDb {
    pub fn open(path: PathBuf) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let db = Self { path };
        db.with_conn(|conn| {
            conn.execute_batch(
                r#"
                create table if not exists transfer_tasks (
                    task_id text primary key,
                    json text not null,
                    updated_at text not null
                );
                create table if not exists transfer_history (
                    id integer primary key autoincrement,
                    task_id text not null,
                    json text not null,
                    created_at text not null
                );
                create table if not exists clipboard_events (
                    id integer primary key autoincrement,
                    kind text not null,
                    hash text not null,
                    direction text not null,
                    created_at text not null
                );
                "#,
            )?;
            Ok(())
        })?;
        Ok(db)
    }

    pub fn upsert_task(&self, task: &TransferTask) -> Result<()> {
        let json = serde_json::to_string(task)?;
        self.with_conn(|conn| {
            conn.execute(
                "insert into transfer_tasks(task_id,json,updated_at) values(?1,?2,?3)
                 on conflict(task_id) do update set json=excluded.json, updated_at=excluded.updated_at",
                params![task.task_id, json, task.updated_at.to_rfc3339()],
            )?;
            Ok(())
        })
    }

    pub fn append_history(&self, task: &TransferTask) -> Result<()> {
        let json = serde_json::to_string(task)?;
        self.with_conn(|conn| {
            conn.execute(
                "insert into transfer_history(task_id,json,created_at) values(?1,?2,?3)",
                params![task.task_id, json, Utc::now().to_rfc3339()],
            )?;
            Ok(())
        })
    }

    pub fn tasks(&self) -> Result<Vec<TransferTask>> {
        self.with_conn(|conn| {
            let mut stmt =
                conn.prepare("select json from transfer_tasks order by updated_at desc")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            let mut out = Vec::new();
            for row in rows {
                out.push(serde_json::from_str(&row?)?);
            }
            Ok(out)
        })
    }

    pub fn history(&self, limit: usize) -> Result<Vec<TransferTask>> {
        self.with_conn(|conn| {
            let mut stmt =
                conn.prepare("select json from transfer_history order by id desc limit ?1")?;
            let rows = stmt.query_map([limit as i64], |row| row.get::<_, String>(0))?;
            let mut out = Vec::new();
            for row in rows {
                out.push(serde_json::from_str(&row?)?);
            }
            Ok(out)
        })
    }

    pub fn delete_task(&self, task_id: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "delete from transfer_tasks where task_id = ?1",
                params![task_id],
            )?;
            Ok(())
        })
    }

    pub fn clear_history(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute("delete from transfer_history", [])?;
            Ok(())
        })
    }

    pub fn prune_history(&self, retention_days: u32) -> Result<()> {
        let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
        self.with_conn(|conn| {
            conn.execute(
                "delete from transfer_history where created_at < ?1",
                params![cutoff.to_rfc3339()],
            )?;
            Ok(())
        })
    }

    pub fn record_clipboard(&self, kind: &str, hash: &str, direction: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "insert into clipboard_events(kind,hash,direction,created_at) values(?1,?2,?3,?4)",
                params![kind, hash, direction, Utc::now().to_rfc3339()],
            )?;
            Ok(())
        })
    }

    fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let conn = Connection::open(&self.path)?;
        f(&conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_manifest_serialization_does_not_include_source_path() {
        let manifest = LocalTransferManifest {
            task_id: "task-1".to_string(),
            root_name: "demo".to_string(),
            total_size: 4,
            files: vec![LocalTransferItem {
                relative_path: "demo/a.txt".to_string(),
                size: 4,
                source_path: PathBuf::from(r"C:\Users\zhang\secret\a.txt"),
                sha256: Some("abc".to_string()),
            }],
        };

        let json = serde_json::to_string(&manifest.to_wire()).expect("serialize wire manifest");
        assert!(!json.contains("source_path"));
        assert!(!json.contains("C:\\"));
        assert!(!json.contains("Users"));
        assert!(json.contains("demo/a.txt"));
    }
}
