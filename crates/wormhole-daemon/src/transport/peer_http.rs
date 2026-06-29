#[cfg(target_os = "macos")]
use anyhow::Context;
use anyhow::{anyhow, Result};
use reqwest::blocking::{Client, Response};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::de::DeserializeOwned;
use std::error::Error;
use std::net::{IpAddr, SocketAddr, UdpSocket};
#[cfg(target_os = "macos")]
use std::process::Command;
use std::time::Duration;

pub fn get_json<T: DeserializeOwned>(
    url: &str,
    token: Option<&str>,
    timeout: Duration,
) -> Result<T> {
    let response = request_with_auth(client(url, timeout)?.get(url), token)
        .send()
        .map_err(|err| request_error(url, err));
    match response {
        Ok(response) => response_json(url, response),
        Err(err) => curl_get_json(url, token, timeout).or(Err(err)),
    }
}

pub fn post_json<T: DeserializeOwned>(
    url: &str,
    body: &impl serde::Serialize,
    token: Option<&str>,
    timeout: Duration,
) -> Result<T> {
    let response = request_with_auth(client(url, timeout)?.post(url).json(body), token)
        .send()
        .map_err(|err| request_error(url, err));
    match response {
        Ok(response) => response_json(url, response),
        Err(err) => {
            let bytes = serde_json::to_vec(body)?;
            curl_body_json(url, "application/json", &bytes, token, timeout).or(Err(err))
        }
    }
}

pub fn post_bytes<T: DeserializeOwned>(
    url: &str,
    bytes: &[u8],
    token: Option<&str>,
    timeout: Duration,
) -> Result<T> {
    let response = request_with_auth(
        client(url, timeout)?
            .post(url)
            .headers(octet_stream_headers())
            .body(bytes.to_vec()),
        token,
    )
    .send()
    .map_err(|err| request_error(url, err));
    match response {
        Ok(response) => response_json(url, response),
        Err(err) => {
            curl_body_json(url, "application/octet-stream", bytes, token, timeout).or(Err(err))
        }
    }
}

pub fn post_empty_with_body(
    url: &str,
    bytes: &[u8],
    token: Option<&str>,
    timeout: Duration,
) -> Result<()> {
    let response = request_with_auth(
        client(url, timeout)?
            .post(url)
            .headers(octet_stream_headers())
            .body(bytes.to_vec()),
        token,
    )
    .send()
    .map_err(|err| request_error(url, err));
    match response {
        Ok(response) => {
            ensure_success(url, response)?;
            Ok(())
        }
        Err(err) => {
            curl_body_empty(url, "application/octet-stream", bytes, token, timeout).or(Err(err))
        }
    }
}

pub fn post_empty(url: &str, token: Option<&str>, timeout: Duration) -> Result<()> {
    let response = request_with_auth(client(url, timeout)?.post(url), token)
        .send()
        .map_err(|err| request_error(url, err));
    match response {
        Ok(response) => {
            ensure_success(url, response)?;
            Ok(())
        }
        Err(err) => {
            curl_body_empty(url, "application/octet-stream", &[], token, timeout).or(Err(err))
        }
    }
}

fn client(url: &str, timeout: Duration) -> Result<Client> {
    let mut builder = Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(8))
        .timeout(timeout)
        .pool_max_idle_per_host(0);
    if let Some(local_addr) = local_addr_for_url(url) {
        builder = builder.local_address(local_addr);
    }
    Ok(builder.build()?)
}

fn local_addr_for_url(url: &str) -> Option<IpAddr> {
    let url = reqwest::Url::parse(url).ok()?;
    let host = url.host_str()?.parse::<IpAddr>().ok()?;
    if !host.is_ipv4() {
        return None;
    }
    let port = url.port_or_known_default().unwrap_or(80);
    let socket = UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect(SocketAddr::new(host, port)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(addr) if !addr.is_unspecified() => Some(IpAddr::V4(addr)),
        _ => None,
    }
}

fn request_with_auth(
    request: reqwest::blocking::RequestBuilder,
    token: Option<&str>,
) -> reqwest::blocking::RequestBuilder {
    if let Some(token) = token {
        request.header("x-wormhole-token", token)
    } else {
        request
    }
}

fn octet_stream_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    headers
}

fn response_json<T: DeserializeOwned>(url: &str, response: Response) -> Result<T> {
    let response = ensure_success(url, response)?;
    response.json().map_err(|err| anyhow!("{url}: {err}"))
}

fn request_error(url: &str, err: reqwest::Error) -> anyhow::Error {
    let mut message = format!("{url}: {err}");
    let mut source = err.source();
    while let Some(err) = source {
        message.push_str(&format!("; caused by: {err}"));
        source = err.source();
    }
    anyhow!(message)
}

#[cfg(target_os = "macos")]
fn curl_get_json<T: DeserializeOwned>(
    url: &str,
    token: Option<&str>,
    timeout: Duration,
) -> Result<T> {
    let body = curl_request(url, "GET", None, &[], token, timeout)?;
    serde_json::from_slice(&body).map_err(|err| anyhow!("{url}: {err}"))
}

#[cfg(not(target_os = "macos"))]
fn curl_get_json<T: DeserializeOwned>(
    url: &str,
    _token: Option<&str>,
    _timeout: Duration,
) -> Result<T> {
    Err(anyhow!("{url}: curl fallback is not available"))
}

#[cfg(target_os = "macos")]
fn curl_body_json<T: DeserializeOwned>(
    url: &str,
    content_type: &str,
    bytes: &[u8],
    token: Option<&str>,
    timeout: Duration,
) -> Result<T> {
    let body = curl_request(url, "POST", Some(content_type), bytes, token, timeout)?;
    serde_json::from_slice(&body).map_err(|err| anyhow!("{url}: {err}"))
}

#[cfg(not(target_os = "macos"))]
fn curl_body_json<T: DeserializeOwned>(
    url: &str,
    _content_type: &str,
    _bytes: &[u8],
    _token: Option<&str>,
    _timeout: Duration,
) -> Result<T> {
    Err(anyhow!("{url}: curl fallback is not available"))
}

#[cfg(target_os = "macos")]
fn curl_body_empty(
    url: &str,
    content_type: &str,
    bytes: &[u8],
    token: Option<&str>,
    timeout: Duration,
) -> Result<()> {
    let _ = curl_request(url, "POST", Some(content_type), bytes, token, timeout)?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn curl_body_empty(
    url: &str,
    _content_type: &str,
    _bytes: &[u8],
    _token: Option<&str>,
    _timeout: Duration,
) -> Result<()> {
    Err(anyhow!("{url}: curl fallback is not available"))
}

#[cfg(target_os = "macos")]
fn curl_request(
    url: &str,
    method: &str,
    content_type: Option<&str>,
    bytes: &[u8],
    token: Option<&str>,
    timeout: Duration,
) -> Result<Vec<u8>> {
    let mut body_path = None;
    if !bytes.is_empty() {
        let path = std::env::temp_dir().join(format!(
            "wormhole-curl-body-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::write(&path, bytes)?;
        body_path = Some(path);
    }

    let mut command = Command::new("/usr/bin/curl");
    command
        .arg("-sS")
        .arg("--fail-with-body")
        .arg("--connect-timeout")
        .arg("8")
        .arg("--max-time")
        .arg(timeout.as_secs().max(1).to_string())
        .arg("-X")
        .arg(method);
    if let Some(content_type) = content_type {
        command
            .arg("-H")
            .arg(format!("Content-Type: {content_type}"));
    }
    if let Some(token) = token {
        command.arg("-H").arg(format!("x-wormhole-token: {token}"));
    }
    if let Some(path) = &body_path {
        command
            .arg("--data-binary")
            .arg(format!("@{}", path.display()));
    } else if method == "POST" {
        command.arg("--data-binary").arg("");
    }
    let output = command.arg(url).output();
    if let Some(path) = body_path {
        let _ = std::fs::remove_file(path);
    }
    let output = output.with_context(|| format!("{url}: run curl fallback"))?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(anyhow!("{url}: curl fallback failed: {stderr}{stdout}"))
}

fn ensure_success(url: &str, response: Response) -> Result<Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let body = response.text().unwrap_or_default();
    Err(anyhow!("{url}: HTTP {status}: {body}"))
}
