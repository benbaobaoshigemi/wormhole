use anyhow::{anyhow, Result};
use axum::{body::Bytes, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
};
use wormhole_core::{ClipboardPayload, ClipboardPort};

use crate::{
    dto::ImagePrepareResponse,
    error::ApiError,
    state::{AppState, PreparedImageState},
    transport::clipboard_transport::{self, ClipboardUploadOutcome},
};

pub const IMAGE_CHUNK_SIZE: usize = 256 * 1024;
pub const MAX_IMAGE_CHUNK_SIZE: usize = 2 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct ReceiveText {
    pub text: String,
    pub hash: String,
    pub source_device_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ImagePrepareRequest {
    pub hash: String,
    pub source_device_id: String,
    pub size: u64,
}

#[derive(Debug, Deserialize)]
pub struct ImageChunkQuery {
    pub hash: String,
    pub source_device_id: String,
    pub final_chunk: bool,
    #[serde(default)]
    pub offset: u64,
}

pub async fn clipboard_enable(state: &AppState) -> Result<Json<serde_json::Value>, ApiError> {
    let mut config = state.config.write().await;
    config.clipboard.enabled = true;
    config.save(&state.config_path)?;
    drop(config);
    state.emit("settings.updated", json!({"clipboard_enabled":true}));
    Ok(Json(json!({"ok":true})))
}

pub async fn clipboard_disable(state: &AppState) -> Result<Json<serde_json::Value>, ApiError> {
    let mut config = state.config.write().await;
    config.clipboard.enabled = false;
    config.save(&state.config_path)?;
    drop(config);
    state.emit("settings.updated", json!({"clipboard_enabled":false}));
    Ok(Json(json!({"ok":true})))
}

pub async fn read_send_text(state: &AppState) -> Result<Json<serde_json::Value>, ApiError> {
    let text = {
        let mut clipboard = state.clipboard.lock().await;
        clipboard
            .read_text()?
            .ok_or_else(|| anyhow!("clipboard has no text"))?
    };
    let hash = ClipboardPayload::hash_text(&text);
    if is_remote_hash(state, &hash).await {
        state.emit(
            "clipboard.ignored",
            json!({"kind":"text","hash":hash,"reason":"loop_prevented"}),
        );
        return Ok(Json(json!({"ok":true,"ignored":true,"hash":hash})));
    }
    send_text_to_peer(state, text, hash.clone()).await?;
    Ok(Json(json!({"ok":true,"hash":hash})))
}

pub async fn read_send_image(state: &AppState) -> Result<Json<serde_json::Value>, ApiError> {
    let png = {
        let mut clipboard = state.clipboard.lock().await;
        clipboard
            .read_png()?
            .ok_or_else(|| anyhow!("clipboard has no image"))?
    };
    send_png_to_peer(state, png).await
}

pub async fn receive_text(
    state: &AppState,
    req: ReceiveText,
) -> Result<Json<serde_json::Value>, ApiError> {
    {
        let config = state.config.read().await;
        if req.source_device_id == config.device_id {
            state.emit(
                "clipboard.ignored",
                json!({"kind":"text","hash":req.hash,"reason":"self_source"}),
            );
            return Ok(Json(json!({"ok":true,"ignored":true})));
        }
    }
    if ClipboardPayload::hash_text(&req.text) != req.hash {
        return Err(ApiError::status(
            StatusCode::UNPROCESSABLE_ENTITY,
            "integrity",
            anyhow!("clipboard text hash mismatch"),
        ));
    }
    state.clipboard.lock().await.write_text(&req.text)?;
    remember_remote_hash(state, req.hash.clone()).await;
    state.db.record_clipboard("text", &req.hash, "receive")?;
    state.emit(
        "clipboard.synced",
        json!({"kind":"text","hash":req.hash,"source_device_id":req.source_device_id}),
    );
    Ok(Json(json!({"ok":true})))
}

pub async fn prepare_image_clipboard(
    state: &AppState,
    req: ImagePrepareRequest,
) -> Result<Json<ImagePrepareResponse>, ApiError> {
    let config = state.config.read().await.clone();
    if !is_hex_sha256(&req.hash) {
        return Ok(Json(ImagePrepareResponse {
            accepted: false,
            reason: Some("invalid_hash".to_string()),
            offset: None,
            max_image_bytes: config.clipboard.max_image_bytes,
        }));
    }
    if req.source_device_id.is_empty() {
        return Ok(Json(ImagePrepareResponse {
            accepted: false,
            reason: Some("missing_source_device_id".to_string()),
            offset: None,
            max_image_bytes: config.clipboard.max_image_bytes,
        }));
    }
    if req.source_device_id == config.device_id {
        state.emit(
            "clipboard.ignored",
            json!({"kind":"image","hash":req.hash,"reason":"self_source"}),
        );
        return Ok(Json(ImagePrepareResponse {
            accepted: false,
            reason: Some("self_source".to_string()),
            offset: None,
            max_image_bytes: config.clipboard.max_image_bytes,
        }));
    }
    if req.size > config.clipboard.max_image_bytes {
        state.emit(
            "clipboard.too_large",
            json!({"kind":"image","hash":req.hash,"size":req.size}),
        );
        return Ok(Json(ImagePrepareResponse {
            accepted: false,
            reason: Some("too_large".to_string()),
            offset: None,
            max_image_bytes: config.clipboard.max_image_bytes,
        }));
    }

    let dir = config.data_dir.join("clipboard");
    fs::create_dir_all(&dir).await?;
    let tmp = clipboard_tmp_path(&config.data_dir, &req.hash);
    let mut offset = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
    if offset > req.size {
        let _ = fs::remove_file(&tmp).await;
        offset = 0;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&tmp)
        .await?;
    offset = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);

    let key = prepared_image_key(&req.source_device_id, &req.hash);
    state.prepared_images.lock().await.insert(
        key,
        PreparedImageState {
            hash: req.hash.clone(),
            source_device_id: req.source_device_id.clone(),
            expected_size: req.size,
            received_size: offset,
            tmp_path: tmp,
            max_image_bytes: config.clipboard.max_image_bytes,
        },
    );

    Ok(Json(ImagePrepareResponse {
        accepted: true,
        reason: None,
        offset: Some(offset),
        max_image_bytes: config.clipboard.max_image_bytes,
    }))
}

pub async fn receive_image_chunk(
    state: &AppState,
    query: ImageChunkQuery,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    if body.len() > MAX_IMAGE_CHUNK_SIZE {
        return Err(ApiError::status(
            StatusCode::PAYLOAD_TOO_LARGE,
            "chunk_too_large",
            anyhow!("clipboard image chunk too large"),
        ));
    }
    if !is_hex_sha256(&query.hash) {
        return Err(ApiError::status(
            StatusCode::BAD_REQUEST,
            "invalid_hash",
            anyhow!("invalid clipboard image hash"),
        ));
    }
    let config = state.config.read().await.clone();
    if query.source_device_id.is_empty() {
        return Err(ApiError::status(
            StatusCode::BAD_REQUEST,
            "missing_source_device_id",
            anyhow!("missing source device id"),
        ));
    }
    if query.source_device_id == config.device_id {
        state.emit(
            "clipboard.ignored",
            json!({"kind":"image","hash":query.hash,"reason":"self_source"}),
        );
        return Ok(Json(json!({"ok":true,"ignored":true})));
    }

    let key = prepared_image_key(&query.source_device_id, &query.hash);
    let prepared = {
        let prepared_images = state.prepared_images.lock().await;
        prepared_images.get(&key).cloned().ok_or_else(|| {
            ApiError::status(
                StatusCode::CONFLICT,
                "image_not_prepared",
                anyhow!("clipboard image chunk received before prepare"),
            )
        })?
    };

    if prepared.hash != query.hash || prepared.source_device_id != query.source_device_id {
        return Err(ApiError::status(
            StatusCode::BAD_REQUEST,
            "prepare_mismatch",
            anyhow!("clipboard image prepare state mismatch"),
        ));
    }
    if query.offset != prepared.received_size {
        return Err(ApiError::status(
            StatusCode::CONFLICT,
            "offset_mismatch",
            anyhow!("clipboard image offset mismatch"),
        ));
    }
    let current_len = std::fs::metadata(&prepared.tmp_path)
        .map(|m| m.len())
        .unwrap_or(0);
    if current_len != prepared.received_size {
        return Err(ApiError::status(
            StatusCode::CONFLICT,
            "offset_mismatch",
            anyhow!("clipboard image temp file size mismatch"),
        ));
    }
    let new_len = current_len.saturating_add(body.len() as u64);
    if new_len > prepared.max_image_bytes || new_len > config.clipboard.max_image_bytes {
        let _ = fs::remove_file(&prepared.tmp_path).await;
        state.prepared_images.lock().await.remove(&key);
        state.emit(
            "clipboard.too_large",
            json!({"kind":"image","hash":query.hash,"size":new_len}),
        );
        return Err(ApiError::status(
            StatusCode::PAYLOAD_TOO_LARGE,
            "too_large",
            anyhow!("clipboard image too large"),
        ));
    }
    if new_len > prepared.expected_size {
        let _ = fs::remove_file(&prepared.tmp_path).await;
        state.prepared_images.lock().await.remove(&key);
        state.emit(
            "clipboard.failed",
            json!({"kind":"image","hash":query.hash,"error_code":"size_exceeded"}),
        );
        return Err(ApiError::status(
            StatusCode::UNPROCESSABLE_ENTITY,
            "size_exceeded",
            anyhow!("clipboard image exceeded prepared size"),
        ));
    }

    {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&prepared.tmp_path)
            .await?;
        file.write_all(&body).await?;
        file.flush().await?;
    }

    if !query.final_chunk {
        if let Some(prepared) = state.prepared_images.lock().await.get_mut(&key) {
            prepared.received_size = new_len;
        }
        return Ok(Json(
            json!({"ok":true,"received":body.len(),"offset":new_len}),
        ));
    }

    if new_len != prepared.expected_size {
        let _ = fs::remove_file(&prepared.tmp_path).await;
        state.prepared_images.lock().await.remove(&key);
        state.emit(
            "clipboard.failed",
            json!({"kind":"image","hash":query.hash,"error_code":"size_mismatch"}),
        );
        return Err(ApiError::status(
            StatusCode::UNPROCESSABLE_ENTITY,
            "size_mismatch",
            anyhow!("clipboard image final size mismatch"),
        ));
    }

    let png = fs::read(&prepared.tmp_path).await?;
    let actual_hash = ClipboardPayload::hash_bytes(&png);
    if actual_hash != query.hash {
        let _ = fs::remove_file(&prepared.tmp_path).await;
        state.prepared_images.lock().await.remove(&key);
        state.emit(
            "clipboard.failed",
            json!({"kind":"image","hash":query.hash,"error_code":"integrity"}),
        );
        return Err(ApiError::status(
            StatusCode::UNPROCESSABLE_ENTITY,
            "integrity",
            anyhow!("clipboard image hash mismatch"),
        ));
    }
    let _ = fs::remove_file(&prepared.tmp_path).await;
    state.prepared_images.lock().await.remove(&key);
    state.clipboard.lock().await.write_png(&png)?;
    remember_remote_hash(state, query.hash.clone()).await;
    state.db.record_clipboard("image", &query.hash, "receive")?;
    state.emit(
        "clipboard.synced",
        json!({"kind":"image","hash":query.hash,"size":png.len(),"source_device_id":query.source_device_id}),
    );
    Ok(Json(json!({"ok":true,"hash":query.hash,"size":png.len()})))
}

pub async fn clipboard_loop(state: AppState) {
    let mut last_text = String::new();
    let mut last_image = String::new();
    loop {
        let config = state.config.read().await.clone();
        tokio::time::sleep(Duration::from_millis(config.clipboard.poll_millis.max(200))).await;
        if !config.clipboard.enabled {
            continue;
        }
        if config.clipboard.text_enabled {
            let read = state.clipboard.lock().await.read_text();
            if let Ok(Some(text)) = read {
                let hash = ClipboardPayload::hash_text(&text);
                if hash != last_text && !is_remote_hash(&state, &hash).await {
                    last_text = hash.clone();
                    let _ = send_text_to_peer(&state, text, hash).await;
                }
            }
        }
        if config.clipboard.image_enabled {
            let read = state.clipboard.lock().await.read_png();
            if let Ok(Some(png)) = read {
                let hash = ClipboardPayload::hash_bytes(&png);
                if hash != last_image && !is_remote_hash(&state, &hash).await {
                    last_image = hash;
                    let _ = send_png_to_peer(&state, png).await;
                }
            }
        }
    }
}

async fn send_text_to_peer(state: &AppState, text: String, hash: String) -> Result<()> {
    let config = state.config.read().await.clone();
    let base = config.peer_base_url();
    let source_device_id = config.device_id;
    let url = format!("{base}/peer/clipboard/text/receive");
    let token = config.shared_token.clone();
    let body = json!({"text":text,"hash":hash,"source_device_id":source_device_id});
    let _: serde_json::Value =
        tokio::task::spawn_blocking(move || peer_post_json(&url, &body, token.as_deref()))
            .await??;
    state.db.record_clipboard("text", &hash, "send")?;
    state.emit(
        "clipboard.synced",
        json!({"kind":"text","hash":hash,"target":"peer"}),
    );
    Ok(())
}

async fn send_png_to_peer(
    state: &AppState,
    png: Vec<u8>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let config = state.config.read().await.clone();
    let hash = ClipboardPayload::hash_bytes(&png);
    if png.len() as u64 > config.clipboard.max_image_bytes {
        state.emit(
            "clipboard.too_large",
            json!({"kind":"image","hash":hash,"size":png.len()}),
        );
        return Ok(Json(
            json!({"ok":true,"ignored":true,"reason":"too_large","hash":hash}),
        ));
    }
    if is_remote_hash(state, &hash).await {
        state.emit(
            "clipboard.ignored",
            json!({"kind":"image","hash":hash,"reason":"loop_prevented"}),
        );
        return Ok(Json(json!({"ok":true,"ignored":true,"hash":hash})));
    }
    let base = config.peer_base_url();
    let target = format!("{base}/peer/clipboard/image");
    let source_device_id = config.device_id.clone();
    let token = config.shared_token.clone();
    let png_len = png.len();
    let hash_for_upload = hash.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        clipboard_transport::post_png_chunks(
            &target,
            &hash_for_upload,
            &source_device_id,
            &png,
            token.as_deref(),
            IMAGE_CHUNK_SIZE,
        )
    })
    .await??;
    match outcome {
        ClipboardUploadOutcome::Uploaded => {
            state.db.record_clipboard("image", &hash, "send")?;
            state.emit(
                "clipboard.synced",
                json!({"kind":"image","hash":hash,"target":"peer","size":png_len}),
            );
            Ok(Json(json!({"ok":true,"hash":hash,"size":png_len})))
        }
        ClipboardUploadOutcome::Ignored { reason } => {
            let event = if reason.as_deref() == Some("too_large") {
                "clipboard.too_large"
            } else {
                "clipboard.ignored"
            };
            state.emit(event, json!({"kind":"image","hash":hash,"reason":reason}));
            Ok(Json(
                json!({"ok":true,"ignored":true,"reason":reason,"hash":hash}),
            ))
        }
    }
}

async fn remember_remote_hash(state: &AppState, hash: String) {
    let limit = state
        .config
        .read()
        .await
        .clipboard
        .remote_hash_window
        .max(32);
    let mut hashes = state.remote_hashes.lock().await;
    hashes.push_back(hash);
    while hashes.len() > limit {
        hashes.pop_front();
    }
}

async fn is_remote_hash(state: &AppState, hash: &str) -> bool {
    state.remote_hashes.lock().await.iter().any(|h| h == hash)
}

fn peer_post_json<T: serde::de::DeserializeOwned>(
    url: &str,
    body: &impl serde::Serialize,
    token: Option<&str>,
) -> Result<T> {
    let mut request = ureq::post(url).timeout(Duration::from_secs(30));
    if let Some(token) = token {
        request = request.set("x-wormhole-token", token);
    }
    Ok(request
        .send_json(serde_json::to_value(body)?)?
        .into_json()?)
}

fn prepared_image_key(source_device_id: &str, hash: &str) -> String {
    format!("{}:{}", source_device_id, hash)
}

fn clipboard_tmp_path(data_dir: &std::path::Path, hash: &str) -> std::path::PathBuf {
    data_dir
        .join("clipboard")
        .join(format!("{}.png.wormhole_tmp", hash))
}

fn is_hex_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
