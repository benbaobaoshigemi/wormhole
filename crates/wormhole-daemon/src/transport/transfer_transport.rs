use anyhow::Result;
use serde::Deserialize;
use std::{
    fs::File as StdFile,
    io::{Read, Seek, SeekFrom},
    path::Path,
    thread,
    time::Duration,
};

#[derive(Debug, Deserialize)]
struct PeerUploadStatus {
    complete: bool,
    offset: u64,
}

pub fn upload_file_chunks(
    status_url: &str,
    base_url: &str,
    path: &Path,
    size: u64,
    sha256: Option<&str>,
    token: Option<&str>,
    chunk_size: usize,
    mut on_progress: impl FnMut(u64) -> Result<()>,
) -> Result<()> {
    let mut file = StdFile::open(path)?;
    let status_url = append_optional_param(status_url, "sha256", sha256);
    let base_url = append_optional_param(base_url, "sha256", sha256);
    let status: PeerUploadStatus = get_json_auth(&status_url, token)?;
    if status.complete {
        return Ok(());
    }
    let mut sent = status.offset.min(size);
    file.seek(SeekFrom::Start(sent))?;
    if size == 0 {
        touch_empty(&base_url, token)?;
        on_progress(0)?;
        return Ok(());
    }
    let mut buf = vec![0u8; chunk_size];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let offset = sent;
        sent += n as u64;
        post_chunk(&base_url, offset, sent >= size, &buf[..n], token)?;
        on_progress(n as u64)?;
    }
    Ok(())
}

fn get_json_auth<T: serde::de::DeserializeOwned>(url: &str, token: Option<&str>) -> Result<T> {
    retry_peer_io(|| {
        let mut request = ureq::get(url).timeout(Duration::from_secs(30));
        if let Some(token) = token {
            request = request.set("x-wormhole-token", token);
        }
        Ok(request.call()?.into_json()?)
    })
}

fn post_chunk(
    base_url: &str,
    offset: u64,
    final_chunk: bool,
    bytes: &[u8],
    token: Option<&str>,
) -> Result<()> {
    let sep = if base_url.contains('?') { "&" } else { "?" };
    let url = format!("{base_url}{sep}final_chunk={final_chunk}&offset={offset}");
    let timeout = if final_chunk {
        Duration::from_secs(600)
    } else {
        Duration::from_secs(45)
    };
    let mut request = ureq::post(&url)
        .timeout(timeout)
        .set("content-type", "application/octet-stream");
    if let Some(token) = token {
        request = request.set("x-wormhole-token", token);
    }
    request.send_bytes(bytes)?;
    Ok(())
}

fn touch_empty(base_url: &str, token: Option<&str>) -> Result<()> {
    let url = base_url.replace("/peer/transfer/upload-chunk/", "/peer/transfer/touch/");
    retry_peer_io(|| {
        let mut request = ureq::post(&url).timeout(Duration::from_secs(45));
        if let Some(token) = token {
            request = request.set("x-wormhole-token", token);
        }
        request.call()?;
        Ok(())
    })
}

fn retry_peer_io<T>(mut op: impl FnMut() -> Result<T>) -> Result<T> {
    let mut last_error = None;
    for attempt in 0..3 {
        match op() {
            Ok(value) => return Ok(value),
            Err(err) => {
                last_error = Some(err);
                if attempt < 2 {
                    thread::sleep(Duration::from_millis(150 * (attempt + 1) as u64));
                }
            }
        }
    }
    Err(last_error.expect("retry loop must record an error"))
}

fn append_optional_param(url: &str, key: &str, value: Option<&str>) -> String {
    let Some(value) = value else {
        return url.to_string();
    };
    let sep = if url.contains('?') { "&" } else { "?" };
    format!("{url}{sep}{key}={}", url_escape(value))
}

fn url_escape(value: &str) -> String {
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
