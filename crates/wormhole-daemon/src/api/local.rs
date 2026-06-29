use axum::{extract::State, Json};
use serde_json::json;
use wormhole_core::{ConnectionStatus, PublicDevice, TransferStatus, TransferTask};

use crate::{
    dto::{
        CancelRequest, ClipboardStatusDto, DiagnosticsDto, PublicSettingsDto, SendRequest,
        SettingsUpdateRequest, StateDto, TransferHistoryDto, TransferTaskDto,
    },
    error::ApiError,
    service::{clipboard, connection, settings, transfer},
    state::AppState,
};

pub async fn state(State(state): State<AppState>) -> Json<StateDto> {
    let config = state.config.read().await;
    let tasks = current_tasks(&state).await;
    let history_count = state.db.history(100).map(|h| h.len()).unwrap_or(0);
    let active_transfer_count = tasks
        .iter()
        .filter(|task| {
            matches!(
                task.status,
                TransferStatus::Queued
                    | TransferStatus::Prepared
                    | TransferStatus::Transferring
                    | TransferStatus::Retrying
            )
        })
        .count();

    let daemon_path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "wormhole-daemon.exe".to_string());
    let config_path = std::env::current_dir()
        .map(|cd| cd.join(&state.config_path))
        .unwrap_or(state.config_path.clone())
        .to_string_lossy()
        .to_string();

    let connection_status = state.status.read().await.clone();
    let incoming_traffic_received = *state.incoming_traffic_received.read().await;
    let raw_firewall_status = state.firewall_status.read().await.clone();
    let firewall_status =
        effective_firewall_status(&raw_firewall_status, incoming_traffic_received).to_string();

    let diagnostics = DiagnosticsDto {
        daemon_path,
        config_path,
        bind_host: config.bind_host.clone(),
        local_port: config.port,
        peer_host: config.peer.host.clone(),
        peer_port: config.peer.port,
        network_profile: state.network_profile.read().await.clone(),
        firewall_status,
        incoming_traffic_received,
        last_handshake_error: state.last_handshake_error.read().await.clone(),
        last_transfer_error_code: state.last_transfer_error_code.read().await.clone(),
        last_transfer_error_message: state.last_transfer_error_message.read().await.clone(),
    };

    Json(StateDto {
        device: PublicDevice::from(&*config),
        status: connection_status,
        peer: state.peer.read().await.clone(),
        settings: PublicSettingsDto::from(&*config),
        clipboard: ClipboardStatusDto::from(&config.clipboard),
        active_transfer_count,
        recent_history_count: history_count,
        tasks: tasks.iter().map(TransferTaskDto::from).collect(),
        events: state.events.latest(100),
        diagnostics,
    })
}

fn effective_firewall_status(status: &str, incoming_traffic_received: bool) -> &str {
    if incoming_traffic_received
        && matches!(status, "missing_rule" | "stale_program_path" | "unknown")
    {
        return "ok";
    }
    status
}

pub async fn get_settings(State(state): State<AppState>) -> Json<PublicSettingsDto> {
    Json(PublicSettingsDto::from(&*state.config.read().await))
}

pub async fn update_settings(
    State(state): State<AppState>,
    Json(req): Json<SettingsUpdateRequest>,
) -> Result<Json<PublicSettingsDto>, ApiError> {
    settings::update_settings(&state, req).await
}

pub async fn connect(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    connection::connect(&state).await
}

pub async fn send_transfer(
    State(state): State<AppState>,
    Json(req): Json<SendRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    transfer::send_transfer(state, req).await
}

pub async fn cancel_transfer(
    State(state): State<AppState>,
    Json(req): Json<CancelRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    transfer::cancel_transfer(&state, &req.task_id).await
}

pub async fn retry_transfer(
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    let req = if body.is_empty() {
        None
    } else {
        Some(serde_json::from_slice(&body)?)
    };
    transfer::retry_transfer(state, req).await
}

pub async fn tasks(State(state): State<AppState>) -> Json<Vec<TransferTaskDto>> {
    Json(
        current_tasks(&state)
            .await
            .iter()
            .map(TransferTaskDto::from)
            .collect(),
    )
}

pub async fn history(State(state): State<AppState>) -> Json<Vec<TransferHistoryDto>> {
    Json(
        state
            .db
            .history(100)
            .unwrap_or_default()
            .iter()
            .map(TransferHistoryDto::from)
            .collect(),
    )
}

pub async fn clear_history(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.db.clear_history()?;
    state.emit("transfer.history_cleared", json!({}));
    Ok(Json(json!({"ok":true})))
}

pub async fn clipboard_status(State(state): State<AppState>) -> Json<ClipboardStatusDto> {
    Json(ClipboardStatusDto::from(
        &state.config.read().await.clipboard,
    ))
}

pub async fn clipboard_enable(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    clipboard::clipboard_enable(&state).await
}

pub async fn clipboard_disable(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    clipboard::clipboard_disable(&state).await
}

pub async fn read_send_text(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    clipboard::read_send_text(&state).await
}

pub async fn read_send_image(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    clipboard::read_send_image(&state).await
}

pub async fn disconnect(State(state): State<AppState>) -> Json<serde_json::Value> {
    *state.status.write().await = ConnectionStatus::PeerOffline;
    state.emit("connection.changed", json!({"status":"peer_offline"}));
    Json(json!({"ok":true}))
}

async fn current_tasks(state: &AppState) -> Vec<TransferTask> {
    let mut tasks = state
        .tasks
        .lock()
        .await
        .values()
        .cloned()
        .collect::<Vec<_>>();
    tasks.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| b.created_at.cmp(&a.created_at))
            .then_with(|| b.task_id.cmp(&a.task_id))
    });
    if tasks.is_empty() {
        let mut recovered = state.db.tasks().unwrap_or_default();
        recovered.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| b.created_at.cmp(&a.created_at))
                .then_with(|| b.task_id.cmp(&a.task_id))
        });
        return recovered;
    }
    tasks
}
