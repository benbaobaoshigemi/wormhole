use anyhow::{anyhow, Result};
use axum::{http::StatusCode, Json};
use serde_json::json;
use std::time::Duration;
use wormhole_core::{ConnectionStatus, PublicDevice};

use crate::{error::ApiError, state::AppState};

pub async fn connect(state: &AppState) -> Result<Json<serde_json::Value>, ApiError> {
    *state.status.write().await = ConnectionStatus::Connecting;
    state.emit("connection.changed", json!({"status":"connecting"}));
    let config = state.config.read().await.clone();
    let base = config.peer_base_url();
    let handshake_url = format!("{base}/peer/handshake");
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
                state.emit(
                    "connection.changed",
                    json!({"status":"failed","error_code":"protocol","error":error}),
                );
                return Err(ApiError::status(
                    StatusCode::UPGRADE_REQUIRED,
                    "protocol",
                    anyhow!(error),
                ));
            }
            *state.status.write().await = ConnectionStatus::Connected;
            *state.peer.write().await = Some(peer.clone());
            state.emit(
                "connection.changed",
                json!({"status":"connected","peer":peer}),
            );
            Ok(Json(json!({"ok":true,"peer":peer})))
        }
        Err(err) => {
            *state.status.write().await = ConnectionStatus::PeerOffline;
            state.emit(
                "connection.changed",
                json!({"status":"peer_offline","error_code":"network","error":"peer unavailable"}),
            );
            Err(ApiError::status(
                StatusCode::SERVICE_UNAVAILABLE,
                "network",
                err.into(),
            ))
        }
    }
}

pub async fn connection_loop(state: AppState) {
    loop {
        let config = state.config.read().await.clone();
        let delay = if matches!(*state.status.read().await, ConnectionStatus::Connected) {
            config.connection.heartbeat_millis
        } else {
            config.connection.reconnect_millis
        };
        tokio::time::sleep(Duration::from_millis(delay.max(500))).await;
        if config.auto_connect {
            let _ = connect(&state).await;
        }
    }
}

fn peer_get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T> {
    Ok(ureq::get(url)
        .timeout(Duration::from_secs(30))
        .call()?
        .into_json()?)
}
