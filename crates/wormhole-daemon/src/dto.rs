use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use wormhole_core::{
    AppConfig, ClipboardSettings, ConnectionSettings, ConnectionStatus, PublicDevice,
    TransferDirection, TransferSettings, TransferStatus, TransferTask,
};

#[derive(Debug, Clone, Serialize)]
pub struct PublicSettingsDto {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub bind_host: String,
    pub port: u16,
    pub peer_name: String,
    pub peer_host: String,
    pub peer_port: u16,
    pub receive_dir: String,
    pub auto_connect: bool,
    pub clipboard: ClipboardSettings,
    pub transfer: TransferSettings,
    pub connection: ConnectionSettings,
    pub history_retention_days: u32,
    pub retry_limit: u32,
}

impl From<&AppConfig> for PublicSettingsDto {
    fn from(config: &AppConfig) -> Self {
        Self {
            device_id: config.device_id.clone(),
            device_name: config.device_name.clone(),
            platform: config.platform.clone(),
            bind_host: config.bind_host.clone(),
            port: config.port,
            peer_name: config.peer.name.clone(),
            peer_host: config.peer.host.clone(),
            peer_port: config.peer.port,
            receive_dir: config.receive_dir.to_string_lossy().to_string(),
            auto_connect: config.auto_connect,
            clipboard: config.clipboard.clone(),
            transfer: config.transfer.clone(),
            connection: config.connection.clone(),
            history_retention_days: config.history_retention_days,
            retry_limit: config.retry_limit,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferTaskDto {
    pub task_id: String,
    pub direction: TransferDirection,
    pub peer_device_id: Option<String>,
    pub root_name: String,
    pub item_count: usize,
    pub total_size: u64,
    pub transferred_size: u64,
    pub status: TransferStatus,
    pub error: Option<String>,
    pub error_code: Option<String>,
    pub save_path: Option<String>,
    pub speed_bytes_per_sec: u64,
    pub eta_seconds: Option<u64>,
    pub retry_count: u32,
    pub parent_task_id: Option<String>,
    pub attempt_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<&TransferTask> for TransferTaskDto {
    fn from(task: &TransferTask) -> Self {
        Self {
            task_id: task.task_id.clone(),
            direction: task.direction.clone(),
            peer_device_id: task.peer_device_id.clone(),
            root_name: task.root_name.clone(),
            item_count: task.item_count,
            total_size: task.total_size,
            transferred_size: task.transferred_size,
            status: task.status.clone(),
            error: task.error.clone(),
            error_code: task.error_code.clone(),
            save_path: task
                .save_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
            speed_bytes_per_sec: task.speed_bytes_per_sec,
            eta_seconds: task.eta_seconds,
            retry_count: task.retry_count,
            parent_task_id: task.parent_task_id.clone(),
            attempt_id: task.attempt_id.clone(),
            created_at: task.created_at,
            updated_at: task.updated_at,
        }
    }
}

pub type TransferHistoryDto = TransferTaskDto;

#[derive(Debug, Clone, Serialize)]
pub struct StateDto {
    pub device: PublicDevice,
    pub status: ConnectionStatus,
    pub peer: Option<PublicDevice>,
    pub settings: PublicSettingsDto,
    pub clipboard: ClipboardStatusDto,
    pub active_transfer_count: usize,
    pub recent_history_count: usize,
    pub tasks: Vec<TransferTaskDto>,
    pub events: Vec<wormhole_core::Event>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClipboardStatusDto {
    pub enabled: bool,
    pub text_enabled: bool,
    pub image_enabled: bool,
    pub max_image_bytes: u64,
}

impl From<&ClipboardSettings> for ClipboardStatusDto {
    fn from(value: &ClipboardSettings) -> Self {
        Self {
            enabled: value.enabled,
            text_enabled: value.text_enabled,
            image_enabled: value.image_enabled,
            max_image_bytes: value.max_image_bytes,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SendRequest {
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub struct CancelRequest {
    pub task_id: String,
}

#[derive(Debug, Deserialize)]
pub struct RetryRequest {
    pub task_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SettingsUpdateRequest {
    pub device_name: Option<String>,
    pub peer_name: Option<String>,
    pub peer_host: Option<String>,
    pub peer_port: Option<u16>,
    pub receive_dir: Option<PathBuf>,
    pub auto_connect: Option<bool>,
    pub clipboard_enabled: Option<bool>,
    pub clipboard_text_enabled: Option<bool>,
    pub clipboard_image_enabled: Option<bool>,
    pub max_image_bytes: Option<u64>,
    pub retry_limit: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ImagePrepareResponse {
    pub accepted: bool,
    pub reason: Option<String>,
    pub offset: Option<u64>,
    pub max_image_bytes: u64,
}
