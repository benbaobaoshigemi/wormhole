use anyhow::Result;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

use crate::transport::peer_http;

#[derive(Debug, Deserialize)]
struct ImagePrepareResponse {
    accepted: bool,
    #[allow(dead_code)]
    reason: Option<String>,
    offset: Option<u64>,
    #[allow(dead_code)]
    max_image_bytes: u64,
}

pub enum ClipboardUploadOutcome {
    Uploaded,
    Ignored { reason: Option<String> },
}

pub fn post_png_chunks(
    base_url: &str,
    hash: &str,
    source_device_id: &str,
    png: &[u8],
    token: Option<&str>,
    chunk_size: usize,
) -> Result<ClipboardUploadOutcome> {
    let prepare_url = format!("{base_url}/prepare");
    let prepare: ImagePrepareResponse = post_json(
        &prepare_url,
        &json!({"hash":hash,"source_device_id":source_device_id,"size":png.len()}),
        token,
    )?;
    if !prepare.accepted {
        return Ok(ClipboardUploadOutcome::Ignored {
            reason: prepare.reason,
        });
    }

    let mut offset = prepare.offset.unwrap_or(0) as usize;
    while offset < png.len() {
        let end = (offset + chunk_size).min(png.len());
        let final_chunk = end >= png.len();
        let url = format!(
            "{base_url}/chunk?hash={}&source_device_id={}&final_chunk={}&offset={}",
            url_escape(hash),
            url_escape(source_device_id),
            final_chunk,
            offset
        );
        post_bytes(&url, &png[offset..end], token)?;
        offset = end;
    }

    if png.is_empty() {
        let url = format!(
            "{base_url}/chunk?hash={}&source_device_id={}&final_chunk=true&offset=0",
            url_escape(hash),
            url_escape(source_device_id)
        );
        post_bytes(&url, &[], token)?;
    }
    Ok(ClipboardUploadOutcome::Uploaded)
}

fn post_json<T: serde::de::DeserializeOwned>(
    url: &str,
    body: &impl serde::Serialize,
    token: Option<&str>,
) -> Result<T> {
    peer_http::post_json(url, body, token, Duration::from_secs(30))
}

fn post_bytes(url: &str, bytes: &[u8], token: Option<&str>) -> Result<()> {
    peer_http::post_empty_with_body(url, bytes, token, Duration::from_secs(60))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        },
        thread,
        time::Duration,
    };

    #[test]
    fn refused_prepare_does_not_upload_chunks() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test peer");
        listener
            .set_nonblocking(true)
            .expect("set listener nonblocking");
        let addr = listener.local_addr().expect("listener addr");
        let stop = Arc::new(AtomicBool::new(false));
        let chunks = Arc::new(AtomicUsize::new(0));
        let stop_for_thread = stop.clone();
        let chunks_for_thread = chunks.clone();
        let handle = thread::spawn(move || {
            while !stop_for_thread.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buf = [0u8; 2048];
                        let n = stream.read(&mut buf).unwrap_or(0);
                        let req = String::from_utf8_lossy(&buf[..n]);
                        let first = req.lines().next().unwrap_or_default();
                        let body = if first.contains("/prepare") {
                            r#"{"accepted":false,"reason":"too_large","offset":null,"max_image_bytes":1}"#
                        } else {
                            chunks_for_thread.fetch_add(1, Ordering::SeqCst);
                            r#"{"ok":true}"#
                        };
                        let response = format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(response.as_bytes());
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        let outcome = post_png_chunks(
            &format!("http://{addr}/peer/clipboard/image"),
            &"a".repeat(64),
            "source-device",
            b"png-bytes",
            None,
            4,
        )
        .expect("post png chunks");
        stop.store(true, Ordering::SeqCst);
        handle.join().expect("test peer thread");

        match outcome {
            ClipboardUploadOutcome::Ignored { reason } => {
                assert_eq!(reason.as_deref(), Some("too_large"));
            }
            ClipboardUploadOutcome::Uploaded => panic!("refused prepare must not upload"),
        }
        assert_eq!(chunks.load(Ordering::SeqCst), 0);
    }
}
