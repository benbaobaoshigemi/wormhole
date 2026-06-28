use anyhow::anyhow;
use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use std::net::{IpAddr, SocketAddr};

use crate::{error::ApiError, state::AppState};

pub async fn require_loopback(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(_state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if is_loopback(addr.ip()) {
        Ok(next.run(request).await)
    } else {
        Err(ApiError::status(
            StatusCode::FORBIDDEN,
            "local_api_forbidden",
            anyhow!("local api accepts loopback clients only"),
        ))
    }
}

pub async fn verify_peer_auth(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
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
            "unauthorized",
            anyhow!("invalid peer token"),
        ))
    }
}

pub fn is_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_loopback(),
        IpAddr::V6(ip) => ip.is_loopback(),
    }
}
