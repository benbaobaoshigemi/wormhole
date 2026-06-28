use anyhow::{anyhow, Result};
use axum::{
    middleware,
    response::Html,
    routing::{get, post},
    Router,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::{broadcast, Mutex, RwLock, Semaphore};
use tower_http::cors::{AllowOrigin, CorsLayer};
use wormhole_core::{AppConfig, ConnectionStatus, EventLog, HistoryDb};
use wormhole_platform::SystemClipboard;

mod api;
mod auth;
mod dto;
mod error;
mod service;
mod state;
mod transport;

use state::AppState;

#[derive(Debug)]
struct ServeArgs {
    config: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("wormhole_daemon=info,tower_http=warn")
        .init();
    let args = parse_args()?;
    let mut config = AppConfig::load(&args.config)?;
    if config.bind_host == "0.0.0.0" {
        config.bind_host = "127.0.0.1".to_string();
    }
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
        failed_task_ids: Arc::new(Mutex::new(HashSet::new())),
        cancelled: Arc::new(Mutex::new(HashSet::new())),
        receive_tasks: Arc::new(Mutex::new(HashMap::new())),
        transfer_slots,
        remote_hashes: Arc::new(Mutex::new(VecDeque::new())),
        clipboard: Arc::new(Mutex::new(SystemClipboard::new()?)),
    };

    let app_state = state.clone();
    tokio::spawn(async move { service::clipboard::clipboard_loop(app_state).await });
    let connection_state = state.clone();
    tokio::spawn(async move { service::connection::connection_loop(connection_state).await });
    let history_state = state.clone();
    tokio::spawn(async move { service::history::history_prune_loop(history_state).await });

    let bind = {
        let config = state.config.read().await;
        format!("{}:{}", config.bind_host, config.port)
    };
    let router = build_router(state);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("wormhole daemon listening on {}", bind);
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

fn build_router(state: AppState) -> Router {
    let local_routes = Router::new()
        .route("/state", get(api::local::state))
        .route("/settings", get(api::local::get_settings))
        .route("/settings/update", post(api::local::update_settings))
        .route("/connect", post(api::local::connect))
        .route("/disconnect", post(api::local::disconnect))
        .route("/transfer/send", post(api::local::send_transfer))
        .route("/transfer/cancel", post(api::local::cancel_transfer))
        .route("/transfer/retry", post(api::local::retry_transfer))
        .route("/transfer/tasks", get(api::local::tasks))
        .route("/transfer/history", get(api::local::history))
        .route("/transfer/history/clear", post(api::local::clear_history))
        .route("/clipboard/status", get(api::local::clipboard_status))
        .route("/clipboard/enable", post(api::local::clipboard_enable))
        .route("/clipboard/disable", post(api::local::clipboard_disable))
        .route(
            "/clipboard/system/read-send-text",
            post(api::local::read_send_text),
        )
        .route(
            "/clipboard/system/read-send-image",
            post(api::local::read_send_image),
        )
        .route("/events", get(api::events::events))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_loopback,
        ));

    let peer_routes = Router::new()
        .route("/handshake", get(api::peer::handshake))
        .route("/transfer/prepare", post(api::peer::prepare_transfer))
        .route(
            "/transfer/upload-status/:task_id",
            get(api::peer::upload_status),
        )
        .route(
            "/transfer/upload-chunk/:task_id",
            post(api::peer::upload_chunk),
        )
        .route(
            "/transfer/touch/:task_id",
            post(api::peer::touch_empty_file),
        )
        .route("/clipboard/text/receive", post(api::peer::receive_text))
        .route(
            "/clipboard/image/prepare",
            post(api::peer::prepare_image_clipboard),
        )
        .route(
            "/clipboard/image/chunk",
            post(api::peer::receive_image_chunk),
        );

    Router::new()
        .route("/", get(index))
        .nest("/local", local_routes)
        .nest("/peer", peer_routes)
        .layer(cors_layer())
        .with_state(state)
}

fn cors_layer() -> CorsLayer {
    CorsLayer::new().allow_origin(AllowOrigin::predicate(|origin, _| {
        origin
            .to_str()
            .map(|origin| {
                origin == "http://127.0.0.1"
                    || origin.starts_with("http://127.0.0.1:")
                    || origin == "http://localhost"
                    || origin.starts_with("http://localhost:")
            })
            .unwrap_or(false)
    }))
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../../../apps/desktop-ui/index.html"))
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
