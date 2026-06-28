use anyhow::{anyhow, Result};
mod clipboard_transport;
mod transfer_transport;

use axum::{
    body::{Body, Bytes},
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        Html, IntoResponse,
    },
    routing::{get, post, put},
    Json, Router,
};
use chrono::Utc;
use futures_util::{Stream, StreamExt};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    convert::Infallible,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
    sync::{broadcast, Mutex, RwLock, Semaphore},
};
use tower_http::cors::CorsLayer;
use wormhole_core::{
    file_sha256, safe_join, scan_manifest, AppConfig, ClipboardPayload, ClipboardPort,
    ConflictStrategy, ConnectionStatus, Event, EventLog, HistoryDb, PublicDevice,
    TransferDirection, TransferManifest, TransferStatus, TransferTask,
};
use wormhole_platform::SystemClipboard;

#[derive(Clone)]
struct AppState {
    config_path: PathBuf,
    config: Arc<RwLock<AppConfig>>,
    status: Arc<RwLock<ConnectionStatus>>,
    peer: Arc<RwLock<Option<PublicDevice>>>,
    db: HistoryDb,
    events: EventLog,
    event_tx: broadcast::Sender<Event>,
    tasks: Arc<Mutex<HashMap<String, TransferTask>>>,
    failed: Arc<Mutex<VecDeque<Vec<PathBuf>>>>,
    cancelled: Arc<Mutex<HashSet<String>>>,
    transfer_slots: Arc<Semaphore>,
    remote_hashes: Arc<Mutex<VecDeque<String>>>,
    clipboard: Arc<Mutex<SystemClipboard>>,
}

#[derive(Debug, Deserialize)]
struct ServeArgs {
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("wormhole_daemon=info,tower_http=warn")
        .init();
    let args = parse_args()?;
    let config = AppConfig::load(&args.config)?;
    config.save(&args.config)?;
    let db = HistoryDb::open(config.data_dir.join("wormhole.sqlite3"))?;
    let (event_tx, _) = broadcast::channel(256);
    let transfer_slots = Arc::new(Semaphore::new(config.transfer.max_concurrent_tasks.max(1)));
    let state = AppState {
        config_path: args.config,
        config: Arc::new(RwLock::new(config)),
        status: Arc::new(RwLock::new(ConnectionStatus::Unconfigured)),
        peer: Arc::new(RwLock::new(None)),
        db,
        events: EventLog::new(500),
        event_tx,
        tasks: Arc::new(Mutex::new(HashMap::new())),
        failed: Arc::new(Mutex::new(VecDeque::new())),
        cancelled: Arc::new(Mutex::new(HashSet::new())),
        transfer_slots,
        remote_hashes: Arc::new(Mutex::new(VecDeque::new())),
        clipboard: Arc::new(Mutex::new(SystemClipboard::new()?)),
    };
    let app_state = state.clone();
    tokio::spawn(async move { clipboard_loop(app_state).await });
    let connection_state = state.clone();
    tokio::spawn(async move { connection_loop(connection_state).await });
    let history_state = state.clone();
    tokio::spawn(async move { history_prune_loop(history_state).await });
    let bind = {
        let config = state.config.read().await;
        format!("{}:{}", config.bind_host, config.port)
    };
    let router = Router::new()
        .route("/", get(index))
        .route("/api/handshake", get(handshake))
        .route("/api/state", get(api_state))
        .route("/api/events", get(api_events))
        .route("/api/settings", get(settings))
        .route("/api/settings/update", post(update_settings))
        .route("/api/connect", post(connect))
        .route("/api/transfer/send", post(send_transfer))
        .route("/api/transfer/cancel", post(cancel_transfer))
        .route("/api/transfer/retry", post(retry_transfer))
        .route("/api/transfer/tasks", get(tasks))
        .route("/api/transfer/history", get(history))
        .route("/api/transfer/history/clear", post(clear_history))
        .route("/api/transfer/prepare", post(prepare_transfer))
        .route("/api/transfer/upload-status/:task_id", get(upload_status))
        .route("/api/transfer/upload/:task_id", put(upload_file))
        .route("/api/transfer/upload-chunk/:task_id", post(upload_chunk))
        .route("/api/transfer/touch/:task_id", post(touch_empty_file))
        .route("/api/clipboard/status", get(clipboard_status))
        .route("/api/clipboard/enable", post(clipboard_enable))
        .route("/api/clipboard/disable", post(clipboard_disable))
        .route("/api/clipboard/system/read-send-text", post(read_send_text))
        .route(
            "/api/clipboard/system/read-send-image",
            post(read_send_image),
        )
        .route("/api/clipboard/text/receive", post(receive_text))
        .route("/api/clipboard/image/receive", post(receive_image))
        .route(
            "/api/clipboard/image/prepare",
            post(prepare_image_clipboard),
        )
        .route("/api/clipboard/image/chunk", post(receive_image_chunk))
        .layer(CorsLayer::permissive())
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("wormhole daemon listening on {}", bind);
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

fn parse_args() -> Result<ServeArgs> {
    let mut args = std::env::args().skip(1);
    let mut config = None;
    while let Some(arg) = args.next() {
        if arg == "--config" {
            config = args.next().map(PathBuf::from);
        }
    }
    Ok(ServeArgs {
        config: config.ok_or_else(|| anyhow!("usage: wormhole-daemon --config <config.json>"))?,
    })
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../../../apps/desktop-ui/index.html"))
}

async fn handshake(State(state): State<AppState>) -> Json<PublicDevice> {
    Json(PublicDevice::from(&*state.config.read().await))
}

async fn api_state(State(state): State<AppState>) -> Json<serde_json::Value> {
    let config = state.config.read().await;
    Json(json!({
        "device": PublicDevice::from(&*config),
        "status": *state.status.read().await,
        "peer": *state.peer.read().await,
        "settings": *config,
        "tasks": state.db.tasks().unwrap_or_default(),
        "events": state.events.latest(100)
    }))
}

async fn settings(State(state): State<AppState>) -> Json<AppConfig> {
    Json(state.config.read().await.clone())
}

async fn update_settings(
    State(state): State<AppState>,
    Json(next): Json<AppConfig>,
) -> Result<Json<AppConfig>, ApiError> {
    next.save(&state.config_path)?;
    *state.config.write().await = next.clone();
    emit(&state, "settings.updated", json!({}));
    Ok(Json(next))
}

async fn connect(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    connect_inner(&state).await
}

async fn connect_inner(state: &AppState) -> Result<Json<serde_json::Value>, ApiError> {
    *state.status.write().await = ConnectionStatus::Connecting;
    emit(&state, "connection.changed", json!({"status":"connecting"}));
    let config = state.config.read().await.clone();
    let base = config.peer_base_url();
    let handshake_url = format!("{base}/api/handshake");
    let handshake_result =
        tokio::task::spawn_blocking(move || peer_get_json::<PublicDevice>(&handshake_url)).await?;
    match handshake_result {
        Ok(peer) => {
            if peer.protocol_version < config.min_peer_protocol_version
                || peer.protocol_version > config.max_peer_protocol_version
            {
                *state.status.write().await = ConnectionStatus::Failed;
                let error = format!(
                    "peer protocol {} is outside supported range {}..={}",
                    peer.protocol_version,
                    config.min_peer_protocol_version,
                    config.max_peer_protocol_version
                );
                emit(
                    &state,
                    "connection.changed",
                    json!({"status":"failed","error":error}),
                );
                return Err(ApiError::status(
                    StatusCode::UPGRADE_REQUIRED,
                    anyhow!(error),
                ));
            }
            *state.status.write().await = ConnectionStatus::Connected;
            *state.peer.write().await = Some(peer.clone());
            emit(
                &state,
                "connection.changed",
                json!({"status":"connected","peer":peer}),
            );
            Ok(Json(json!({"ok":true,"peer":peer})))
        }
        Err(err) => {
            *state.status.write().await = ConnectionStatus::PeerOffline;
            emit(
                &state,
                "connection.changed",
                json!({"status":"peer_offline","error":format!("{err:?}")}),
            );
            Err(ApiError::status(
                StatusCode::SERVICE_UNAVAILABLE,
                err.into(),
            ))
        }
    }
}

async fn connection_loop(state: AppState) {
    loop {
        let config = state.config.read().await.clone();
        let delay = if matches!(*state.status.read().await, ConnectionStatus::Connected) {
            config.connection.heartbeat_millis
        } else {
            config.connection.reconnect_millis
        };
        tokio::time::sleep(Duration::from_millis(delay.max(500))).await;
        if !config.auto_connect {
            continue;
        }
        let _ = connect_inner(&state).await;
    }
}

async fn history_prune_loop(state: AppState) {
    loop {
        let retention = state.config.read().await.history_retention_days;
        let _ = state.db.prune_history(retention);
        tokio::time::sleep(Duration::from_secs(6 * 60 * 60)).await;
    }
}

async fn api_events(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let latest = state.events.latest(100);
    let mut rx = state.event_tx.subscribe();
    let stream = async_stream::stream! {
        for event in latest {
            if let Ok(data) = serde_json::to_string(&event) {
                yield Ok(SseEvent::default().event(event.event_type).data(data));
            }
        }
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Ok(data) = serde_json::to_string(&event) {
                        yield Ok(SseEvent::default().event(event.event_type).data(data));
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[derive(Debug, Deserialize)]
struct SendRequest {
    paths: Vec<PathBuf>,
}

async fn send_transfer(
    State(state): State<AppState>,
    Json(req): Json<SendRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let manifest = scan_manifest(&req.paths)?;
    let task = TransferTask::new_send(&manifest, req.paths.clone());
    state.db.upsert_task(&task)?;
    state
        .tasks
        .lock()
        .await
        .insert(task.task_id.clone(), task.clone());
    emit(&state, "transfer.created", json!({"task":task}));
    let runner_state = state.clone();
    tokio::spawn(async move {
        if let Err(err) = run_send_transfer(runner_state.clone(), manifest, req.paths).await {
            emit(
                &runner_state,
                "daemon.error",
                json!({"error":err.to_string()}),
            );
        }
    });
    Ok(Json(json!({"ok":true,"task_id":task.task_id})))
}

async fn run_send_transfer(
    state: AppState,
    manifest: TransferManifest,
    paths: Vec<PathBuf>,
) -> Result<()> {
    let _permit = state.transfer_slots.clone().acquire_owned().await?;
    update_task(
        &state,
        &manifest.task_id,
        TransferStatus::Transferring,
        None,
        0,
    )
    .await?;
    let config = state.config.read().await.clone();
    let base = config.peer_base_url();
    let token = config.shared_token.clone();
    let prepare_url = format!("{base}/api/transfer/prepare");
    let prepare_manifest = manifest.clone();
    let prepared_result: Result<serde_json::Value> = tokio::task::spawn_blocking(move || {
        peer_post_json(&prepare_url, &prepare_manifest, token.as_deref())
    })
    .await?;
    let prepared = match prepared_result {
        Ok(prepared) => prepared,
        Err(err) => {
            mark_failed(
                &state,
                &manifest.task_id,
                classify_error(&err),
                err.to_string(),
                paths.clone(),
            )
            .await?;
            return Ok(());
        }
    };
    emit(
        &state,
        "transfer.started",
        json!({"task_id":manifest.task_id,"peer":prepared}),
    );
    let mut transferred = 0u64;
    for item in &manifest.files {
        if is_cancelled(&state, &manifest.task_id).await {
            update_task(
                &state,
                &manifest.task_id,
                TransferStatus::Cancelled,
                None,
                transferred,
            )
            .await?;
            emit(
                &state,
                "transfer.cancelled",
                json!({"task_id":manifest.task_id}),
            );
            return Ok(());
        }
        let url = format!(
            "{base}/api/transfer/upload-chunk/{}?path={}",
            manifest.task_id,
            url_escape(&item.relative_path)
        );
        let status_url = format!(
            "{base}/api/transfer/upload-status/{}?path={}",
            manifest.task_id,
            url_escape(&item.relative_path)
        );
        let source_path = item.source_path.clone();
        let size = item.size;
        let sha256 = item.sha256.clone();
        let token = state.config.read().await.shared_token.clone();
        let upload_result = tokio::task::spawn_blocking(move || {
            transfer_transport::upload_file_chunks(
                &status_url,
                &url,
                &source_path,
                size,
                sha256.as_deref(),
                token.as_deref(),
                CHUNK_SIZE,
            )
        })
        .await?;
        if let Err(err) = upload_result {
            mark_failed(
                &state,
                &manifest.task_id,
                classify_error(&err),
                err.to_string(),
                vec![item.source_path.clone()],
            )
            .await?;
            return Ok(());
        }
        transferred += item.size;
        emit(
            &state,
            "transfer.progress",
            json!({
                "task_id": manifest.task_id,
                "relative_path": item.relative_path,
                "transferred_size": transferred,
                "total_size": manifest.total_size
            }),
        );
    }
    update_task(
        &state,
        &manifest.task_id,
        TransferStatus::Completed,
        None,
        manifest.total_size,
    )
    .await?;
    let task = state.tasks.lock().await.get(&manifest.task_id).cloned();
    if let Some(task) = task {
        state.db.append_history(&task)?;
    }
    emit(
        &state,
        "transfer.completed",
        json!({"task_id":manifest.task_id}),
    );
    Ok(())
}

async fn prepare_transfer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(manifest): Json<TransferManifest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    verify_peer_auth(&state, &headers).await?;
    let config = state.config.read().await.clone();
    fs::create_dir_all(&config.receive_dir).await?;
    let available = fs2::available_space(&config.receive_dir)?;
    let needed = manifest
        .total_size
        .saturating_add(config.transfer.min_free_space_bytes);
    if available < needed {
        emit(
            &state,
            "transfer.failed",
            json!({"task_id":manifest.task_id,"error_code":"disk_space","available":available,"needed":needed}),
        );
        return Err(ApiError::status(
            StatusCode::INSUFFICIENT_STORAGE,
            anyhow!("not enough disk space"),
        ));
    }
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
    };
    state.db.upsert_task(&task)?;
    state
        .tasks
        .lock()
        .await
        .insert(task.task_id.clone(), task.clone());
    emit(&state, "transfer.queued", json!({"task":task}));
    Ok(Json(json!({"ok":true,"task_id":task.task_id})))
}

#[derive(Debug, Deserialize)]
struct UploadQuery {
    path: String,
    #[serde(default)]
    sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChunkQuery {
    path: String,
    final_chunk: bool,
    #[serde(default)]
    offset: u64,
    #[serde(default)]
    sha256: Option<String>,
}

async fn upload_status(
    State(state): State<AppState>,
    AxumPath(_task_id): AxumPath<String>,
    Query(query): Query<UploadQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    verify_peer_auth(&state, &headers).await?;
    let config = state.config.read().await.clone();
    let final_path = receive_final_path(&config, &query.path)?;
    if final_path.exists() {
        if matches!(config.transfer.conflict_strategy, ConflictStrategy::Skip) {
            return Ok(Json(
                json!({"ok":true,"complete":true,"offset":0,"skipped":true}),
            ));
        }
        if config.transfer.verify_hash {
            if let Some(expected) = &query.sha256 {
                if file_sha256(&final_path).ok().as_deref() == Some(expected.as_str()) {
                    return Ok(Json(json!({"ok":true,"complete":true,"offset":0})));
                }
            }
        } else {
            return Ok(Json(json!({"ok":true,"complete":true,"offset":0})));
        }
    }
    let tmp_path = tmp_path_for(&final_path);
    let offset = if config.transfer.resume_enabled {
        std::fs::metadata(&tmp_path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };
    Ok(Json(json!({"ok":true,"complete":false,"offset":offset})))
}

async fn upload_file(
    State(state): State<AppState>,
    AxumPath(task_id): AxumPath<String>,
    Query(query): Query<UploadQuery>,
    headers: HeaderMap,
    body: Body,
) -> Result<Json<serde_json::Value>, ApiError> {
    verify_peer_auth(&state, &headers).await?;
    let config = state.config.read().await.clone();
    let final_path = receive_final_path(&config, &query.path)?;
    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let tmp_path = final_path.with_extension(format!(
        "{}wormhole_tmp",
        final_path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| format!("{s}."))
            .unwrap_or_default()
    ));
    let mut file = fs::File::create(&tmp_path).await?;
    let mut transferred = 0u64;
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|err| anyhow!(err.to_string()))?;
        file.write_all(&chunk).await?;
        transferred += chunk.len() as u64;
        update_task_progress(&state, &task_id, transferred).await?;
        emit(
            &state,
            "transfer.progress",
            json!({"task_id":task_id,"relative_path":query.path,"received_size":transferred}),
        );
    }
    file.flush().await?;
    drop(file);
    if let Some(expected) = &query.sha256 {
        verify_tmp_hash(&tmp_path, expected).await?;
    }
    fs::rename(&tmp_path, &final_path).await?;
    let existing_task = {
        let lock = state.tasks.lock().await;
        lock.get(&task_id).cloned()
    };
    if let Some(mut task) = existing_task {
        let total = task.total_size;
        task.transferred_size = (task.transferred_size + transferred).min(total);
        task.status = if task.transferred_size >= total {
            TransferStatus::Completed
        } else {
            TransferStatus::Transferring
        };
        task.updated_at = Utc::now();
        state.db.upsert_task(&task)?;
        state
            .tasks
            .lock()
            .await
            .insert(task_id.clone(), task.clone());
        if matches!(task.status, TransferStatus::Completed) {
            state.db.append_history(&task)?;
            emit(
                &state,
                "transfer.completed",
                json!({"task_id":task_id,"save_path":final_path}),
            );
        }
    }
    Ok(Json(json!({"ok":true,"path":final_path})))
}

async fn upload_chunk(
    State(state): State<AppState>,
    AxumPath(task_id): AxumPath<String>,
    Query(query): Query<ChunkQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    verify_peer_auth(&state, &headers).await?;
    let config = state.config.read().await.clone();
    let final_path = receive_final_path(&config, &query.path)?;
    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let tmp_path = tmp_path_for(&final_path);
    if query.offset == 0 && !config.transfer.resume_enabled {
        let _ = fs::remove_file(&tmp_path).await;
    }
    let current_len = std::fs::metadata(&tmp_path).map(|m| m.len()).unwrap_or(0);
    if current_len != query.offset {
        return Err(ApiError::status(
            StatusCode::CONFLICT,
            anyhow!(
                "upload offset mismatch: expected {}, got {}",
                current_len,
                query.offset
            ),
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
    let received = body.len();
    if query.final_chunk {
        if let Some(expected) = &query.sha256 {
            verify_tmp_hash(&tmp_path, expected).await?;
        }
        fs::rename(&tmp_path, &final_path).await?;
    }
    let state_for_update = state.clone();
    let task_for_update = task_id.clone();
    let path_for_update = query.path.clone();
    let final_path_for_update = final_path.clone();
    let final_chunk = query.final_chunk;
    tokio::spawn(async move {
        let _ = record_chunk_received(
            state_for_update,
            task_for_update,
            path_for_update,
            received as u64,
            final_chunk,
            final_path_for_update,
        )
        .await;
    });
    Ok(Json(
        json!({"ok":true,"path":final_path,"received":received}),
    ))
}

async fn record_chunk_received(
    state: AppState,
    task_id: String,
    relative_path: String,
    received: u64,
    final_chunk: bool,
    final_path: PathBuf,
) -> Result<()> {
    update_task_progress(&state, &task_id, received).await?;
    emit(
        &state,
        "transfer.progress",
        json!({"task_id":task_id,"relative_path":relative_path,"received_chunk":received,"final_chunk":final_chunk}),
    );
    if final_chunk {
        let existing_task = {
            let lock = state.tasks.lock().await;
            lock.get(&task_id).cloned()
        };
        if let Some(mut task) = existing_task {
            if task.transferred_size >= task.total_size {
                task.status = TransferStatus::Completed;
                task.updated_at = Utc::now();
                state.db.upsert_task(&task)?;
                state.db.append_history(&task)?;
                state
                    .tasks
                    .lock()
                    .await
                    .insert(task_id.clone(), task.clone());
                emit(
                    &state,
                    "transfer.completed",
                    json!({"task_id":task_id,"save_path":final_path}),
                );
            }
        }
    }
    Ok(())
}

async fn retry_transfer(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let paths = state.failed.lock().await.pop_back().ok_or_else(|| {
        ApiError::status(
            StatusCode::NOT_FOUND,
            anyhow!("no failed transfer to retry"),
        )
    })?;
    emit(&state, "transfer.retrying", json!({"paths":paths}));
    let manifest = scan_manifest(&paths)?;
    let mut task = TransferTask::new_send(&manifest, paths.clone());
    task.retry_count = 1;
    state.db.upsert_task(&task)?;
    state
        .tasks
        .lock()
        .await
        .insert(task.task_id.clone(), task.clone());
    let runner_state = state.clone();
    tokio::spawn(async move {
        let _ = run_send_transfer(runner_state, manifest, paths).await;
    });
    Ok(Json(json!({"ok":true,"task_id":task.task_id})))
}

#[derive(Debug, Deserialize)]
struct CancelRequest {
    task_id: String,
}

async fn cancel_transfer(
    State(state): State<AppState>,
    Json(req): Json<CancelRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.cancelled.lock().await.insert(req.task_id.clone());
    update_task(&state, &req.task_id, TransferStatus::Cancelled, None, 0).await?;
    state.db.delete_task(&req.task_id)?;
    emit(&state, "transfer.cancelled", json!({"task_id":req.task_id}));
    Ok(Json(json!({"ok":true})))
}

async fn tasks(State(state): State<AppState>) -> Json<Vec<TransferTask>> {
    Json(state.db.tasks().unwrap_or_default())
}

async fn history(State(state): State<AppState>) -> Json<Vec<TransferTask>> {
    Json(state.db.history(100).unwrap_or_default())
}

async fn clear_history(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    state.db.clear_history()?;
    emit(&state, "transfer.history_cleared", json!({}));
    Ok(Json(json!({"ok":true})))
}

async fn clipboard_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let config = state.config.read().await;
    Json(json!({"clipboard":config.clipboard}))
}

async fn clipboard_enable(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut config = state.config.write().await;
    config.clipboard.enabled = true;
    config.save(&state.config_path)?;
    emit(
        &state,
        "settings.updated",
        json!({"clipboard_enabled":true}),
    );
    Ok(Json(json!({"ok":true})))
}

async fn clipboard_disable(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut config = state.config.write().await;
    config.clipboard.enabled = false;
    config.save(&state.config_path)?;
    emit(
        &state,
        "settings.updated",
        json!({"clipboard_enabled":false}),
    );
    Ok(Json(json!({"ok":true})))
}

async fn read_send_text(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let text = {
        let mut clipboard = state.clipboard.lock().await;
        clipboard
            .read_text()?
            .ok_or_else(|| anyhow!("clipboard has no text"))?
    };
    let hash = ClipboardPayload::hash_text(&text);
    if is_remote_hash(&state, &hash).await {
        emit(
            &state,
            "clipboard.ignored",
            json!({"kind":"text","hash":hash,"reason":"loop_prevented"}),
        );
        return Ok(Json(json!({"ok":true,"ignored":true,"hash":hash})));
    }
    send_text_to_peer(&state, text, hash.clone()).await?;
    Ok(Json(json!({"ok":true,"hash":hash})))
}

async fn read_send_image(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let png = {
        let mut clipboard = state.clipboard.lock().await;
        clipboard
            .read_png()?
            .ok_or_else(|| anyhow!("clipboard has no image"))?
    };
    send_png_to_peer(&state, png).await
}

#[derive(Debug, Deserialize)]
struct ReceiveText {
    text: String,
    hash: String,
    source_device_id: String,
}

async fn receive_text(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ReceiveText>,
) -> Result<Json<serde_json::Value>, ApiError> {
    verify_peer_auth(&state, &headers).await?;
    {
        let config = state.config.read().await;
        if req.source_device_id == config.device_id {
            emit(
                &state,
                "clipboard.ignored",
                json!({"kind":"text","hash":req.hash,"reason":"self_source"}),
            );
            return Ok(Json(json!({"ok":true,"ignored":true})));
        }
    }
    state.clipboard.lock().await.write_text(&req.text)?;
    remember_remote_hash(&state, req.hash.clone()).await;
    state.db.record_clipboard("text", &req.hash, "receive")?;
    emit(
        &state,
        "clipboard.synced",
        json!({"kind":"text","hash":req.hash,"source_device_id":req.source_device_id}),
    );
    Ok(Json(json!({"ok":true})))
}

async fn receive_image(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    verify_peer_auth(&state, &headers).await?;
    let hash = header(&headers, "x-wormhole-hash")?;
    let source = header(&headers, "x-wormhole-source-device-id")?;
    {
        let config = state.config.read().await;
        if source == config.device_id {
            emit(
                &state,
                "clipboard.ignored",
                json!({"kind":"image","hash":hash,"reason":"self_source"}),
            );
            return Ok(Json(json!({"ok":true,"ignored":true})));
        }
        if body.len() as u64 > config.clipboard.max_image_bytes {
            emit(
                &state,
                "clipboard.too_large",
                json!({"kind":"image","hash":hash,"size":body.len()}),
            );
            return Ok(Json(json!({"ok":true,"ignored":true,"reason":"too_large"})));
        }
    }
    state.clipboard.lock().await.write_png(&body)?;
    remember_remote_hash(&state, hash.clone()).await;
    state.db.record_clipboard("image", &hash, "receive")?;
    emit(
        &state,
        "clipboard.synced",
        json!({"kind":"image","hash":hash,"size":body.len(),"source_device_id":source}),
    );
    Ok(Json(json!({"ok":true,"hash":hash,"size":body.len()})))
}

#[derive(Debug, Deserialize)]
struct ImagePrepareRequest {
    hash: String,
    source_device_id: String,
    size: u64,
}

async fn prepare_image_clipboard(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ImagePrepareRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    verify_peer_auth(&state, &headers).await?;
    let config = state.config.read().await;
    if req.source_device_id == config.device_id {
        emit(
            &state,
            "clipboard.ignored",
            json!({"kind":"image","hash":req.hash,"reason":"self_source"}),
        );
        return Ok(Json(json!({"ok":true,"ignored":true})));
    }
    if req.size > config.clipboard.max_image_bytes {
        emit(
            &state,
            "clipboard.too_large",
            json!({"kind":"image","hash":req.hash,"size":req.size}),
        );
        return Ok(Json(json!({"ok":true,"ignored":true,"reason":"too_large"})));
    }
    let dir = config.data_dir.join("clipboard");
    fs::create_dir_all(&dir).await?;
    let tmp = dir.join(format!("{}.png.wormhole_tmp", req.hash));
    fs::File::create(&tmp).await?;
    Ok(Json(json!({"ok":true,"hash":req.hash})))
}

#[derive(Debug, Deserialize)]
struct ImageChunkQuery {
    hash: String,
    source_device_id: String,
    final_chunk: bool,
    #[serde(default)]
    offset: u64,
}

async fn receive_image_chunk(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ImageChunkQuery>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    verify_peer_auth(&state, &headers).await?;
    let config = state.config.read().await.clone();
    if query.source_device_id == config.device_id {
        emit(
            &state,
            "clipboard.ignored",
            json!({"kind":"image","hash":query.hash,"reason":"self_source"}),
        );
        return Ok(Json(json!({"ok":true,"ignored":true})));
    }
    let tmp = config
        .data_dir
        .join("clipboard")
        .join(format!("{}.png.wormhole_tmp", query.hash));
    let current_len = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
    if current_len != query.offset {
        return Err(ApiError::status(
            StatusCode::CONFLICT,
            anyhow!(
                "clipboard image offset mismatch: expected {}, got {}",
                current_len,
                query.offset
            ),
        ));
    }
    {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&tmp)
            .await?;
        file.write_all(&body).await?;
        file.flush().await?;
    }
    if !query.final_chunk {
        return Ok(Json(json!({"ok":true,"received":body.len()})));
    }
    let png = fs::read(&tmp).await?;
    let _ = fs::remove_file(&tmp).await;
    let actual_hash = ClipboardPayload::hash_bytes(&png);
    if actual_hash != query.hash {
        emit(
            &state,
            "clipboard.failed",
            json!({"kind":"image","hash":query.hash,"error_code":"integrity"}),
        );
        return Err(ApiError::status(
            StatusCode::UNPROCESSABLE_ENTITY,
            anyhow!("clipboard image hash mismatch"),
        ));
    }
    if png.len() as u64 > config.clipboard.max_image_bytes {
        emit(
            &state,
            "clipboard.too_large",
            json!({"kind":"image","hash":query.hash,"size":png.len()}),
        );
        return Ok(Json(json!({"ok":true,"ignored":true,"reason":"too_large"})));
    }
    state.clipboard.lock().await.write_png(&png)?;
    remember_remote_hash(&state, query.hash.clone()).await;
    state.db.record_clipboard("image", &query.hash, "receive")?;
    emit(
        &state,
        "clipboard.synced",
        json!({"kind":"image","hash":query.hash,"size":png.len(),"source_device_id":query.source_device_id}),
    );
    Ok(Json(json!({"ok":true,"hash":query.hash,"size":png.len()})))
}

async fn clipboard_loop(state: AppState) {
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
    let url = format!("{base}/api/clipboard/text/receive");
    let token = config.shared_token.clone();
    let body = json!({"text":text,"hash":hash,"source_device_id":source_device_id});
    let _: serde_json::Value = tokio::task::spawn_blocking(move || {
        peer_post_json(&url, &body, token.as_deref())
    })
    .await??;
    state.db.record_clipboard("text", &hash, "send")?;
    emit(
        state,
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
        emit(
            state,
            "clipboard.too_large",
            json!({"kind":"image","hash":hash,"size":png.len()}),
        );
        return Ok(Json(
            json!({"ok":true,"ignored":true,"reason":"too_large","hash":hash}),
        ));
    }
    if is_remote_hash(state, &hash).await {
        emit(
            state,
            "clipboard.ignored",
            json!({"kind":"image","hash":hash,"reason":"loop_prevented"}),
        );
        return Ok(Json(json!({"ok":true,"ignored":true,"hash":hash})));
    }
    let base = config.peer_base_url();
    let target = format!("{base}/api/clipboard/image");
    let source_device_id = config.device_id.clone();
    let token = config.shared_token.clone();
    let png_len = png.len();
    let hash_for_upload = hash.clone();
    tokio::task::spawn_blocking(move || {
        clipboard_transport::post_png_chunks(
            &target,
            &hash_for_upload,
            &source_device_id,
            &png,
            token.as_deref(),
            CHUNK_SIZE,
        )
    })
    .await??;
    state.db.record_clipboard("image", &hash, "send")?;
    emit(
        state,
        "clipboard.synced",
        json!({"kind":"image","hash":hash,"target":"peer","size":png_len}),
    );
    Ok(Json(json!({"ok":true,"hash":hash,"size":png_len})))
}

async fn update_task(
    state: &AppState,
    task_id: &str,
    status: TransferStatus,
    error: Option<String>,
    transferred: u64,
) -> Result<()> {
    update_task_with_code(state, task_id, status, None, error, transferred).await
}

async fn update_task_with_code(
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
        task.transferred_size = transferred;
        update_speed_eta(task);
        task.updated_at = Utc::now();
        state.db.upsert_task(task)?;
        emit(
            state,
            "transfer.progress",
            json!({"task_id":task_id,"status":status,"transferred_size":transferred,"error_code":error_code,"error":error}),
        );
    }
    Ok(())
}

async fn update_task_progress(state: &AppState, task_id: &str, delta: u64) -> Result<()> {
    let mut lock = state.tasks.lock().await;
    if let Some(task) = lock.get_mut(task_id) {
        task.status = TransferStatus::Transferring;
        task.transferred_size = (task.transferred_size + delta).min(task.total_size);
        update_speed_eta(task);
        task.updated_at = Utc::now();
        state.db.upsert_task(task)?;
    }
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

async fn is_cancelled(state: &AppState, task_id: &str) -> bool {
    state.cancelled.lock().await.contains(task_id)
}

async fn mark_failed(
    state: &AppState,
    task_id: &str,
    error_code: String,
    error: String,
    paths: Vec<PathBuf>,
) -> Result<()> {
    update_task_with_code(
        state,
        task_id,
        TransferStatus::Failed,
        Some(error_code.clone()),
        Some(error.clone()),
        0,
    )
    .await?;
    state.failed.lock().await.push_back(paths);
    emit(
        state,
        "transfer.failed",
        json!({"task_id":task_id,"error_code":error_code,"error":error}),
    );
    Ok(())
}

fn emit(state: &AppState, event_type: &str, data: serde_json::Value) {
    let event = state.events.push(event_type, data);
    let _ = state.event_tx.send(event);
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

fn header(headers: &HeaderMap, name: &str) -> Result<String, ApiError> {
    Ok(headers
        .get(name)
        .ok_or_else(|| ApiError::status(StatusCode::BAD_REQUEST, anyhow!("missing header {name}")))?
        .to_str()
        .map_err(|err| ApiError::status(StatusCode::BAD_REQUEST, anyhow!(err.to_string())))?
        .to_string())
}

async fn verify_peer_auth(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let expected = state.config.read().await.shared_token.clone();
    let Some(expected) = expected else {
        return Ok(());
    };
    let got = headers
        .get("x-wormhole-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if got == expected {
        Ok(())
    } else {
        Err(ApiError::status(
            StatusCode::UNAUTHORIZED,
            anyhow!("invalid peer token"),
        ))
    }
}

fn url_escape(value: &str) -> String {
    value
        .bytes()
        .flat_map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => vec![b as char],
            _ => format!("%{b:02X}").chars().collect(),
        })
        .collect()
}

fn peer_get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T> {
    Ok(ureq::get(url)
        .timeout(Duration::from_secs(30))
        .call()?
        .into_json()?)
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

async fn touch_empty_file(
    State(state): State<AppState>,
    AxumPath(task_id): AxumPath<String>,
    Query(query): Query<UploadQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    verify_peer_auth(&state, &headers).await?;
    let config = state.config.read().await.clone();
    let final_path = receive_final_path(&config, &query.path)?;
    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::File::create(&final_path).await?;
    let state_for_update = state.clone();
    let task_for_update = task_id.clone();
    let path_for_update = query.path.clone();
    let final_path_for_update = final_path.clone();
    tokio::spawn(async move {
        let _ = record_chunk_received(
            state_for_update,
            task_for_update,
            path_for_update,
            0,
            true,
            final_path_for_update,
        )
        .await;
    });
    Ok(Json(json!({"ok":true,"path":final_path,"received":0})))
}

fn tmp_path_for(final_path: &std::path::Path) -> PathBuf {
    final_path.with_extension(format!(
        "{}wormhole_tmp",
        final_path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| format!("{s}."))
            .unwrap_or_default()
    ))
}

fn receive_final_path(config: &AppConfig, relative_path: &str) -> Result<PathBuf> {
    let path = safe_join(&config.receive_dir, relative_path)?;
    Ok(match config.transfer.conflict_strategy {
        ConflictStrategy::Overwrite => path,
        ConflictStrategy::Skip => path,
        ConflictStrategy::Rename => unique_path(&path),
    })
}

fn unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
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
        if !candidate.exists() {
            return candidate;
        }
    }
    path.to_path_buf()
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
            anyhow!("sha256 mismatch"),
        ))
    }
}

fn classify_error(error: &anyhow::Error) -> String {
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

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    error: anyhow::Error,
}

impl ApiError {
    fn status(status: StatusCode, error: anyhow::Error) -> Self {
        Self { status, error }
    }
}

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>,
{
    fn from(value: E) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: value.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(json!({"ok":false,"error":self.error.to_string()})),
        )
            .into_response()
    }
}
const CHUNK_SIZE: usize = 256 * 1024;
