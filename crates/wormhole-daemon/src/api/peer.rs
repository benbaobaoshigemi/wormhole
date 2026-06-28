use axum::{
    body::Bytes,
    extract::{Path as AxumPath, Query, State},
    http::HeaderMap,
    Json,
};
use wormhole_core::{PublicDevice, WireTransferManifest};

use crate::{
    auth,
    dto::ImagePrepareResponse,
    error::ApiError,
    service::{
        clipboard::{self, ImageChunkQuery, ImagePrepareRequest, ReceiveText},
        transfer::{self, ChunkQuery, UploadQuery},
    },
    state::AppState,
};

pub async fn handshake(State(state): State<AppState>) -> Json<PublicDevice> {
    Json(PublicDevice::from(&*state.config.read().await))
}

pub async fn prepare_transfer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(manifest): Json<WireTransferManifest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth::verify_peer_auth(&state, &headers).await?;
    transfer::prepare_transfer(&state, manifest).await
}

pub async fn upload_status(
    State(state): State<AppState>,
    AxumPath(task_id): AxumPath<String>,
    Query(query): Query<UploadQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth::verify_peer_auth(&state, &headers).await?;
    transfer::upload_status(&state, &task_id, query).await
}

pub async fn upload_chunk(
    State(state): State<AppState>,
    AxumPath(task_id): AxumPath<String>,
    Query(query): Query<ChunkQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth::verify_peer_auth(&state, &headers).await?;
    transfer::upload_chunk(&state, &task_id, query, body).await
}

pub async fn touch_empty_file(
    State(state): State<AppState>,
    AxumPath(task_id): AxumPath<String>,
    Query(query): Query<UploadQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth::verify_peer_auth(&state, &headers).await?;
    transfer::touch_empty_file(&state, &task_id, query).await
}

pub async fn receive_text(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ReceiveText>,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth::verify_peer_auth(&state, &headers).await?;
    clipboard::receive_text(&state, req).await
}

pub async fn prepare_image_clipboard(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ImagePrepareRequest>,
) -> Result<Json<ImagePrepareResponse>, ApiError> {
    auth::verify_peer_auth(&state, &headers).await?;
    clipboard::prepare_image_clipboard(&state, req).await
}

pub async fn receive_image_chunk(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ImageChunkQuery>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    auth::verify_peer_auth(&state, &headers).await?;
    clipboard::receive_image_chunk(&state, query, body).await
}
