#!/usr/bin/env python3
"""Temporary technical validation harness for Wormhole Link.

This file is intentionally kept under _verification_scripts. It proves protocol
and platform assumptions before the real Rust Core Daemon is built.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import http.client
import json
import os
import shutil
import subprocess
import sys
import threading
import time
import urllib.parse
import uuid
from dataclasses import dataclass, field
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path, PurePosixPath
from typing import Any


PROTOCOL_VERSION = "validation-0.1"
PLATFORM = "windows" if os.name == "nt" else sys.platform
PYTHON_EXE = r"C:/Users/zhang/miniconda3/python.exe"
DEFAULT_ROOT = Path(__file__).resolve().parents[1] / "_verification_runtime"
CHUNK_SIZE = 1024 * 1024
MAX_IMAGE_BYTES = 20 * 1024 * 1024


def utc_ms() -> int:
    return int(time.time() * 1000)


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_text(text: str) -> str:
    return sha256_bytes(text.encode("utf-8"))


def json_dumps(data: Any) -> bytes:
    return json.dumps(data, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def safe_relative_path(raw_path: str) -> Path:
    posix = PurePosixPath(raw_path)
    if posix.is_absolute() or ".." in posix.parts:
        raise ValueError(f"unsafe relative path: {raw_path}")
    parts = [part for part in posix.parts if part not in ("", ".")]
    if not parts:
        raise ValueError("empty relative path")
    return Path(*parts)


def read_json_file(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as f:
        return json.load(f)


def write_json_file(path: Path, data: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=2)


def request_json(
    host: str,
    port: int,
    method: str,
    path: str,
    payload: dict[str, Any] | None = None,
    timeout: float = 5.0,
) -> tuple[int, dict[str, Any]]:
    body = json_dumps(payload) if payload is not None else None
    headers = {"Content-Type": "application/json; charset=utf-8"} if body else {}
    conn = http.client.HTTPConnection(host, port, timeout=timeout)
    try:
        conn.request(method, path, body=body, headers=headers)
        resp = conn.getresponse()
        raw = resp.read()
        if not raw:
            return resp.status, {}
        return resp.status, json.loads(raw.decode("utf-8"))
    finally:
        conn.close()


def request_bytes(
    host: str,
    port: int,
    method: str,
    path: str,
    data: bytes,
    headers: dict[str, str] | None = None,
    timeout: float = 10.0,
) -> tuple[int, dict[str, Any]]:
    conn = http.client.HTTPConnection(host, port, timeout=timeout)
    try:
        merged = {"Content-Length": str(len(data))}
        if headers:
            merged.update(headers)
        conn.request(method, path, body=data, headers=merged)
        resp = conn.getresponse()
        raw = resp.read()
        return resp.status, json.loads(raw.decode("utf-8")) if raw else {}
    finally:
        conn.close()


def stream_file_upload(
    host: str,
    port: int,
    path: str,
    source: Path,
    task_id: str,
    rel_path: str,
    event_fn,
    timeout: float = 30.0,
) -> tuple[int, dict[str, Any]]:
    size = source.stat().st_size
    conn = http.client.HTTPConnection(host, port, timeout=timeout)
    sent = 0
    try:
        conn.putrequest("PUT", path)
        conn.putheader("Content-Length", str(size))
        conn.putheader("Content-Type", "application/octet-stream")
        conn.endheaders()
        with source.open("rb") as f:
            while True:
                chunk = f.read(CHUNK_SIZE)
                if not chunk:
                    break
                conn.send(chunk)
                sent += len(chunk)
                event_fn(
                    "transfer.progress",
                    {
                        "task_id": task_id,
                        "relative_path": rel_path,
                        "bytes_done": sent,
                        "bytes_total": size,
                    },
                )
        resp = conn.getresponse()
        raw = resp.read()
        return resp.status, json.loads(raw.decode("utf-8")) if raw else {}
    finally:
        conn.close()


@dataclass
class NodeConfig:
    node: str
    device_id: str
    device_name: str
    host: str
    port: int
    peer_host: str
    peer_port: int
    config_dir: Path
    receive_dir: Path
    log_dir: Path
    max_image_bytes: int = MAX_IMAGE_BYTES

    @classmethod
    def from_file(cls, path: Path) -> "NodeConfig":
        raw = read_json_file(path)
        return cls(
            node=raw["node"],
            device_id=raw["device_id"],
            device_name=raw["device_name"],
            host=raw.get("host", "127.0.0.1"),
            port=int(raw["port"]),
            peer_host=raw.get("peer_host", "127.0.0.1"),
            peer_port=int(raw["peer_port"]),
            config_dir=Path(raw["config_dir"]),
            receive_dir=Path(raw["receive_dir"]),
            log_dir=Path(raw["log_dir"]),
            max_image_bytes=int(raw.get("max_image_bytes", MAX_IMAGE_BYTES)),
        )

    def as_public(self) -> dict[str, Any]:
        return {
            "device_id": self.device_id,
            "device_name": self.device_name,
            "platform": PLATFORM,
            "protocol_version": PROTOCOL_VERSION,
            "host": self.host,
            "port": self.port,
        }


@dataclass
class NodeState:
    config: NodeConfig
    peer: dict[str, Any] | None = None
    connection_status: str = "not_configured"
    last_error: str | None = None
    events: list[dict[str, Any]] = field(default_factory=list)
    tasks: dict[str, dict[str, Any]] = field(default_factory=dict)
    last_failed_task: dict[str, Any] | None = None
    memory_clipboard: dict[str, Any] | None = None
    recent_remote_hashes: dict[str, int] = field(default_factory=dict)
    fail_next_upload_after_bytes: int | None = None
    lock: threading.Lock = field(default_factory=threading.Lock)

    def emit(self, event_type: str, data: dict[str, Any]) -> None:
        event = {"ts": utc_ms(), "node": self.config.node, "type": event_type, "data": data}
        with self.lock:
            self.events.append(event)
            self.events = self.events[-500:]
        line = json.dumps(event, ensure_ascii=False)
        self.config.log_dir.mkdir(parents=True, exist_ok=True)
        with (self.config.log_dir / "events.jsonl").open("a", encoding="utf-8") as f:
            f.write(line + "\n")
        print(line, flush=True)

    def set_connection(self, status: str, peer: dict[str, Any] | None = None, error: str | None = None) -> None:
        with self.lock:
            self.connection_status = status
            self.peer = peer
            self.last_error = error
        self.emit("connection.changed", {"status": status, "peer": peer, "error": error})


def make_handler(state: NodeState):
    class Handler(BaseHTTPRequestHandler):
        server_version = "WormholeValidation/0.1"

        def log_message(self, fmt: str, *args: Any) -> None:
            state.config.log_dir.mkdir(parents=True, exist_ok=True)
            with (state.config.log_dir / "http.log").open("a", encoding="utf-8") as f:
                f.write(f"{utc_ms()} {self.address_string()} {fmt % args}\n")

        def send_json(self, code: int, data: dict[str, Any]) -> None:
            body = json_dumps(data)
            self.send_response(code)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def read_json(self) -> dict[str, Any]:
            length = int(self.headers.get("Content-Length", "0"))
            raw = self.rfile.read(length) if length else b"{}"
            return json.loads(raw.decode("utf-8"))

        def do_GET(self) -> None:
            parsed = urllib.parse.urlparse(self.path)
            if parsed.path == "/api/handshake":
                self.send_json(200, state.config.as_public())
                return
            if parsed.path == "/api/state":
                with state.lock:
                    payload = {
                        "self": state.config.as_public(),
                        "peer": state.peer,
                        "connection_status": state.connection_status,
                        "last_error": state.last_error,
                        "tasks": list(state.tasks.values()),
                    }
                self.send_json(200, payload)
                return
            if parsed.path == "/api/events":
                qs = urllib.parse.parse_qs(parsed.query)
                limit = int(qs.get("limit", ["100"])[0])
                with state.lock:
                    events = state.events[-limit:]
                self.send_json(200, {"events": events})
                return
            self.send_json(404, {"error": "not_found"})

        def do_POST(self) -> None:
            parsed = urllib.parse.urlparse(self.path)
            try:
                if parsed.path == "/api/connect":
                    self.handle_connect()
                    return
                if parsed.path == "/api/transfer/prepare":
                    self.handle_prepare()
                    return
                if parsed.path == "/api/transfer/send":
                    self.handle_send_transfer()
                    return
                if parsed.path == "/api/transfer/retry":
                    self.handle_retry()
                    return
                if parsed.path == "/api/clipboard/text/set":
                    self.handle_clipboard_text_set()
                    return
                if parsed.path == "/api/clipboard/text/receive":
                    self.handle_clipboard_text_receive()
                    return
                if parsed.path == "/api/clipboard/image/send":
                    self.handle_clipboard_image_send()
                    return
                if parsed.path == "/api/clipboard/image/receive":
                    self.handle_clipboard_image_receive()
                    return
                if parsed.path == "/api/clipboard/windows/read-send-text":
                    self.handle_windows_read_send_text()
                    return
                if parsed.path == "/api/clipboard/windows/write-text":
                    self.handle_windows_write_text()
                    return
                if parsed.path == "/api/clipboard/system/text/read-send":
                    self.handle_system_text_read_send()
                    return
                if parsed.path == "/api/clipboard/system/text/receive":
                    self.handle_system_text_receive()
                    return
                if parsed.path == "/api/clipboard/system/image/read-send":
                    self.handle_system_image_read_send()
                    return
                if parsed.path == "/api/clipboard/system/image/receive":
                    self.handle_system_image_receive()
                    return
                if parsed.path == "/api/debug/fail-next-upload-after":
                    self.handle_debug_fail_next_upload_after()
                    return
                if parsed.path == "/api/debug/set-image-limit":
                    self.handle_debug_set_image_limit()
                    return
            except Exception as exc:
                state.emit("daemon.error", {"operation": parsed.path, "error": repr(exc)})
                self.send_json(500, {"ok": False, "error": repr(exc)})
                return
            self.send_json(404, {"error": "not_found"})

        def do_PUT(self) -> None:
            parsed = urllib.parse.urlparse(self.path)
            try:
                if parsed.path == "/api/transfer/upload":
                    self.handle_upload(parsed)
                    return
            except Exception as exc:
                state.emit("transfer.failed", {"error": repr(exc), "path": self.path})
                self.send_json(500, {"ok": False, "error": repr(exc)})
                return
            self.send_json(404, {"error": "not_found"})

        def handle_connect(self) -> None:
            state.set_connection("connecting")
            try:
                status, peer = request_json(state.config.peer_host, state.config.peer_port, "GET", "/api/handshake")
                if status != 200:
                    raise RuntimeError(f"handshake_status_{status}")
                state.set_connection("connected", peer)
                self.send_json(200, {"ok": True, "peer": peer})
            except Exception as exc:
                state.set_connection("connection_failed", error=repr(exc))
                self.send_json(503, {"ok": False, "error": repr(exc)})

        def handle_prepare(self) -> None:
            payload = self.read_json()
            task_id = payload["task_id"]
            manifest = payload["manifest"]
            if not state.config.receive_dir.exists() or not state.config.receive_dir.is_dir():
                state.emit("transfer.failed", {"task_id": task_id, "reason": "receive_dir_unavailable"})
                self.send_json(500, {"ok": False, "error": "receive_dir_unavailable"})
                return
            task = {
                "task_id": task_id,
                "direction": "receive",
                "status": "prepared",
                "root_name": manifest["root_name"],
                "item_count": len(manifest["files"]),
                "total_size": manifest["total_size"],
                "transferred_size": 0,
            }
            with state.lock:
                state.tasks[task_id] = task
            state.emit("transfer.created", task)
            self.send_json(200, {"ok": True, "task_id": task_id})

        def handle_upload(self, parsed: urllib.parse.ParseResult) -> None:
            qs = urllib.parse.parse_qs(parsed.query)
            task_id = qs["task_id"][0]
            rel_raw = qs["path"][0]
            rel = safe_relative_path(rel_raw)
            length = int(self.headers.get("Content-Length", "0"))
            final_path = state.config.receive_dir / rel
            final_path.parent.mkdir(parents=True, exist_ok=True)
            tmp_path = final_path.with_name(final_path.name + ".wormhole_tmp")
            done = 0
            state.emit("transfer.started", {"task_id": task_id, "relative_path": rel_raw, "bytes_total": length})
            with tmp_path.open("wb") as f:
                remaining = length
                while remaining:
                    chunk = self.rfile.read(min(CHUNK_SIZE, remaining))
                    if not chunk:
                        raise RuntimeError("stream_interrupted")
                    f.write(chunk)
                    remaining -= len(chunk)
                    done += len(chunk)
                    state.emit(
                        "transfer.progress",
                        {
                            "task_id": task_id,
                            "relative_path": rel_raw,
                            "bytes_done": done,
                            "bytes_total": length,
                        },
                    )
                    fail_after = state.fail_next_upload_after_bytes
                    if fail_after is not None and done >= fail_after:
                        state.fail_next_upload_after_bytes = None
                        raise RuntimeError(f"injected_transfer_interruption_after_{done}_bytes")
            if final_path.exists():
                final_path.unlink()
            tmp_path.replace(final_path)
            with state.lock:
                task = state.tasks.get(task_id)
                if task:
                    task["transferred_size"] = int(task.get("transferred_size", 0)) + done
            self.send_json(200, {"ok": True, "path": str(final_path), "bytes": done})

        def handle_send_transfer(self) -> None:
            payload = self.read_json()
            paths = [Path(p) for p in payload["paths"]]
            result = send_paths(state, paths)
            code = 200 if result.get("ok") else 500
            self.send_json(code, result)

        def handle_retry(self) -> None:
            with state.lock:
                failed = state.last_failed_task
            if not failed:
                self.send_json(404, {"ok": False, "error": "no_failed_task"})
                return
            state.emit("transfer.retrying", {"previous_task_id": failed["task_id"]})
            result = send_paths(state, [Path(p) for p in failed["paths"]])
            self.send_json(200 if result.get("ok") else 500, result)

        def handle_clipboard_text_set(self) -> None:
            payload = self.read_json()
            text = payload["text"]
            content_hash = sha256_text(text)
            ignored_loop = False
            with state.lock:
                if state.recent_remote_hashes.get(content_hash):
                    ignored_loop = True
                else:
                    state.memory_clipboard = {"kind": "text", "hash": content_hash, "source_device_id": state.config.device_id}
            if ignored_loop:
                state.emit("clipboard.ignored", {"kind": "text", "hash": content_hash, "reason": "loop_prevented"})
                self.send_json(200, {"ok": True, "ignored": True, "hash": content_hash})
                return
            body = {"kind": "text", "text": text, "hash": content_hash, "source_device_id": state.config.device_id}
            try:
                status, resp = request_json(
                    state.config.peer_host,
                    state.config.peer_port,
                    "POST",
                    "/api/clipboard/text/receive",
                    body,
                )
                if status != 200:
                    raise RuntimeError(f"peer_status_{status}:{resp}")
                state.emit("clipboard.synced", {"kind": "text", "hash": content_hash, "target": "peer"})
                self.send_json(200, {"ok": True, "hash": content_hash, "peer": resp})
            except Exception as exc:
                state.emit("clipboard.failed", {"kind": "text", "hash": content_hash, "error": repr(exc)})
                self.send_json(503, {"ok": False, "error": repr(exc), "hash": content_hash})

        def handle_clipboard_text_receive(self) -> None:
            payload = self.read_json()
            content_hash = payload["hash"]
            source = payload["source_device_id"]
            if source == state.config.device_id:
                state.emit("clipboard.ignored", {"kind": "text", "hash": content_hash, "reason": "self_source"})
                self.send_json(200, {"ok": True, "ignored": True})
                return
            with state.lock:
                state.recent_remote_hashes[content_hash] = utc_ms()
                state.memory_clipboard = {"kind": "text", "hash": content_hash, "source_device_id": source}
            state.emit("clipboard.synced", {"kind": "text", "hash": content_hash, "source_device_id": source})
            self.send_json(200, {"ok": True, "hash": content_hash})

        def handle_clipboard_image_send(self) -> None:
            payload = self.read_json()
            png_path = Path(payload["path"])
            data = png_path.read_bytes()
            content_hash = sha256_bytes(data)
            if len(data) > state.config.max_image_bytes:
                state.emit(
                    "clipboard.too_large",
                    {
                        "kind": "image",
                        "hash": content_hash,
                        "size": len(data),
                        "limit": state.config.max_image_bytes,
                    },
                )
                self.send_json(200, {"ok": True, "ignored": True, "reason": "too_large", "hash": content_hash})
                return
            ignored_loop = False
            with state.lock:
                if state.recent_remote_hashes.get(content_hash):
                    ignored_loop = True
                else:
                    state.memory_clipboard = {"kind": "image", "hash": content_hash, "source_device_id": state.config.device_id}
            if ignored_loop:
                state.emit("clipboard.ignored", {"kind": "image", "hash": content_hash, "reason": "loop_prevented"})
                self.send_json(200, {"ok": True, "ignored": True, "hash": content_hash})
                return
            body = {
                "kind": "image",
                "mime": "image/png",
                "size": len(data),
                "hash": content_hash,
                "source_device_id": state.config.device_id,
                "png_base64": base64.b64encode(data).decode("ascii"),
            }
            status, resp = request_json(
                state.config.peer_host,
                state.config.peer_port,
                "POST",
                "/api/clipboard/image/receive",
                body,
                timeout=10.0,
            )
            if status != 200:
                state.emit("clipboard.failed", {"kind": "image", "hash": content_hash, "error": resp})
                self.send_json(503, {"ok": False, "error": resp, "hash": content_hash})
                return
            state.emit("clipboard.synced", {"kind": "image", "hash": content_hash, "target": "peer", "size": len(data)})
            self.send_json(200, {"ok": True, "hash": content_hash, "peer": resp})

        def handle_clipboard_image_receive(self) -> None:
            payload = self.read_json()
            data = base64.b64decode(payload["png_base64"])
            content_hash = sha256_bytes(data)
            if content_hash != payload["hash"]:
                raise RuntimeError("image_hash_mismatch")
            if len(data) > state.config.max_image_bytes:
                state.emit("clipboard.too_large", {"kind": "image", "hash": content_hash, "size": len(data)})
                self.send_json(200, {"ok": True, "ignored": True, "reason": "too_large"})
                return
            with state.lock:
                state.recent_remote_hashes[content_hash] = utc_ms()
                state.memory_clipboard = {
                    "kind": "image",
                    "hash": content_hash,
                    "source_device_id": payload["source_device_id"],
                    "size": len(data),
                }
            image_dir = state.config.config_dir / "received_clipboard_images"
            image_dir.mkdir(parents=True, exist_ok=True)
            out_path = image_dir / f"{content_hash}.png"
            out_path.write_bytes(data)
            state.emit("clipboard.synced", {"kind": "image", "hash": content_hash, "size": len(data)})
            self.send_json(200, {"ok": True, "hash": content_hash, "path": str(out_path)})

        def handle_windows_read_send_text(self) -> None:
            text = windows_get_clipboard_text()
            body = {"text": text}
            status, resp = request_json(state.config.host, state.config.port, "POST", "/api/clipboard/text/set", body)
            self.send_json(status, resp)

        def handle_windows_write_text(self) -> None:
            payload = self.read_json()
            windows_set_clipboard_text(payload["text"])
            content_hash = sha256_text(payload["text"])
            with state.lock:
                state.recent_remote_hashes[content_hash] = utc_ms()
                state.memory_clipboard = {"kind": "text", "hash": content_hash, "source_device_id": "windows-clipboard"}
            state.emit("clipboard.synced", {"kind": "text", "hash": content_hash, "target": "windows"})
            self.send_json(200, {"ok": True, "hash": content_hash})

        def handle_system_text_read_send(self) -> None:
            text = platform_get_clipboard_text()
            content_hash = sha256_text(text)
            with state.lock:
                if state.recent_remote_hashes.get(content_hash):
                    ignored = True
                else:
                    ignored = False
                    state.memory_clipboard = {"kind": "text", "hash": content_hash, "source_device_id": state.config.device_id}
            if ignored:
                state.emit("clipboard.ignored", {"kind": "text", "hash": content_hash, "reason": "loop_prevented"})
                self.send_json(200, {"ok": True, "ignored": True, "hash": content_hash})
                return
            body = {"kind": "text", "text": text, "hash": content_hash, "source_device_id": state.config.device_id}
            status, resp = request_json(
                state.config.peer_host,
                state.config.peer_port,
                "POST",
                "/api/clipboard/system/text/receive",
                body,
                timeout=10.0,
            )
            if status != 200:
                state.emit("clipboard.failed", {"kind": "text", "hash": content_hash, "error": resp})
                self.send_json(503, {"ok": False, "hash": content_hash, "error": resp})
                return
            state.emit("clipboard.synced", {"kind": "text", "hash": content_hash, "target": "peer_system"})
            self.send_json(200, {"ok": True, "hash": content_hash, "peer": resp})

        def handle_system_text_receive(self) -> None:
            payload = self.read_json()
            content_hash = payload["hash"]
            if content_hash != sha256_text(payload["text"]):
                raise RuntimeError("text_hash_mismatch")
            if payload["source_device_id"] == state.config.device_id:
                state.emit("clipboard.ignored", {"kind": "text", "hash": content_hash, "reason": "self_source"})
                self.send_json(200, {"ok": True, "ignored": True, "hash": content_hash})
                return
            platform_set_clipboard_text(payload["text"])
            with state.lock:
                state.recent_remote_hashes[content_hash] = utc_ms()
                state.memory_clipboard = {"kind": "text", "hash": content_hash, "source_device_id": payload["source_device_id"]}
            state.emit("clipboard.synced", {"kind": "text", "hash": content_hash, "target": "system_clipboard"})
            self.send_json(200, {"ok": True, "hash": content_hash})

        def handle_system_image_read_send(self) -> None:
            data = platform_get_clipboard_png(state.config.config_dir)
            content_hash = sha256_bytes(data)
            if len(data) > state.config.max_image_bytes:
                state.emit(
                    "clipboard.too_large",
                    {"kind": "image", "hash": content_hash, "size": len(data), "limit": state.config.max_image_bytes},
                )
                self.send_json(200, {"ok": True, "ignored": True, "reason": "too_large", "hash": content_hash, "size": len(data)})
                return
            with state.lock:
                if state.recent_remote_hashes.get(content_hash):
                    ignored = True
                else:
                    ignored = False
                    state.memory_clipboard = {
                        "kind": "image",
                        "hash": content_hash,
                        "source_device_id": state.config.device_id,
                        "size": len(data),
                    }
            if ignored:
                state.emit("clipboard.ignored", {"kind": "image", "hash": content_hash, "reason": "loop_prevented"})
                self.send_json(200, {"ok": True, "ignored": True, "hash": content_hash})
                return
            body = {
                "kind": "image",
                "mime": "image/png",
                "size": len(data),
                "hash": content_hash,
                "source_device_id": state.config.device_id,
                "png_base64": base64.b64encode(data).decode("ascii"),
            }
            status, resp = request_json(
                state.config.peer_host,
                state.config.peer_port,
                "POST",
                "/api/clipboard/system/image/receive",
                body,
                timeout=20.0,
            )
            if status != 200:
                state.emit("clipboard.failed", {"kind": "image", "hash": content_hash, "error": resp})
                self.send_json(503, {"ok": False, "hash": content_hash, "error": resp})
                return
            state.emit("clipboard.synced", {"kind": "image", "hash": content_hash, "target": "peer_system", "size": len(data)})
            self.send_json(200, {"ok": True, "hash": content_hash, "size": len(data), "peer": resp})

        def handle_system_image_receive(self) -> None:
            payload = self.read_json()
            data = base64.b64decode(payload["png_base64"])
            content_hash = sha256_bytes(data)
            if content_hash != payload["hash"]:
                raise RuntimeError("image_hash_mismatch")
            if len(data) > state.config.max_image_bytes:
                state.emit("clipboard.too_large", {"kind": "image", "hash": content_hash, "size": len(data)})
                self.send_json(200, {"ok": True, "ignored": True, "reason": "too_large", "hash": content_hash})
                return
            platform_set_clipboard_png(data, state.config.config_dir)
            with state.lock:
                state.recent_remote_hashes[content_hash] = utc_ms()
                state.memory_clipboard = {
                    "kind": "image",
                    "hash": content_hash,
                    "source_device_id": payload["source_device_id"],
                    "size": len(data),
                }
            state.emit("clipboard.synced", {"kind": "image", "hash": content_hash, "target": "system_clipboard", "size": len(data)})
            self.send_json(200, {"ok": True, "hash": content_hash, "size": len(data)})

        def handle_debug_fail_next_upload_after(self) -> None:
            payload = self.read_json()
            after = int(payload["bytes"])
            if after <= 0:
                raise ValueError("bytes must be positive")
            state.fail_next_upload_after_bytes = after
            state.emit("debug.fail_next_upload_after", {"bytes": after})
            self.send_json(200, {"ok": True, "bytes": after})

        def handle_debug_set_image_limit(self) -> None:
            payload = self.read_json()
            limit = int(payload["bytes"])
            if limit <= 0:
                raise ValueError("bytes must be positive")
            state.config.max_image_bytes = limit
            state.emit("debug.set_image_limit", {"bytes": limit})
            self.send_json(200, {"ok": True, "bytes": limit})

    return Handler


def scan_paths(paths: list[Path]) -> tuple[dict[str, Any], list[tuple[Path, str, int]]]:
    files: list[tuple[Path, str, int]] = []
    root_name = paths[0].name if len(paths) == 1 else "multi-item-transfer"
    for path in paths:
        if path.is_file():
            files.append((path, path.name, path.stat().st_size))
        elif path.is_dir():
            base_parent = path.parent
            for child in sorted(path.rglob("*")):
                if child.is_file():
                    rel = child.relative_to(base_parent).as_posix()
                    files.append((child, rel, child.stat().st_size))
        else:
            raise FileNotFoundError(str(path))
    total = sum(item[2] for item in files)
    manifest = {
        "root_name": root_name,
        "total_size": total,
        "files": [{"relative_path": rel, "size": size} for _, rel, size in files],
    }
    return manifest, files


def send_paths(state: NodeState, paths: list[Path]) -> dict[str, Any]:
    task_id = str(uuid.uuid4())
    manifest, files = scan_paths(paths)
    task = {
        "task_id": task_id,
        "direction": "send",
        "status": "created",
        "root_name": manifest["root_name"],
        "item_count": len(files),
        "total_size": manifest["total_size"],
        "transferred_size": 0,
        "paths": [str(p) for p in paths],
    }
    with state.lock:
        state.tasks[task_id] = task
    state.emit("transfer.created", {k: v for k, v in task.items() if k != "paths"})
    prepare = {"task_id": task_id, "sender": state.config.as_public(), "manifest": manifest}
    try:
        status, resp = request_json(
            state.config.peer_host,
            state.config.peer_port,
            "POST",
            "/api/transfer/prepare",
            prepare,
            timeout=5.0,
        )
        if status != 200:
            raise RuntimeError(f"prepare_failed_{status}:{resp}")
        task["status"] = "transferring"
        state.emit("transfer.started", {"task_id": task_id, "total_size": manifest["total_size"]})
        transferred = 0
        for source, rel, size in files:
            upload_path = "/api/transfer/upload?" + urllib.parse.urlencode({"task_id": task_id, "path": rel})
            status, resp = stream_file_upload(
                state.config.peer_host,
                state.config.peer_port,
                upload_path,
                source,
                task_id,
                rel,
                state.emit,
            )
            if status != 200:
                raise RuntimeError(f"upload_failed_{status}:{resp}")
            transferred += size
            task["transferred_size"] = transferred
        task["status"] = "completed"
        state.emit("transfer.completed", {"task_id": task_id, "bytes": transferred, "item_count": len(files)})
        return {"ok": True, "task_id": task_id, "manifest": manifest}
    except Exception as exc:
        task["status"] = "failed"
        task["error"] = repr(exc)
        with state.lock:
            state.last_failed_task = {"task_id": task_id, "paths": [str(p) for p in paths], "error": repr(exc)}
        state.emit("transfer.failed", {"task_id": task_id, "error": repr(exc)})
        return {"ok": False, "task_id": task_id, "error": repr(exc)}


def windows_get_clipboard_text() -> str:
    if os.name != "nt":
        raise RuntimeError("windows_clipboard_requires_windows")
    script = "$s=Get-Clipboard -Raw; [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($s))"
    result = subprocess.run(["powershell", "-NoProfile", "-Command", script], check=True, capture_output=True, text=True, encoding="ascii")
    return base64.b64decode(result.stdout.strip()).decode("utf-8")


def windows_set_clipboard_text(text: str) -> None:
    if os.name != "nt":
        raise RuntimeError("windows_clipboard_requires_windows")
    encoded = base64.b64encode(text.encode("utf-8")).decode("ascii")
    script = f"$s=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{encoded}')); Set-Clipboard -Value $s"
    subprocess.run(["powershell", "-NoProfile", "-Command", script], check=True, text=True, encoding="utf-8")


def platform_get_clipboard_text() -> str:
    if os.name == "nt":
        return windows_get_clipboard_text()
    if sys.platform == "darwin":
        env = os.environ.copy()
        env["LANG"] = "en_US.UTF-8"
        env["LC_CTYPE"] = "en_US.UTF-8"
        result = subprocess.run(["pbpaste"], check=True, capture_output=True, text=True, encoding="utf-8", env=env)
        return result.stdout
    raise RuntimeError(f"unsupported text clipboard platform: {sys.platform}")


def platform_set_clipboard_text(text: str) -> None:
    if os.name == "nt":
        windows_set_clipboard_text(text)
        return
    if sys.platform == "darwin":
        env = os.environ.copy()
        env["LANG"] = "en_US.UTF-8"
        env["LC_CTYPE"] = "en_US.UTF-8"
        subprocess.run(["pbcopy"], input=text, check=True, text=True, encoding="utf-8", env=env)
        return
    raise RuntimeError(f"unsupported text clipboard platform: {sys.platform}")


def powershell_quote(path: Path) -> str:
    return "'" + str(path).replace("'", "''") + "'"


def windows_set_clipboard_png(data: bytes, work_dir: Path) -> None:
    work_dir.mkdir(parents=True, exist_ok=True)
    png_path = work_dir / "system_clipboard_set.png"
    png_path.write_bytes(data)
    path_literal = powershell_quote(png_path)
    script = (
        "Add-Type -AssemblyName System.Windows.Forms; "
        "Add-Type -AssemblyName System.Drawing; "
        f"$img=[System.Drawing.Image]::FromFile({path_literal}); "
        "try { [System.Windows.Forms.Clipboard]::SetImage($img) } finally { $img.Dispose() }"
    )
    subprocess.run(["powershell", "-NoProfile", "-STA", "-Command", script], check=True, text=True, encoding="utf-8")


def windows_get_clipboard_png(work_dir: Path) -> bytes:
    work_dir.mkdir(parents=True, exist_ok=True)
    png_path = work_dir / "system_clipboard_get.png"
    path_literal = powershell_quote(png_path)
    script = (
        "Add-Type -AssemblyName System.Windows.Forms; "
        "Add-Type -AssemblyName System.Drawing; "
        "if (-not [System.Windows.Forms.Clipboard]::ContainsImage()) { throw 'clipboard_has_no_image' }; "
        "$img=[System.Windows.Forms.Clipboard]::GetImage(); "
        f"try {{ $img.Save({path_literal}, [System.Drawing.Imaging.ImageFormat]::Png) }} finally {{ $img.Dispose() }}"
    )
    subprocess.run(["powershell", "-NoProfile", "-STA", "-Command", script], check=True, text=True, encoding="utf-8")
    return png_path.read_bytes()


def macos_set_clipboard_png(data: bytes, work_dir: Path) -> None:
    work_dir.mkdir(parents=True, exist_ok=True)
    png_path = work_dir / "system_clipboard_set.png"
    png_path.write_bytes(data)
    script = (
        'use framework "Foundation"\n'
        'use framework "AppKit"\n'
        'use scripting additions\n'
        f'set imagePath to "{str(png_path).replace(chr(34), chr(92) + chr(34))}"\n'
        'set imageData to current application\'s NSData\'s dataWithContentsOfFile:imagePath\n'
        'set pasteboard to current application\'s NSPasteboard\'s generalPasteboard()\n'
        'pasteboard\'s clearContents()\n'
        'pasteboard\'s setData:imageData forType:(current application\'s NSPasteboardTypePNG)\n'
    )
    subprocess.run(["osascript", "-e", script], check=True, text=True, encoding="utf-8")


def macos_get_clipboard_png(work_dir: Path) -> bytes:
    work_dir.mkdir(parents=True, exist_ok=True)
    png_path = work_dir / "system_clipboard_get.png"
    script = (
        'set png_data to the clipboard as «class PNGf»\n'
        f'set the_file to open for access POSIX path of (POSIX file "{str(png_path)}") with write permission\n'
        'set eof of the_file to 0\n'
        'write png_data to the_file\n'
        'close access the_file\n'
    )
    subprocess.run(["osascript", "-e", script], check=True, text=True, encoding="utf-8")
    return png_path.read_bytes()


def platform_set_clipboard_png(data: bytes, work_dir: Path) -> None:
    if os.name == "nt":
        windows_set_clipboard_png(data, work_dir)
        return
    if sys.platform == "darwin":
        macos_set_clipboard_png(data, work_dir)
        return
    raise RuntimeError(f"unsupported image clipboard platform: {sys.platform}")


def platform_get_clipboard_png(work_dir: Path) -> bytes:
    if os.name == "nt":
        return windows_get_clipboard_png(work_dir)
    if sys.platform == "darwin":
        return macos_get_clipboard_png(work_dir)
    raise RuntimeError(f"unsupported image clipboard platform: {sys.platform}")


def node_config_path(root: Path, node: str) -> Path:
    return root / node / "config" / "node.json"


def write_default_configs(root: Path) -> dict[str, Path]:
    ports = {"A": 54101, "B": 54102}
    result: dict[str, Path] = {}
    for node, peer in (("A", "B"), ("B", "A")):
        node_root = root / node
        config = {
            "node": node,
            "device_id": f"node-{node.lower()}-{uuid.uuid5(uuid.NAMESPACE_DNS, 'wormhole-validation-' + node)}",
            "device_name": f"Node {node}",
            "host": "127.0.0.1",
            "port": ports[node],
            "peer_host": "127.0.0.1",
            "peer_port": ports[peer],
            "config_dir": str(node_root / "config"),
            "receive_dir": str(node_root / "received"),
            "log_dir": str(node_root / "logs"),
            "max_image_bytes": MAX_IMAGE_BYTES,
        }
        for key in ("config_dir", "receive_dir", "log_dir"):
            Path(config[key]).mkdir(parents=True, exist_ok=True)
        path = node_config_path(root, node)
        write_json_file(path, config)
        result[node] = path
    return result


def run_node(config_path: Path) -> None:
    config = NodeConfig.from_file(config_path)
    config.config_dir.mkdir(parents=True, exist_ok=True)
    config.receive_dir.mkdir(parents=True, exist_ok=True)
    config.log_dir.mkdir(parents=True, exist_ok=True)
    state = NodeState(config=config, connection_status="peer_offline")
    state.emit("daemon.started", {"self": config.as_public(), "receive_dir": str(config.receive_dir)})
    server = ThreadingHTTPServer((config.host, config.port), make_handler(state))
    try:
        server.serve_forever()
    finally:
        state.emit("daemon.stopped", {"self": config.as_public()})


def cli_request(args: argparse.Namespace, method: str, path: str, payload: dict[str, Any] | None = None) -> int:
    config = NodeConfig.from_file(node_config_path(args.root, args.node))
    status, data = request_json(config.host, config.port, method, path, payload)
    print(json.dumps({"status": status, "response": data}, ensure_ascii=False, indent=2))
    return 0 if 200 <= status < 300 else 1


def create_sample_data(root: Path) -> dict[str, Path]:
    sample = root / "sample_data"
    if sample.exists():
        shutil.rmtree(sample)
    sample.mkdir(parents=True, exist_ok=True)
    text = sample / "small.txt"
    text.write_text("hello wormhole\n中文内容\n", encoding="utf-8")
    binary = sample / "binary.bin"
    binary.write_bytes(bytes(range(256)) * 16)
    spaced = sample / "file with spaces.txt"
    spaced.write_text("spaces are preserved\n", encoding="utf-8")
    chinese = sample / "中文文件名.txt"
    chinese.write_text("文件名验证\n", encoding="utf-8")
    large = sample / "large-stream.bin"
    with large.open("wb") as f:
        block = hashlib.sha256(b"wormhole-large-block").digest() * 32768
        for _ in range(16):
            f.write(block)
    folder = sample / "folder-root"
    (folder / "层级一" / "层级二").mkdir(parents=True)
    (folder / "层级一" / "层级二" / "deep.txt").write_text("deep file\n", encoding="utf-8")
    (folder / "empty.txt").write_bytes(b"")
    (folder / "中文目录").mkdir()
    (folder / "中文目录" / "另一个文件.txt").write_text("folder transfer\n", encoding="utf-8")
    png = sample / "tiny.png"
    png.write_bytes(
        base64.b64decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGA"
            "WjR9awAAAABJRU5ErkJggg=="
        )
    )
    too_large_png = sample / "too-large.png"
    too_large_png.write_bytes(b"\x89PNG\r\n\x1a\n" + b"x" * (MAX_IMAGE_BYTES + 1))
    return {
        "text": text,
        "binary": binary,
        "spaced": spaced,
        "chinese": chinese,
        "large": large,
        "folder": folder,
        "png": png,
        "too_large_png": too_large_png,
    }


def wait_for_node(config: NodeConfig, timeout: float = 8.0) -> None:
    deadline = time.time() + timeout
    last: Exception | None = None
    while time.time() < deadline:
        try:
            status, _ = request_json(config.host, config.port, "GET", "/api/state", timeout=1.0)
            if status == 200:
                return
        except Exception as exc:
            last = exc
        time.sleep(0.2)
    raise RuntimeError(f"node {config.node} did not start: {last!r}")


def start_node_process(config_path: Path) -> subprocess.Popen:
    exe = PYTHON_EXE if Path(PYTHON_EXE).exists() else sys.executable
    raw = read_json_file(config_path)
    log_dir = Path(raw["log_dir"])
    log_dir.mkdir(parents=True, exist_ok=True)
    stdout_path = log_dir / "process.stdout.log"
    stdout_file = stdout_path.open("a", encoding="utf-8")
    return subprocess.Popen(
        [exe, str(Path(__file__).resolve()), "serve", "--config", str(config_path)],
        stdout=stdout_file,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
    )


def assert_ok(name: str, condition: bool, details: str = "") -> dict[str, Any]:
    if not condition:
        raise AssertionError(f"{name} failed {details}")
    return {"name": name, "ok": True, "details": details}


def run_all_validation(root: Path) -> int:
    if root.exists():
        shutil.rmtree(root)
    configs = write_default_configs(root)
    samples = create_sample_data(root)
    proc_a = start_node_process(configs["A"])
    proc_b = start_node_process(configs["B"])
    results: list[dict[str, Any]] = []
    original_clipboard_text: str | None = None
    try:
        if os.name == "nt":
            try:
                original_clipboard_text = windows_get_clipboard_text()
            except Exception:
                original_clipboard_text = None
        config_a = NodeConfig.from_file(configs["A"])
        config_b = NodeConfig.from_file(configs["B"])
        wait_for_node(config_a)
        wait_for_node(config_b)

        status, data = request_json(config_a.host, config_a.port, "POST", "/api/connect", {})
        results.append(assert_ok("A connects B", status == 200 and data["ok"], json.dumps(data, ensure_ascii=False)))
        status, data = request_json(config_b.host, config_b.port, "POST", "/api/connect", {})
        results.append(assert_ok("B connects A", status == 200 and data["ok"], json.dumps(data, ensure_ascii=False)))

        try:
            request_json("127.0.0.1", 54999, "GET", "/api/state", timeout=0.5)
            missing_port_failed = False
            missing_port_detail = "unexpected success"
        except Exception as exc:
            missing_port_failed = True
            missing_port_detail = repr(exc)
        results.append(assert_ok("port error fails", missing_port_failed, missing_port_detail))

        for key in ("text", "binary", "chinese", "spaced", "large"):
            status, data = request_json(
                config_a.host,
                config_a.port,
                "POST",
                "/api/transfer/send",
                {"paths": [str(samples[key])]},
                timeout=60.0,
            )
            results.append(assert_ok(f"send {key}", status == 200 and data["ok"], data.get("task_id", "")))

        status, data = request_json(
            config_a.host,
            config_a.port,
            "POST",
            "/api/transfer/send",
            {"paths": [str(samples["folder"])]},
            timeout=60.0,
        )
        results.append(assert_ok("send folder", status == 200 and data["ok"], data.get("task_id", "")))
        results.append(assert_ok("folder deep file exists", (config_b.receive_dir / "folder-root" / "层级一" / "层级二" / "deep.txt").exists()))
        results.append(assert_ok("folder empty file exists", (config_b.receive_dir / "folder-root" / "empty.txt").exists()))

        status, data = request_json(
            config_a.host,
            config_a.port,
            "POST",
            "/api/clipboard/text/set",
            {"text": "A to B text 中文"},
        )
        results.append(assert_ok("clipboard text A to B", status == 200 and data["ok"], data.get("hash", "")))
        status, data = request_json(
            config_b.host,
            config_b.port,
            "POST",
            "/api/clipboard/text/set",
            {"text": "B to A text 中文"},
        )
        results.append(assert_ok("clipboard text B to A", status == 200 and data["ok"], data.get("hash", "")))
        status, data = request_json(
            config_b.host,
            config_b.port,
            "POST",
            "/api/clipboard/text/set",
            {"text": "A to B text 中文"},
        )
        results.append(assert_ok("clipboard text loop ignored", status == 200 and data.get("ignored") is True, json.dumps(data, ensure_ascii=False)))

        if os.name == "nt":
            windows_set_clipboard_text("Windows真实剪贴板 -> 模拟对端")
            status, data = request_json(
                config_a.host,
                config_a.port,
                "POST",
                "/api/clipboard/windows/read-send-text",
                {},
            )
            results.append(assert_ok("windows clipboard to simulated peer", status == 200 and data["ok"], json.dumps(data, ensure_ascii=False)))
            status, data = request_json(
                config_b.host,
                config_b.port,
                "POST",
                "/api/clipboard/windows/write-text",
                {"text": "模拟对端 -> Windows真实剪贴板"},
            )
            current_clipboard = windows_get_clipboard_text()
            results.append(
                assert_ok(
                    "simulated peer to windows clipboard",
                    status == 200 and data["ok"] and current_clipboard.rstrip("\r\n") == "模拟对端 -> Windows真实剪贴板",
                    json.dumps({"response": data, "clipboard": current_clipboard}, ensure_ascii=False),
                )
            )

        status, data = request_json(
            config_a.host,
            config_a.port,
            "POST",
            "/api/clipboard/image/send",
            {"path": str(samples["png"])},
        )
        results.append(assert_ok("clipboard image png", status == 200 and data["ok"], data.get("hash", "")))
        status, data = request_json(
            config_a.host,
            config_a.port,
            "POST",
            "/api/clipboard/image/send",
            {"path": str(samples["too_large_png"])},
        )
        results.append(assert_ok("clipboard image size limit", status == 200 and data.get("ignored") is True, json.dumps(data, ensure_ascii=False)))

        status, data = request_json(
            config_b.host,
            config_b.port,
            "POST",
            "/api/debug/fail-next-upload-after",
            {"bytes": CHUNK_SIZE},
        )
        results.append(assert_ok("inject transfer interruption", status == 200 and data["ok"], json.dumps(data, ensure_ascii=False)))
        interrupted_final = config_b.receive_dir / "large-stream.bin"
        if interrupted_final.exists():
            interrupted_final.unlink()
        status, data = request_json(
            config_a.host,
            config_a.port,
            "POST",
            "/api/transfer/send",
            {"paths": [str(samples["large"])]},
            timeout=30.0,
        )
        results.append(assert_ok("transfer interruption fails", status == 500 and not data["ok"], json.dumps(data, ensure_ascii=False)))
        results.append(assert_ok("interrupted final file absent", not interrupted_final.exists(), str(interrupted_final)))
        status, data = request_json(config_a.host, config_a.port, "POST", "/api/transfer/retry", {}, timeout=60.0)
        results.append(assert_ok("retry after interruption", status == 200 and data["ok"], json.dumps(data, ensure_ascii=False)))

        proc_b.terminate()
        proc_b.wait(timeout=5)
        time.sleep(0.5)
        status, data = request_json(config_a.host, config_a.port, "POST", "/api/connect", {}, timeout=5.0)
        results.append(assert_ok("peer shutdown detected", status == 503 and not data["ok"], json.dumps(data, ensure_ascii=False)))
        status, data = request_json(
            config_a.host,
            config_a.port,
            "POST",
            "/api/transfer/send",
            {"paths": [str(samples["text"])]},
            timeout=5.0,
        )
        results.append(assert_ok("receive node down fails", status == 500 and not data["ok"], json.dumps(data, ensure_ascii=False)))
        proc_b = start_node_process(configs["B"])
        wait_for_node(config_b)
        status, data = request_json(config_a.host, config_a.port, "POST", "/api/connect", {}, timeout=5.0)
        results.append(assert_ok("peer restart reconnects", status == 200 and data["ok"], json.dumps(data, ensure_ascii=False)))
        status, data = request_json(config_a.host, config_a.port, "POST", "/api/transfer/retry", {}, timeout=30.0)
        results.append(assert_ok("retry after restart", status == 200 and data["ok"], json.dumps(data, ensure_ascii=False)))

        status, data = request_json(
            config_a.host,
            config_a.port,
            "POST",
            "/api/clipboard/text/set",
            {"text": "B to A text 中文"},
        )
        results.append(assert_ok("clipboard loop ignored after restart", status == 200 and data.get("ignored") is True, json.dumps(data, ensure_ascii=False)))

        unavailable = config_b.receive_dir
        backup = config_b.config_dir / "received-backup"
        if backup.exists():
            shutil.rmtree(backup)
        unavailable.rename(backup)
        unavailable.write_text("not a directory", encoding="utf-8")
        status, data = request_json(
            config_a.host,
            config_a.port,
            "POST",
            "/api/transfer/send",
            {"paths": [str(samples["text"])]},
            timeout=10.0,
        )
        results.append(assert_ok("receive dir unavailable fails", status == 500 and not data["ok"], json.dumps(data, ensure_ascii=False)))
        unavailable.unlink()
        backup.rename(unavailable)

        status_a, data_a = request_json(config_a.host, config_a.port, "GET", "/api/events?limit=300")
        status_b, data_b = request_json(config_b.host, config_b.port, "GET", "/api/events?limit=300")
        events = []
        if status_a == 200:
            events.extend(data_a["events"])
        if status_b == 200:
            events.extend(data_b["events"])
        event_types = {event["type"] for event in events}
        needed = {
            "connection.changed",
            "transfer.created",
            "transfer.started",
            "transfer.progress",
            "transfer.completed",
            "transfer.failed",
            "transfer.retrying",
            "clipboard.synced",
            "clipboard.ignored",
            "clipboard.too_large",
        }
        results.append(assert_ok("event coverage", needed.issubset(event_types), ",".join(sorted(event_types))))

        report = {"ok": True, "root": str(root), "results": results}
        write_json_file(root / "validation-result.json", report)
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 0
    finally:
        if original_clipboard_text is not None and os.name == "nt":
            try:
                windows_set_clipboard_text(original_clipboard_text)
            except Exception:
                pass
        for proc in (proc_a, proc_b):
            if proc and proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    proc.kill()


def main() -> int:
    parser = argparse.ArgumentParser(description="Wormhole Link technical validation harness")
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT)
    sub = parser.add_subparsers(dest="cmd", required=True)

    init = sub.add_parser("init", help="create Node A/B configs")
    init.set_defaults(func=lambda args: (write_default_configs(args.root), 0)[1])

    serve = sub.add_parser("serve", help="start one node")
    serve.add_argument("--config", type=Path, required=True)
    serve.set_defaults(func=lambda args: run_node(args.config) or 0)

    for name, method, path in (
        ("status", "GET", "/api/state"),
        ("connect", "POST", "/api/connect"),
        ("events", "GET", "/api/events?limit=100"),
        ("retry", "POST", "/api/transfer/retry"),
    ):
        p = sub.add_parser(name)
        p.add_argument("--node", choices=["A", "B"], required=True)
        p.set_defaults(func=lambda args, m=method, pth=path: cli_request(args, m, pth, {} if m == "POST" else None))

    send_file = sub.add_parser("send-file")
    send_file.add_argument("--node", choices=["A", "B"], required=True)
    send_file.add_argument("path", type=Path)
    send_file.set_defaults(func=lambda args: cli_request(args, "POST", "/api/transfer/send", {"paths": [str(args.path)]}))

    send_folder = sub.add_parser("send-folder")
    send_folder.add_argument("--node", choices=["A", "B"], required=True)
    send_folder.add_argument("path", type=Path)
    send_folder.set_defaults(func=lambda args: cli_request(args, "POST", "/api/transfer/send", {"paths": [str(args.path)]}))

    clip_text = sub.add_parser("clipboard-text")
    clip_text.add_argument("--node", choices=["A", "B"], required=True)
    clip_text.add_argument("text")
    clip_text.set_defaults(func=lambda args: cli_request(args, "POST", "/api/clipboard/text/set", {"text": args.text}))

    clip_image = sub.add_parser("clipboard-image")
    clip_image.add_argument("--node", choices=["A", "B"], required=True)
    clip_image.add_argument("png_path", type=Path)
    clip_image.set_defaults(func=lambda args: cli_request(args, "POST", "/api/clipboard/image/send", {"path": str(args.png_path)}))

    win_read = sub.add_parser("windows-clipboard-read-send-text")
    win_read.add_argument("--node", choices=["A", "B"], required=True)
    win_read.set_defaults(func=lambda args: cli_request(args, "POST", "/api/clipboard/windows/read-send-text", {}))

    win_write = sub.add_parser("windows-clipboard-write-text")
    win_write.add_argument("--node", choices=["A", "B"], required=True)
    win_write.add_argument("text")
    win_write.set_defaults(func=lambda args: cli_request(args, "POST", "/api/clipboard/windows/write-text", {"text": args.text}))

    samples = sub.add_parser("make-samples")
    samples.set_defaults(func=lambda args: (print(json.dumps({k: str(v) for k, v in create_sample_data(args.root).items()}, ensure_ascii=False, indent=2)), 0)[1])

    run_all = sub.add_parser("run-all")
    run_all.set_defaults(func=lambda args: run_all_validation(args.root))

    args = parser.parse_args()
    return int(args.func(args))


if __name__ == "__main__":
    raise SystemExit(main())
