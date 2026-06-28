use anyhow::Result;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

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
    let mut request = ureq::post(url).timeout(Duration::from_secs(30));
    if let Some(token) = token {
        request = request.set("x-wormhole-token", token);
    }
    Ok(request
        .send_json(serde_json::to_value(body)?)?
        .into_json()?)
}

fn post_bytes(url: &str, bytes: &[u8], token: Option<&str>) -> Result<()> {
    let mut request = ureq::post(url)
        .timeout(Duration::from_secs(60))
        .set("content-type", "application/octet-stream");
    if let Some(token) = token {
        request = request.set("x-wormhole-token", token);
    }
    request.send_bytes(bytes)?;
    Ok(())
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
