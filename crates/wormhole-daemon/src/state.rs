use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::Arc,
};

use tokio::sync::{broadcast, Mutex, RwLock, Semaphore};
use wormhole_core::{
    AppConfig, ConnectionStatus, Event, EventLog, HistoryDb, LocalTransferManifest, PublicDevice,
    TransferTask,
};
use wormhole_platform::SystemClipboard;

#[derive(Debug, Clone)]
pub struct ReceiveFileState {
    pub relative_path: String,
    pub expected_size: u64,
    pub expected_sha256: Option<String>,
    pub final_path: PathBuf,
    pub tmp_path: PathBuf,
    pub received_size: u64,
    pub completed: bool,
}

#[derive(Debug, Clone)]
pub struct ReceiveTaskState {
    pub files: HashMap<String, ReceiveFileState>,
}

#[derive(Debug, Clone)]
pub struct PreparedImageState {
    pub hash: String,
    pub source_device_id: String,
    pub expected_size: u64,
    pub received_size: u64,
    pub tmp_path: PathBuf,
    pub max_image_bytes: u64,
}

#[derive(Clone)]
pub struct FailedTransfer {
    pub task_id: String,
    pub paths: Vec<PathBuf>,
    pub manifest: LocalTransferManifest,
}

#[derive(Clone)]
pub struct AppState {
    pub config_path: PathBuf,
    pub config: Arc<RwLock<AppConfig>>,
    pub status: Arc<RwLock<ConnectionStatus>>,
    pub peer: Arc<RwLock<Option<PublicDevice>>>,
    pub db: HistoryDb,
    pub events: EventLog,
    pub event_tx: broadcast::Sender<Event>,
    pub tasks: Arc<Mutex<HashMap<String, TransferTask>>>,
    pub failed: Arc<Mutex<VecDeque<FailedTransfer>>>,
    pub failed_task_ids: Arc<Mutex<HashSet<String>>>,
    pub cancelled: Arc<Mutex<HashSet<String>>>,
    pub receive_tasks: Arc<Mutex<HashMap<String, ReceiveTaskState>>>,
    pub prepared_images: Arc<Mutex<HashMap<String, PreparedImageState>>>,
    pub transfer_slots: Arc<Semaphore>,
    pub remote_hashes: Arc<Mutex<VecDeque<String>>>,
    pub clipboard: Arc<Mutex<SystemClipboard>>,
}

impl AppState {
    pub fn emit(&self, event_type: &str, data: serde_json::Value) {
        let event = self.events.push(event_type, data);
        let _ = self.event_tx.send(event);
    }
}
