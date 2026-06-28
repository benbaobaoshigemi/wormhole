use anyhow::{anyhow, Result};
use axum::{
    http::{header, Method},
    middleware,
    response::Html,
    routing::{get, post},
    Router,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::{broadcast, Mutex, RwLock, Semaphore};
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
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
    // 强制清除代理环境变量，避免局域网 P2P 流量被系统/VPN 代理劫持（ureq 会默认读取）
    for key in [
        "http_proxy", "https_proxy", "all_proxy",
        "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"
    ] {
        std::env::remove_var(key);
    }

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
        failed_task_ids: Arc::new(Mutex::new(HashSet::new())),
        cancelled: Arc::new(Mutex::new(HashSet::new())),
        receive_tasks: Arc::new(Mutex::new(HashMap::new())),
        prepared_images: Arc::new(Mutex::new(HashMap::new())),
        transfer_slots,
        remote_hashes: Arc::new(Mutex::new(VecDeque::new())),
        clipboard: Arc::new(Mutex::new(SystemClipboard::new()?)),
        incoming_traffic_received: Arc::new(RwLock::new(false)),
        last_handshake_error: Arc::new(RwLock::new(None)),
        last_transfer_error_code: Arc::new(RwLock::new(None)),
        last_transfer_error_message: Arc::new(RwLock::new(None)),
        firewall_status: Arc::new(RwLock::new("unknown".to_string())),
        network_profile: Arc::new(RwLock::new("unknown".to_string())),
    };
    service::transfer::restore_tasks_from_db(&state).await?;

    let app_state = state.clone();
    tokio::spawn(async move { service::clipboard::clipboard_loop(app_state).await });
    let connection_state = state.clone();
    tokio::spawn(async move { service::connection::connection_loop(connection_state).await });
    let firewall_state = state.clone();
    tokio::spawn(async move { service::connection::firewall_loop(firewall_state).await });
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

    let router = Router::new()
        .nest("/local", local_routes)
        .nest("/peer", peer_routes)
        .layer(cors_layer())
        .with_state(state);

    if let Some(web_dir) = control_center_dir() {
        tracing::info!("serving browser control center from {}", web_dir.display());
        router.fallback_service(
            ServeDir::new(&web_dir).fallback(ServeFile::new(web_dir.join("index.html"))),
        )
    } else {
        router.fallback(get(missing_control_center))
    }
}

fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _| {
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
        .allow_methods(AllowMethods::list([Method::GET, Method::POST]))
        .allow_headers(AllowHeaders::list([header::CONTENT_TYPE]))
}

async fn missing_control_center() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<html lang="zh-CN">
  <head><meta charset="utf-8"><title>Wormhole</title></head>
  <body style="font-family: system-ui; padding: 32px">
    <h1>Wormhole control center is not built</h1>
    <p>Run the product build script or set WORMHOLE_WEB_DIR to a built desktop-ui dist directory.</p>
  </body>
</html>"#,
    )
}

fn control_center_dir() -> Option<PathBuf> {
    let candidates = [
        std::env::var_os("WORMHOLE_WEB_DIR").map(PathBuf::from),
        std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|parent| parent.join("web"))),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop-ui/dist")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|path| is_built_control_center(path))
}

fn is_built_control_center(path: &Path) -> bool {
    path.is_dir() && path.join("index.html").is_file()
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
