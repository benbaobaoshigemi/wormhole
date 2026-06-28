use axum::{extract::State, Json};
use serde_json::json;
use wormhole_core::{ConnectionStatus, PublicDevice, TransferStatus};

use crate::{
    dto::{
        CancelRequest, ClipboardStatusDto, PublicSettingsDto, SendRequest, SettingsUpdateRequest,
        StateDto, TransferHistoryDto, TransferTaskDto,
    },
    error::ApiError,
    service::{clipboard, connection, settings, transfer},
    state::AppState,
};

pub async fn state(State(state): State<AppState>) -> Json<StateDto> {
    let config = state.config.read().await;
    let tasks = state.db.tasks().unwrap_or_default();
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
    Json(StateDto {
        device: PublicDevice::from(&*config),
        status: state.status.read().await.clone(),
        peer: state.peer.read().await.clone(),
        settings: PublicSettingsDto::from(&*config),
        clipboard: ClipboardStatusDto::from(&config.clipboard),
        active_transfer_count,
        recent_history_count: history_count,
        tasks: tasks.iter().map(TransferTaskDto::from).collect(),
        events: state.events.latest(100),
    })
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
) -> Result<Json<serde_json::Value>, ApiError> {
    transfer::retry_transfer(state).await
}

pub async fn tasks(State(state): State<AppState>) -> Json<Vec<TransferTaskDto>> {
    Json(
        state
            .db
            .tasks()
            .unwrap_or_default()
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
