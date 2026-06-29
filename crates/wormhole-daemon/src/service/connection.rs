use anyhow::{anyhow, Result};
use axum::{http::StatusCode, Json};
use serde_json::json;
use std::time::Duration;
use wormhole_core::{ConnectionStatus, PublicDevice};

#[cfg(windows)]
use std::process::Command;

use crate::{error::ApiError, state::AppState, transport::peer_http};

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
                *state.last_handshake_error.write().await = Some(error.clone());
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
            *state.last_handshake_error.write().await = None;
            state.emit(
                "connection.changed",
                json!({"status":"connected","peer":peer}),
            );
            Ok(Json(json!({"ok":true,"peer":peer})))
        }
        Err(err) => {
            *state.status.write().await = ConnectionStatus::PeerOffline;
            *state.last_handshake_error.write().await = Some(err.to_string());
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
    peer_http::get_json(url, None, Duration::from_secs(30))
}

#[cfg(windows)]
pub fn query_firewall_status(daemon_path: &std::path::Path) -> (String, String) {
    let mut daemon_str = daemon_path.to_string_lossy().into_owned();
    if daemon_str.starts_with(r"\\?\") {
        daemon_str = daemon_str[4..].to_string();
    }
    let script = format!(
        r#"
        $daemonPath = "{}"
        $networkProfile = (Get-NetConnectionProfile -ErrorAction SilentlyContinue | Select-Object -ExpandProperty NetworkCategory -First 1)
        if (-not $networkProfile) {{ $networkProfile = "Unknown" }}

        $blockRules = Get-NetFirewallRule -AssociatedNetFirewallApplicationFilter (Get-NetFirewallApplicationFilter -ErrorAction SilentlyContinue | Where-Object {{ $_.Program -like "*wormhole-daemon.exe" }}) -ErrorAction SilentlyContinue | Where-Object {{ $_.Direction -eq "Inbound" -and $_.Action -eq "Block" -and $_.Enabled -eq "True" }}

        if ($blockRules) {{
            Write-Output "blocked_by_rule|$networkProfile"
            exit
        }}

        $allowRule = Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object {{ $_.DisplayName -like "*Wormhole*" -and $_.Direction -eq "Inbound" -and $_.Action -eq "Allow" -and $_.Enabled -eq "True" }} | Select-Object -First 1

        if (-not $allowRule) {{
            Write-Output "missing_rule|$networkProfile"
            exit
        }}

        $allowProgram = Get-NetFirewallApplicationFilter -AssociatedNetFirewallRule $allowRule -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Program -First 1
        if (-not $allowProgram) {{
            Write-Output "missing_rule|$networkProfile"
            exit
        }}

        $resolvedAllow = (Resolve-Path -LiteralPath $allowProgram -ErrorAction SilentlyContinue).Path
        if ($resolvedAllow) {{ $resolvedAllow = $resolvedAllow.Replace("\\?\", "") }}
        $resolvedDaemon = (Resolve-Path -LiteralPath $daemonPath -ErrorAction SilentlyContinue).Path
        if ($resolvedDaemon) {{ $resolvedDaemon = $resolvedDaemon.Replace("\\?\", "") }}

        if ($resolvedAllow -ne $resolvedDaemon) {{
            Write-Output "stale_program_path|$networkProfile"
            exit
        }}

        if ($networkProfile -eq "Public") {{
            Write-Output "public_network|$networkProfile"
            exit
        }}

        Write-Output "ok|$networkProfile"
        "#,
        daemon_str.replace('"', "\\\"")
    );

    let output = Command::new("powershell")
        .args(&["-NoProfile", "-Command", &script])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let mut parts = stdout.split('|');
            let status = parts.next().unwrap_or("unknown").to_string();
            let profile = parts.next().unwrap_or("unknown").to_string();
            (status, profile)
        }
        Err(_) => ("unknown".to_string(), "unknown".to_string()),
    }
}

#[cfg(not(windows))]
pub fn query_firewall_status(_daemon_path: &std::path::Path) -> (String, String) {
    ("ok".to_string(), "private".to_string())
}

pub async fn firewall_loop(state: AppState) {
    loop {
        let daemon_path = match std::env::current_exe() {
            Ok(p) => p,
            Err(_) => std::path::PathBuf::from("wormhole-daemon.exe"),
        };

        let (status, profile) =
            tokio::task::spawn_blocking(move || query_firewall_status(&daemon_path))
                .await
                .unwrap_or_else(|_| ("unknown".to_string(), "unknown".to_string()));

        *state.firewall_status.write().await = status;
        *state.network_profile.write().await = profile;

        tokio::time::sleep(Duration::from_secs(15)).await;
    }
}
