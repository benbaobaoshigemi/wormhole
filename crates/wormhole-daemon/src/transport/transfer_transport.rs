use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::{
    collections::VecDeque,
    fs::File as StdFile,
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
};

use crate::transport::peer_http;

#[derive(Debug, Deserialize)]
struct PeerUploadStatus {
    complete: bool,
    offset: u64,
    #[serde(default)]
    received_ranges: Vec<[u64; 2]>,
    #[serde(default)]
    parallel_upload: bool,
}

#[derive(Debug, Deserialize)]
struct PeerUploadChunkResponse {
    #[serde(default)]
    received: Option<u64>,
}

enum WorkerMessage {
    Progress(u64),
    Done(Result<()>),
}

pub fn upload_file_chunks(
    status_url: &str,
    base_url: &str,
    path: &Path,
    size: u64,
    sha256: Option<&str>,
    token: Option<&str>,
    chunk_size: usize,
    parallelism: usize,
    mut on_progress: impl FnMut(u64) -> Result<()>,
) -> Result<()> {
    let chunk_size = chunk_size.max(1);
    let status_url = append_optional_param(status_url, "sha256", sha256);
    let base_url = append_optional_param(base_url, "sha256", sha256);
    let status: PeerUploadStatus = get_json_auth(&status_url, token)?;
    if status.complete {
        return Ok(());
    }
    if size == 0 {
        touch_empty(&base_url, token)?;
        on_progress(0)?;
        return Ok(());
    }
    if !status.parallel_upload || parallelism <= 1 {
        return upload_file_chunks_sequential(
            &base_url,
            path,
            size,
            status.offset.min(size),
            token,
            chunk_size,
            on_progress,
        );
    }

    let mut ranges = status
        .received_ranges
        .into_iter()
        .filter_map(|range| valid_range(range[0], range[1], size))
        .collect::<Vec<_>>();
    if ranges.is_empty() && status.offset > 0 {
        ranges.push((0, status.offset.min(size)));
    }
    let jobs = missing_chunk_jobs(size, chunk_size as u64, &ranges);
    if jobs.is_empty() {
        return Ok(());
    }
    upload_file_chunks_parallel(&base_url, path, size, token, parallelism, jobs, on_progress)
}

fn upload_file_chunks_sequential(
    base_url: &str,
    path: &Path,
    size: u64,
    start_offset: u64,
    token: Option<&str>,
    chunk_size: usize,
    mut on_progress: impl FnMut(u64) -> Result<()>,
) -> Result<()> {
    let mut file = StdFile::open(path)?;
    let mut sent = start_offset.min(size);
    file.seek(SeekFrom::Start(sent))?;
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

fn upload_file_chunks_parallel(
    base_url: &str,
    path: &Path,
    size: u64,
    token: Option<&str>,
    parallelism: usize,
    jobs: Vec<(u64, u64)>,
    mut on_progress: impl FnMut(u64) -> Result<()>,
) -> Result<()> {
    let workers = parallelism.clamp(1, 16).min(jobs.len());
    let jobs = Arc::new(Mutex::new(VecDeque::from(jobs)));
    let stop = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel::<WorkerMessage>();
    let path = path.to_path_buf();
    let base_url = base_url.to_string();
    let token = token.map(str::to_string);

    for _ in 0..workers {
        let jobs = Arc::clone(&jobs);
        let stop = Arc::clone(&stop);
        let tx = tx.clone();
        let path = path.clone();
        let base_url = base_url.clone();
        let token = token.clone();
        thread::spawn(move || {
            let result = run_upload_worker(
                &base_url,
                &path,
                size,
                token.as_deref(),
                jobs,
                stop,
                tx.clone(),
            );
            let _ = tx.send(WorkerMessage::Done(result));
        });
    }
    drop(tx);

    let mut completed = 0usize;
    let mut first_error: Option<anyhow::Error> = None;
    while completed < workers {
        match rx.recv() {
            Ok(WorkerMessage::Progress(delta)) => on_progress(delta)?,
            Ok(WorkerMessage::Done(Ok(()))) => completed += 1,
            Ok(WorkerMessage::Done(Err(err))) => {
                if first_error.is_none() {
                    stop.store(true, Ordering::SeqCst);
                    first_error = Some(err);
                }
                completed += 1;
            }
            Err(_) => break,
        }
    }
    if let Some(err) = first_error {
        Err(err)
    } else {
        Ok(())
    }
}

fn run_upload_worker(
    base_url: &str,
    path: &Path,
    size: u64,
    token: Option<&str>,
    jobs: Arc<Mutex<VecDeque<(u64, u64)>>>,
    stop: Arc<AtomicBool>,
    progress: mpsc::Sender<WorkerMessage>,
) -> Result<()> {
    let mut file = StdFile::open(path)?;
    loop {
        if stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        let Some((offset, len)) = jobs.lock().expect("upload jobs poisoned").pop_front() else {
            return Ok(());
        };
        let mut buf = vec![0u8; len as usize];
        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(&mut buf)?;
        let response = post_chunk(base_url, offset, offset + len >= size, &buf, token)?;
        let delta = response.received.unwrap_or(len);
        progress
            .send(WorkerMessage::Progress(delta))
            .map_err(|_| anyhow!("upload progress receiver closed"))?;
    }
}

fn get_json_auth<T: serde::de::DeserializeOwned>(url: &str, token: Option<&str>) -> Result<T> {
    retry_peer_io(|| peer_http::get_json(url, token, Duration::from_secs(30)))
}

fn post_chunk(
    base_url: &str,
    offset: u64,
    final_chunk: bool,
    bytes: &[u8],
    token: Option<&str>,
) -> Result<PeerUploadChunkResponse> {
    let sep = if base_url.contains('?') { "&" } else { "?" };
    let url = format!("{base_url}{sep}final_chunk={final_chunk}&offset={offset}");
    let timeout = if final_chunk {
        Duration::from_secs(600)
    } else {
        Duration::from_secs(45)
    };
    peer_http::post_bytes(&url, bytes, token, timeout)
}

fn missing_chunk_jobs(
    size: u64,
    chunk_size: u64,
    received_ranges: &[(u64, u64)],
) -> Vec<(u64, u64)> {
    let mut sorted = received_ranges.to_vec();
    sorted.sort_by_key(|range| range.0);
    let mut jobs = Vec::new();
    let mut cursor = 0u64;
    for (start, end) in sorted {
        if start > cursor {
            push_chunk_jobs(&mut jobs, cursor, start.min(size), chunk_size);
        }
        cursor = cursor.max(end.min(size));
        if cursor >= size {
            break;
        }
    }
    if cursor < size {
        push_chunk_jobs(&mut jobs, cursor, size, chunk_size);
    }
    jobs
}

fn push_chunk_jobs(jobs: &mut Vec<(u64, u64)>, start: u64, end: u64, chunk_size: u64) {
    let chunk_size = chunk_size.max(1);
    let mut offset = start;
    while offset < end {
        let next = offset.saturating_add(chunk_size).min(end);
        jobs.push((offset, next - offset));
        offset = next;
    }
}

fn valid_range(start: u64, end: u64, size: u64) -> Option<(u64, u64)> {
    if start < end && end <= size {
        Some((start, end))
    } else {
        None
    }
}

fn touch_empty(base_url: &str, token: Option<&str>) -> Result<()> {
    let url = base_url.replace("/peer/transfer/upload-chunk/", "/peer/transfer/touch/");
    retry_peer_io(|| peer_http::post_empty(&url, token, Duration::from_secs(45)))
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

#[cfg(test)]
mod tests {
    use super::{missing_chunk_jobs, valid_range};

    #[test]
    fn missing_chunk_jobs_cover_gaps_in_order() {
        let jobs = missing_chunk_jobs(10, 3, &[(0, 2), (5, 7)]);
        assert_eq!(jobs, vec![(2, 3), (7, 3)]);
    }

    #[test]
    fn missing_chunk_jobs_handles_unsorted_overlapping_ranges() {
        let jobs = missing_chunk_jobs(12, 4, &[(8, 12), (0, 5), (3, 8)]);
        assert!(jobs.is_empty());
    }

    #[test]
    fn upload_ranges_reject_invalid_status_ranges() {
        assert_eq!(valid_range(0, 4, 10), Some((0, 4)));
        assert_eq!(valid_range(4, 4, 10), None);
        assert_eq!(valid_range(8, 14, 10), None);
    }
}
