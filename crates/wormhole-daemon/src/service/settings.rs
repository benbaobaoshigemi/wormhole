use axum::Json;
use serde_json::json;

use crate::{
    dto::{PublicSettingsDto, SettingsUpdateRequest},
    error::ApiError,
    state::AppState,
};

pub async fn update_settings(
    state: &AppState,
    req: SettingsUpdateRequest,
) -> Result<Json<PublicSettingsDto>, ApiError> {
    let mut config = state.config.write().await;
    if let Some(value) = req.device_name {
        config.device_name = value;
    }
    if let Some(value) = req.peer_name {
        config.peer.name = value;
    }
    if let Some(value) = req.peer_host {
        config.peer.host = value;
    }
    if let Some(value) = req.peer_port {
        config.peer.port = value;
    }
    if let Some(value) = req.receive_dir {
        config.receive_dir = value;
    }
    if let Some(value) = req.auto_connect {
        config.auto_connect = value;
    }
    if let Some(value) = req.clipboard_enabled {
        config.clipboard.enabled = value;
    }
    if let Some(value) = req.clipboard_text_enabled {
        config.clipboard.text_enabled = value;
    }
    if let Some(value) = req.clipboard_image_enabled {
        config.clipboard.image_enabled = value;
    }
    if let Some(value) = req.max_image_bytes {
        config.clipboard.max_image_bytes = value;
    }
    if let Some(value) = req.retry_limit {
        config.retry_limit = value;
    }
    config.save(&state.config_path)?;
    let dto = PublicSettingsDto::from(&*config);
    drop(config);
    state.emit("settings.updated", json!({"settings":dto}));
    Ok(Json(dto))
}
