use anyhow::{anyhow, bail, Result};
use axum::{body::Bytes, http::StatusCode, Json};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
    sync::mpsc,
};
use wormhole_core::{
    file_sha256, normalize_relative_path, safe_join, scan_manifest, ConflictStrategy,
    LocalTransferManifest, TransferDirection, TransferStatus, TransferTask, WireTransferManifest,
};

use crate::{
    dto::{RetryRequest, SendRequest},
    error::ApiError,
    state::{AppState, FailedTransfer, ReceiveFileState, ReceiveTaskState},
    transport::transfer_transport,
};

pub const CHUNK_SIZE: usize = 256 * 1024;
pub const MAX_CHUNK_SIZE: usize = 2 * 1024 * 1024;
pub const MAX_MANIFEST_FILES: usize = 100_000;

#[derive(Debug, Deserialize)]
pub struct UploadQuery {
    pub path: String,
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChunkQuery {
    pub path: String,
    pub final_chunk: bool,
    #[serde(default)]
    pub offset: u64,
    #[serde(default)]
    pub sha256: Option<String>,
}

pub async fn send_transfer(
    state: AppState,
    req: SendRequest,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.emit("transfer.scanning", json!({"status":"started"}));
    let manifest = scan_manifest(&req.paths)?;
    state.emit(
        "transfer.scanning",
        json!({
            "status":"completed",
            "task_id": manifest.task_id,
            "file_count": manifest.files.len(),
            "total_size": manifest.total_size
        }),
    );
    let task = TransferTask::new_send(&manifest, req.paths.clone());
    state.db.upsert_task(&task)?;
    state
        .tasks
        .lock()
        .await
        .insert(task.task_id.clone(), task.clone());
    state.emit("transfer.created", json!({"task": public_task(&task)}));
    let runner_state = state.clone();
    tokio::spawn(async move {
        if run_send_transfer(runner_state.clone(), manifest, req.paths)
            .await
            .is_err()
        {
            runner_state.emit(
                "daemon.error",
                json!({"error_code":"internal","error":"background transfer task failed"}),
            );
        }
    });
    Ok(Json(json!({"ok":true,"task_id":task.task_id})))
}

pub async fn restore_tasks_from_db(state: &AppState) -> Result<()> {
    let mut restored = HashMap::new();
    for mut task in state.db.tasks().unwrap_or_default() {
        if matches!(
            task.status,
            TransferStatus::Queued
                | TransferStatus::Prepared
                | TransferStatus::Transferring
                | TransferStatus::Retrying
        ) {
            task.status = TransferStatus::Failed;
            task.error_code = Some("daemon_restarted".to_string());
            task.error = Some("daemon restarted before task completed".to_string());
            task.updated_at = Utc::now();
            state.db.upsert_task(&task)?;
        }
        restored.insert(task.task_id.clone(), task);
    }
    *state.tasks.lock().await = restored;
    Ok(())
}

pub async fn run_send_transfer(
    state: AppState,
    mut manifest: LocalTransferManifest,
    paths: Vec<PathBuf>,
) -> Result<()> {
    let _permit = state.transfer_slots.clone().acquire_owned().await?;
    state.cancelled.lock().await.remove(&manifest.task_id);
    update_task(
        &state,
        &manifest.task_id,
        TransferStatus::Transferring,
        None,
        None,
        0,
    )
    .await?;
    let config = state.config.read().await.clone();
    let base = config.peer_base_url();
    let token = config.shared_token.clone();
    let prepare_url = format!("{base}/peer/transfer/prepare");
    let wire = manifest.to_wire();
    let prepared_result: Result<serde_json::Value> =
        tokio::task::spawn_blocking(move || peer_post_json(&prepare_url, &wire, token.as_deref()))
            .await?;
    if let Err(err) = prepared_result {
        mark_failed(
            &state,
            &manifest.task_id,
            classify_error(&err),
            Some(err.to_string()),
            &manifest,
            &paths,
        )
        .await?;
        return Ok(());
    }
    state.emit(
        "transfer.started",
        json!({"task_id":manifest.task_id,"direction":"send"}),
    );

    let mut transferred = task_transferred_size(&state, &manifest.task_id).await;
    for item in &mut manifest.files {
        if is_cancelled(&state, &manifest.task_id).await {
            update_task(
                &state,
                &manifest.task_id,
                TransferStatus::Cancelled,
                None,
                None,
                transferred,
            )
            .await?;
            state.emit("transfer.cancelled", json!({"task_id":manifest.task_id}));
            return Ok(());
        }
        let source_path = item.source_path.clone();
        let sha256_result = tokio::task::spawn_blocking(move || file_sha256(&source_path)).await?;
        let sha256 = match sha256_result {
            Ok(sha256) => sha256,
            Err(err) => {
                mark_failed(
                    &state,
                    &manifest.task_id,
                    classify_error(&err),
                    Some(err.to_string()),
                    &manifest,
                    &paths,
                )
                .await?;
                return Ok(());
            }
        };
        item.sha256 = Some(sha256.clone());
        let url = format!(
            "{base}/peer/transfer/upload-chunk/{}?path={}",
            manifest.task_id,
            url_escape(&item.relative_path)
        );
        let status_url = format!(
            "{base}/peer/transfer/upload-status/{}?path={}",
            manifest.task_id,
            url_escape(&item.relative_path)
        );
        let source_path = item.source_path.clone();
        let size = item.size;
        let token = state.config.read().await.shared_token.clone();
        let (tx, mut rx) = mpsc::unbounded_channel::<u64>();
        let upload_handle = tokio::task::spawn_blocking(move || {
            transfer_transport::upload_file_chunks(
                &status_url,
                &url,
                &source_path,
                size,
                Some(&sha256),
                token.as_deref(),
                CHUNK_SIZE,
                |delta| {
                    let _ = tx.send(delta);
                    Ok(())
                },
            )
        });
        loop {
            if upload_handle.is_finished() {
                break;
            }
            match tokio::time::timeout(Duration::from_millis(50), rx.recv()).await {
                Ok(Some(delta)) => {
                    transferred = transferred.saturating_add(delta).min(manifest.total_size);
                    update_progress(
                        &state,
                        &manifest.task_id,
                        &item.relative_path,
                        TransferDirection::Send,
                        transferred,
                        manifest.total_size,
                    )
                    .await?;
                }
                Ok(None) | Err(_) => {}
            }
        }
        if let Err(err) = upload_handle.await? {
            mark_failed(
                &state,
                &manifest.task_id,
                classify_error(&err),
                Some(err.to_string()),
                &manifest,
                &paths,
            )
            .await?;
            return Ok(());
        }
        while let Ok(delta) = rx.try_recv() {
            transferred = transferred.saturating_add(delta).min(manifest.total_size);
            update_progress(
                &state,
                &manifest.task_id,
                &item.relative_path,
                TransferDirection::Send,
                transferred,
                manifest.total_size,
            )
            .await?;
        }
    }
    update_task(
        &state,
        &manifest.task_id,
        TransferStatus::Completed,
        None,
        None,
        manifest.total_size,
    )
    .await?;
    if let Some(task) = state.tasks.lock().await.get(&manifest.task_id).cloned() {
        state.db.append_history(&task)?;
    }
    state.emit(
        "transfer.completed",
        json!({"task_id":manifest.task_id,"direction":"send"}),
    );
    Ok(())
}

pub async fn prepare_transfer(
    state: &AppState,
    manifest: WireTransferManifest,
) -> Result<Json<serde_json::Value>, ApiError> {
    validate_wire_manifest(&manifest)?;
    let config = state.config.read().await.clone();
    fs::create_dir_all(&config.receive_dir).await?;
    let available = fs2::available_space(&config.receive_dir)?;
    let needed = manifest
        .total_size
        .saturating_add(config.transfer.min_free_space_bytes);
    if available < needed {
        state.emit(
            "transfer.failed",
            json!({"task_id":manifest.task_id,"error_code":"disk_space","available":available,"needed":needed}),
        );
        return Err(ApiError::status(
            StatusCode::INSUFFICIENT_STORAGE,
            "disk_space",
            anyhow!("not enough disk space"),
        ));
    }

    let mut receive_files = std::collections::HashMap::new();
    for item in &manifest.files {
        let final_path = receive_final_path_locked(
            &config,
            &item.relative_path,
            receive_files
                .values()
                .map(|f: &ReceiveFileState| f.final_path.clone()),
        )?;
        let tmp_path = tmp_path_for(&final_path);
        receive_files.insert(
            item.relative_path.clone(),
            ReceiveFileState {
                relative_path: item.relative_path.clone(),
                expected_size: item.size,
                expected_sha256: item.sha256.clone(),
                final_path,
                tmp_path,
                received_size: 0,
                completed: false,
            },
        );
    }
    state.receive_tasks.lock().await.insert(
        manifest.task_id.clone(),
        ReceiveTaskState {
            files: receive_files,
        },
    );

    let now = Utc::now();
    let task = TransferTask {
        task_id: manifest.task_id.clone(),
        direction: TransferDirection::Receive,
        peer_device_id: None,
        root_name: manifest.root_name,
        item_count: manifest.files.len(),
        total_size: manifest.total_size,
        transferred_size: 0,
        status: TransferStatus::Prepared,
        error: None,
        save_path: Some(config.receive_dir),
        speed_bytes_per_sec: 0,
        eta_seconds: None,
        retry_count: 0,
        error_code: None,
        created_at: now,
        updated_at: now,
        source_paths: Vec::new(),
        parent_task_id: None,
        attempt_id: None,
    };
    state.db.upsert_task(&task)?;
    state
        .tasks
        .lock()
        .await
        .insert(task.task_id.clone(), task.clone());
    state.emit("transfer.queued", json!({"task": public_task(&task)}));
    Ok(Json(json!({"ok":true,"task_id":task.task_id})))
}

pub fn validate_wire_manifest(manifest: &WireTransferManifest) -> Result<(), ApiError> {
    if manifest.files.is_empty() {
        return Err(ApiError::status(
            StatusCode::BAD_REQUEST,
            "empty_manifest",
            anyhow!("transfer manifest has no files"),
        ));
    }
    if manifest.files.len() > MAX_MANIFEST_FILES {
        return Err(ApiError::status(
            StatusCode::PAYLOAD_TOO_LARGE,
            "manifest_too_large",
            anyhow!("transfer manifest has too many files"),
        ));
    }
    let mut seen = HashSet::with_capacity(manifest.files.len());
    let mut total = 0u64;
    for item in &manifest.files {
        normalize_relative_path(PathBuf::from(&item.relative_path)).map_err(|_| {
            ApiError::status(
                StatusCode::BAD_REQUEST,
                "unsafe_path",
                anyhow!("unsafe relative path"),
            )
        })?;
        if !seen.insert(item.relative_path.clone()) {
            return Err(ApiError::status(
                StatusCode::BAD_REQUEST,
                "duplicate_path",
                anyhow!("duplicate relative path"),
            ));
        }
        if let Some(sha256) = &item.sha256 {
            validate_sha256(sha256)?;
        }
        total = total.checked_add(item.size).ok_or_else(|| {
            ApiError::status(
                StatusCode::BAD_REQUEST,
                "size_overflow",
                anyhow!("manifest total size overflow"),
            )
        })?;
    }
    if total != manifest.total_size {
        return Err(ApiError::status(
            StatusCode::BAD_REQUEST,
            "total_size_mismatch",
            anyhow!("manifest total_size does not match files"),
        ));
    }
    Ok(())
}

pub async fn upload_status(
    state: &AppState,
    task_id: &str,
    query: UploadQuery,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut lock = state.receive_tasks.lock().await;
    let task = lock.get_mut(task_id).ok_or_else(|| {
        ApiError::status(
            StatusCode::NOT_FOUND,
            "task_not_found",
            anyhow!("task not found"),
        )
    })?;
    let file = task.files.get_mut(&query.path).ok_or_else(|| {
        ApiError::status(
            StatusCode::NOT_FOUND,
            "path_not_found",
            anyhow!("path not in manifest"),
        )
    })?;
    bind_or_check_sha256(file, query.sha256.as_deref())?;
    if file.completed {
        return Ok(Json(
            json!({"ok":true,"complete":true,"offset":file.expected_size}),
        ));
    }
    file.received_size = std::fs::metadata(&file.tmp_path)
        .map(|m| m.len())
        .unwrap_or(0);
    Ok(Json(
        json!({"ok":true,"complete":false,"offset":file.received_size}),
    ))
}

pub async fn upload_chunk(
    state: &AppState,
    task_id: &str,
    query: ChunkQuery,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    if body.len() > MAX_CHUNK_SIZE {
        return Err(ApiError::status(
            StatusCode::PAYLOAD_TOO_LARGE,
            "chunk_too_large",
            anyhow!("chunk too large"),
        ));
    }
    let (tmp_path, final_path, expected_size, expected_sha256, relative_path) = {
        let mut lock = state.receive_tasks.lock().await;
        let task = lock.get_mut(task_id).ok_or_else(|| {
            ApiError::status(
                StatusCode::NOT_FOUND,
                "task_not_found",
                anyhow!("task not found"),
            )
        })?;
        let file = task.files.get_mut(&query.path).ok_or_else(|| {
            ApiError::status(
                StatusCode::NOT_FOUND,
                "path_not_found",
                anyhow!("path not in manifest"),
            )
        })?;
        bind_or_check_sha256(file, query.sha256.as_deref())?;
        if query.offset != file.received_size {
            return Err(ApiError::status(
                StatusCode::CONFLICT,
                "offset_mismatch",
                anyhow!("upload offset mismatch"),
            ));
        }
        if query.offset > file.expected_size
            || query.offset.saturating_add(body.len() as u64) > file.expected_size
        {
            return Err(ApiError::status(
                StatusCode::BAD_REQUEST,
                "size_mismatch",
                anyhow!("chunk exceeds expected size"),
            ));
        }
        (
            file.tmp_path.clone(),
            file.final_path.clone(),
            file.expected_size,
            file.expected_sha256.clone(),
            file.relative_path.clone(),
        )
    };
    if let Some(parent) = tmp_path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let current_len = std::fs::metadata(&tmp_path).map(|m| m.len()).unwrap_or(0);
    if current_len != query.offset {
        return Err(ApiError::status(
            StatusCode::CONFLICT,
            "offset_mismatch",
            anyhow!("tmp file offset mismatch"),
        ));
    }
    {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&tmp_path)
            .await?;
        file.write_all(&body).await?;
        file.flush().await?;
    }
    let new_size = query.offset + body.len() as u64;
    if query.final_chunk {
        if new_size != expected_size {
            return Err(ApiError::status(
                StatusCode::BAD_REQUEST,
                "size_mismatch",
                anyhow!("final chunk size mismatch"),
            ));
        }
        let actual_tmp_size = std::fs::metadata(&tmp_path).map(|m| m.len()).unwrap_or(0);
        if actual_tmp_size != expected_size {
            return Err(ApiError::status(
                StatusCode::BAD_REQUEST,
                "size_mismatch",
                anyhow!("tmp file size mismatch"),
            ));
        }
        if let Some(expected) = expected_sha256.as_deref() {
            if let Err(err) = verify_tmp_hash(&tmp_path, expected).await {
                let _ = fs::remove_file(&tmp_path).await;
                mark_receive_failed(state, task_id, "integrity").await?;
                state.emit(
                    "transfer.failed",
                    json!({"task_id":task_id,"relative_path":relative_path,"error_code":"integrity"}),
                );
                return Err(err);
            }
        }
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::rename(&tmp_path, &final_path).await?;
    }
    {
        let mut lock = state.receive_tasks.lock().await;
        let task = lock.get_mut(task_id).ok_or_else(|| {
            ApiError::status(
                StatusCode::NOT_FOUND,
                "task_not_found",
                anyhow!("task not found"),
            )
        })?;
        let file = task.files.get_mut(&query.path).ok_or_else(|| {
            ApiError::status(
                StatusCode::NOT_FOUND,
                "path_not_found",
                anyhow!("path not in manifest"),
            )
        })?;
        file.received_size = new_size;
        if query.final_chunk {
            file.completed = true;
        }
    }
    record_chunk_received(
        state,
        task_id,
        &relative_path,
        body.len() as u64,
        query.final_chunk,
        final_path.clone(),
    )
    .await?;
    Ok(Json(
        json!({"ok":true,"received":body.len(),"offset":new_size}),
    ))
}

pub async fn touch_empty_file(
    state: &AppState,
    task_id: &str,
    query: UploadQuery,
) -> Result<Json<serde_json::Value>, ApiError> {
    let final_path = {
        let mut lock = state.receive_tasks.lock().await;
        let task = lock.get_mut(task_id).ok_or_else(|| {
            ApiError::status(
                StatusCode::NOT_FOUND,
                "task_not_found",
                anyhow!("task not found"),
            )
        })?;
        let file = task.files.get_mut(&query.path).ok_or_else(|| {
            ApiError::status(
                StatusCode::NOT_FOUND,
                "path_not_found",
                anyhow!("path not in manifest"),
            )
        })?;
        if file.expected_size != 0 {
            return Err(ApiError::status(
                StatusCode::BAD_REQUEST,
                "size_mismatch",
                anyhow!("touch is valid only for empty files"),
            ));
        }
        file.completed = true;
        file.final_path.clone()
    };
    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::File::create(&final_path).await?;
    record_chunk_received(state, task_id, &query.path, 0, true, final_path).await?;
    Ok(Json(json!({"ok":true,"received":0})))
}

pub async fn retry_transfer(
    state: AppState,
    req: Option<RetryRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let requested_task_id = req.and_then(|req| req.task_id);
    let failed = pop_failed_transfer(&state, requested_task_id.as_deref()).await?;
    state.failed_task_ids.lock().await.remove(&failed.task_id);
    state.cancelled.lock().await.remove(&failed.task_id);
    let task = {
        let mut lock = state.tasks.lock().await;
        let task = lock.get_mut(&failed.task_id).ok_or_else(|| {
            ApiError::status(
                StatusCode::NOT_FOUND,
                "task_not_found",
                anyhow!("task not found"),
            )
        })?;
        task.status = TransferStatus::Retrying;
        task.retry_count = task.retry_count.saturating_add(1);
        task.error = None;
        task.error_code = None;
        task.transferred_size = 0;
        task.item_count = failed.manifest.files.len();
        task.total_size = failed.manifest.total_size;
        task.root_name = failed.manifest.root_name.clone();
        task.source_paths = failed.paths.clone();
        task.updated_at = Utc::now();
        state.db.upsert_task(task)?;
        task.clone()
    };
    state.emit(
        "transfer.retrying",
        json!({"task_id":failed.task_id,"retry_count":task.retry_count}),
    );
    let runner_state = state.clone();
    let retry_manifest = build_retry_manifest(&failed);
    tokio::spawn(async move {
        let _ = run_send_transfer(runner_state, retry_manifest, failed.paths).await;
    });
    Ok(Json(json!({"ok":true,"task_id":failed.task_id})))
}

async fn pop_failed_transfer(
    state: &AppState,
    task_id: Option<&str>,
) -> Result<FailedTransfer, ApiError> {
    let mut failed = state.failed.lock().await;
    match task_id {
        Some(task_id) => {
            if let Some(index) = failed.iter().position(|item| item.task_id == task_id) {
                Ok(failed.remove(index).expect("failed queue index valid"))
            } else {
                Err(ApiError::status(
                    StatusCode::NOT_FOUND,
                    "task_not_found",
                    anyhow!("failed task not found"),
                ))
            }
        }
        None => failed.pop_back().ok_or_else(|| {
            ApiError::status(
                StatusCode::NOT_FOUND,
                "not_found",
                anyhow!("no failed transfer to retry"),
            )
        }),
    }
}

fn build_retry_manifest(failed: &FailedTransfer) -> LocalTransferManifest {
    let mut manifest = failed.manifest.clone();
    manifest.task_id = failed.task_id.clone();
    manifest
}

pub async fn cancel_transfer(
    state: &AppState,
    task_id: &str,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.cancelled.lock().await.insert(task_id.to_string());
    update_task(state, task_id, TransferStatus::Cancelled, None, None, 0).await?;
    state.emit("transfer.cancelled", json!({"task_id":task_id}));
    Ok(Json(json!({"ok":true})))
}

pub async fn mark_failed(
    state: &AppState,
    task_id: &str,
    error_code: String,
    detail: Option<String>,
    manifest: &LocalTransferManifest,
    paths: &[PathBuf],
) -> Result<()> {
    let public_error = public_error_message(&error_code);
    let message = detail
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("{public_error}: {value}"))
        .unwrap_or_else(|| public_error.to_string());
    *state.last_transfer_error_code.write().await = Some(error_code.clone());
    *state.last_transfer_error_message.write().await = Some(message.clone());
    update_task(
        state,
        task_id,
        TransferStatus::Failed,
        Some(error_code.clone()),
        Some(message.clone()),
        0,
    )
    .await?;
    let mut ids = state.failed_task_ids.lock().await;
    if ids.insert(task_id.to_string()) {
        state.failed.lock().await.push_back(FailedTransfer {
            task_id: task_id.to_string(),
            paths: paths.to_vec(),
            manifest: manifest.clone(),
        });
    }
    state.emit(
        "transfer.failed",
        json!({"task_id":task_id,"error_code":error_code,"error":message}),
    );
    Ok(())
}

async fn mark_receive_failed(state: &AppState, task_id: &str, error_code: &str) -> Result<()> {
    let msg = public_error_message(error_code).to_string();
    *state.last_transfer_error_code.write().await = Some(error_code.to_string());
    *state.last_transfer_error_message.write().await = Some(msg.clone());
    update_task(
        state,
        task_id,
        TransferStatus::Failed,
        Some(error_code.to_string()),
        Some(msg),
        task_transferred_size(state, task_id).await,
    )
    .await
}

async fn record_chunk_received(
    state: &AppState,
    task_id: &str,
    relative_path: &str,
    received: u64,
    final_chunk: bool,
    final_path: PathBuf,
) -> Result<()> {
    let transferred = increment_task_progress(state, task_id, received).await?;
    let total = task_total_size(state, task_id).await;
    update_progress(
        state,
        task_id,
        relative_path,
        TransferDirection::Receive,
        transferred,
        total,
    )
    .await?;
    if final_chunk && all_receive_files_completed(state, task_id).await {
        update_task(
            state,
            task_id,
            TransferStatus::Completed,
            None,
            None,
            transferred,
        )
        .await?;
        if let Some(task) = state.tasks.lock().await.get(task_id).cloned() {
            state.db.append_history(&task)?;
        }
        state.emit(
            "transfer.completed",
            json!({"task_id":task_id,"direction":"receive","save_path":final_path}),
        );
    }
    Ok(())
}

async fn update_task(
    state: &AppState,
    task_id: &str,
    status: TransferStatus,
    error_code: Option<String>,
    error: Option<String>,
    transferred: u64,
) -> Result<()> {
    let mut lock = state.tasks.lock().await;
    if let Some(task) = lock.get_mut(task_id) {
        task.status = status.clone();
        task.error_code = error_code.clone();
        task.error = error.clone();
        task.transferred_size = transferred.min(task.total_size);
        update_speed_eta(task);
        task.updated_at = Utc::now();
        state.db.upsert_task(task)?;
    }
    Ok(())
}

async fn increment_task_progress(state: &AppState, task_id: &str, delta: u64) -> Result<u64> {
    let mut lock = state.tasks.lock().await;
    let Some(task) = lock.get_mut(task_id) else {
        bail!("task not found");
    };
    task.status = TransferStatus::Transferring;
    task.transferred_size = task
        .transferred_size
        .saturating_add(delta)
        .min(task.total_size);
    update_speed_eta(task);
    task.updated_at = Utc::now();
    if matches!(
        task.status,
        TransferStatus::Completed | TransferStatus::Failed | TransferStatus::Cancelled
    ) {
        state.db.upsert_task(task)?;
    }
    Ok(task.transferred_size)
}

async fn update_progress(
    state: &AppState,
    task_id: &str,
    relative_path: &str,
    direction: TransferDirection,
    transferred_size: u64,
    total_size: u64,
) -> Result<()> {
    let (speed, eta) = {
        let lock = state.tasks.lock().await;
        let task = lock.get(task_id);
        (
            task.map(|t| t.speed_bytes_per_sec).unwrap_or(0),
            task.and_then(|t| t.eta_seconds),
        )
    };
    state.emit(
        "transfer.progress",
        json!({
            "task_id": task_id,
            "relative_path": relative_path,
            "direction": direction,
            "transferred_size": transferred_size,
            "total_size": total_size,
            "speed_bytes_per_sec": speed,
            "eta_seconds": eta
        }),
    );
    Ok(())
}

fn update_speed_eta(task: &mut TransferTask) {
    let elapsed = (Utc::now() - task.created_at).num_seconds().max(1) as u64;
    task.speed_bytes_per_sec = task.transferred_size / elapsed;
    task.eta_seconds = if task.speed_bytes_per_sec > 0 && task.transferred_size < task.total_size {
        Some((task.total_size - task.transferred_size).div_ceil(task.speed_bytes_per_sec))
    } else {
        None
    };
}

fn bind_or_check_sha256(
    file: &mut ReceiveFileState,
    incoming: Option<&str>,
) -> Result<(), ApiError> {
    if let Some(incoming) = incoming {
        validate_sha256(incoming)?;
        match &file.expected_sha256 {
            Some(expected) if expected != incoming => {
                return Err(ApiError::status(
                    StatusCode::CONFLICT,
                    "sha256_mismatch",
                    anyhow!("sha256 does not match prepared manifest"),
                ));
            }
            Some(_) => {}
            None => file.expected_sha256 = Some(incoming.to_string()),
        }
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), ApiError> {
    if value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(ApiError::status(
            StatusCode::BAD_REQUEST,
            "invalid_sha256",
            anyhow!("invalid sha256"),
        ))
    }
}

async fn task_transferred_size(state: &AppState, task_id: &str) -> u64 {
    state
        .tasks
        .lock()
        .await
        .get(task_id)
        .map(|task| task.transferred_size)
        .unwrap_or(0)
}

async fn task_total_size(state: &AppState, task_id: &str) -> u64 {
    state
        .tasks
        .lock()
        .await
        .get(task_id)
        .map(|task| task.total_size)
        .unwrap_or(0)
}

async fn all_receive_files_completed(state: &AppState, task_id: &str) -> bool {
    state
        .receive_tasks
        .lock()
        .await
        .get(task_id)
        .map(|task| task.files.values().all(|file| file.completed))
        .unwrap_or(false)
}

pub async fn is_cancelled(state: &AppState, task_id: &str) -> bool {
    state.cancelled.lock().await.contains(task_id)
}

fn receive_final_path_locked(
    config: &wormhole_core::AppConfig,
    relative_path: &str,
    reserved: impl Iterator<Item = PathBuf>,
) -> Result<PathBuf> {
    let path = safe_join(&config.receive_dir, relative_path)?;
    Ok(match config.transfer.conflict_strategy {
        ConflictStrategy::Overwrite | ConflictStrategy::Skip => path,
        ConflictStrategy::Rename => unique_path_excluding(&path, reserved.collect()),
    })
}

fn unique_path_excluding(path: &Path, reserved: Vec<PathBuf>) -> PathBuf {
    if !path.exists() && !reserved.iter().any(|p| p == path) {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|s| s.to_str());
    for i in 1..10_000 {
        let name = match ext {
            Some(ext) if !ext.is_empty() => format!("{stem} ({i}).{ext}"),
            _ => format!("{stem} ({i})"),
        };
        let candidate = parent.join(name);
        if !candidate.exists() && !reserved.iter().any(|p| p == &candidate) {
            return candidate;
        }
    }
    path.to_path_buf()
}

pub fn tmp_path_for(final_path: &Path) -> PathBuf {
    final_path.with_extension(format!(
        "{}wormhole_tmp",
        final_path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| format!("{s}."))
            .unwrap_or_default()
    ))
}

async fn verify_tmp_hash(path: &Path, expected: &str) -> Result<(), ApiError> {
    let path = path.to_path_buf();
    let expected = expected.to_string();
    let actual = tokio::task::spawn_blocking(move || file_sha256(&path)).await??;
    if actual == expected {
        Ok(())
    } else {
        Err(ApiError::status(
            StatusCode::UNPROCESSABLE_ENTITY,
            "integrity",
            anyhow!("sha256 mismatch"),
        ))
    }
}

pub fn classify_error(error: &anyhow::Error) -> String {
    let text = error.to_string().to_lowercase();
    if text.contains("401") || text.contains("unauthorized") || text.contains("token") {
        "auth".to_string()
    } else if text.contains("space") || text.contains("storage") || text.contains("disk") {
        "disk_space".to_string()
    } else if text.contains("sha256") || text.contains("hash") || text.contains("mismatch") {
        "integrity".to_string()
    } else if text.contains("path") || text.contains("file name") {
        "path".to_string()
    } else if text.contains("connection") || text.contains("network") || text.contains("timed out")
    {
        "network".to_string()
    } else {
        "unknown".to_string()
    }
}

fn public_error_message(error_code: &str) -> &'static str {
    match error_code {
        "auth" => "peer authentication failed",
        "disk_space" => "not enough disk space",
        "integrity" => "integrity check failed",
        "path" => "invalid path",
        "network" => "network transfer failed",
        "daemon_restarted" => "daemon restarted before task completed",
        _ => "transfer failed",
    }
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

pub fn public_task(task: &TransferTask) -> serde_json::Value {
    json!(crate::dto::TransferTaskDto::from(task))
}

pub fn url_escape(value: &str) -> String {
    value
        .bytes()
        .flat_map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![b as char]
            }
            _ => format!("%{b:02X}").chars().collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wormhole_core::{AppConfig, ConflictStrategy, WireTransferItem};

    #[test]
    fn locked_paths_do_not_change_after_conflict_resolution() {
        let base =
            std::env::temp_dir().join(format!("wormhole-path-lock-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).expect("create temp dir");
        std::fs::write(base.join("a.txt"), b"exists").expect("seed conflict");
        let mut config = AppConfig::default_at(
            &base.join("config.json"),
            53_000 + 317,
            "127.0.0.1".to_string(),
            53318,
        )
        .expect("config");
        config.receive_dir = base.clone();
        config.transfer.conflict_strategy = ConflictStrategy::Rename;
        let first = receive_final_path_locked(&config, "a.txt", std::iter::empty()).expect("first");
        let second = receive_final_path_locked(&config, "a.txt", vec![first.clone()].into_iter())
            .expect("second");
        assert_eq!(first, base.join("a (1).txt"));
        assert_eq!(second, base.join("a (2).txt"));
        assert_eq!(first, base.join("a (1).txt"));
        let _ = std::fs::remove_dir_all(base);
    }

    fn wire_manifest(files: Vec<WireTransferItem>, total_size: u64) -> WireTransferManifest {
        WireTransferManifest {
            task_id: "task".to_string(),
            root_name: "root".to_string(),
            files,
            total_size,
        }
    }

    #[test]
    fn duplicate_relative_path_is_rejected() {
        let manifest = wire_manifest(
            vec![
                WireTransferItem {
                    relative_path: "a.txt".to_string(),
                    size: 1,
                    sha256: None,
                },
                WireTransferItem {
                    relative_path: "a.txt".to_string(),
                    size: 1,
                    sha256: None,
                },
            ],
            2,
        );
        let err = validate_wire_manifest(&manifest).expect_err("duplicate path must fail");
        assert_eq!(err.code, "duplicate_path");
    }

    #[test]
    fn total_size_mismatch_is_rejected() {
        let manifest = wire_manifest(
            vec![WireTransferItem {
                relative_path: "a.txt".to_string(),
                size: 10,
                sha256: None,
            }],
            9,
        );
        let err = validate_wire_manifest(&manifest).expect_err("total mismatch must fail");
        assert_eq!(err.code, "total_size_mismatch");
    }

    #[test]
    fn unsafe_relative_path_is_rejected() {
        let manifest = wire_manifest(
            vec![WireTransferItem {
                relative_path: "../a.txt".to_string(),
                size: 1,
                sha256: None,
            }],
            1,
        );
        let err = validate_wire_manifest(&manifest).expect_err("unsafe path must fail");
        assert_eq!(err.code, "unsafe_path");
    }

    #[test]
    fn sha256_mismatch_is_rejected_after_prepare_binding() {
        let mut file = ReceiveFileState {
            relative_path: "a.txt".to_string(),
            expected_size: 1,
            expected_sha256: Some("a".repeat(64)),
            final_path: PathBuf::from("a.txt"),
            tmp_path: PathBuf::from("a.txt.wormhole_tmp"),
            received_size: 0,
            completed: false,
        };
        let err = bind_or_check_sha256(&mut file, Some(&"b".repeat(64)))
            .expect_err("sha mismatch must fail");
        assert_eq!(err.code, "sha256_mismatch");
    }

    #[test]
    fn retry_manifest_preserves_full_failed_manifest() {
        let manifest = LocalTransferManifest {
            task_id: "original".to_string(),
            root_name: "two files".to_string(),
            total_size: 30,
            files: vec![
                wormhole_core::LocalTransferItem {
                    relative_path: "a.txt".to_string(),
                    size: 10,
                    source_path: PathBuf::from("a.txt"),
                    sha256: None,
                },
                wormhole_core::LocalTransferItem {
                    relative_path: "b.txt".to_string(),
                    size: 20,
                    source_path: PathBuf::from("b.txt"),
                    sha256: None,
                },
            ],
        };
        let failed = FailedTransfer {
            task_id: "original".to_string(),
            paths: vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")],
            manifest,
        };
        let retry = build_retry_manifest(&failed);
        assert_eq!(retry.task_id, "original");
        assert_eq!(retry.root_name, "two files");
        assert_eq!(retry.files.len(), 2);
        assert_eq!(retry.total_size, 30);
    }
}
