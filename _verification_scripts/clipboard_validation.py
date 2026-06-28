#!/usr/bin/env python3
"""Clipboard-focused Windows <-> macOS technical validation."""

from __future__ import annotations

import argparse
import base64
import getpass
import json
import os
import shutil
import subprocess
import time
import uuid
from pathlib import Path

import paramiko

import wormhole_validation as wh


DEFAULT_LOCAL_ROOT = Path(__file__).resolve().parents[1] / "_verification_runtime_clipboard"
DEFAULT_REMOTE_ROOT = "/Users/benbaobaoshigemi/Desktop/hole"
DEFAULT_REMOTE_HOST = "Air.local"
DEFAULT_REMOTE_USER = "benbaobaoshigemi"
LOCAL_NODE_PORT = 54301
REMOTE_NODE_PORT = 54302


def ssh_connect(host: str, user: str, password: str) -> paramiko.SSHClient:
    client = paramiko.SSHClient()
    client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    client.connect(hostname=host, username=user, password=password, timeout=10, look_for_keys=False, allow_agent=False)
    return client


def run_ssh(client: paramiko.SSHClient, command: str, timeout: float = 30.0) -> tuple[int, str, str]:
    stdin, stdout, stderr = client.exec_command(command, timeout=timeout)
    code = stdout.channel.recv_exit_status()
    out = stdout.read().decode("utf-8", errors="replace")
    err = stderr.read().decode("utf-8", errors="replace")
    return code, out, err


def sftp_write_text(sftp: paramiko.SFTPClient, path: str, text: str) -> None:
    with sftp.file(path, "w") as f:
        f.write(text.encode("utf-8"))


def write_local_config(root: Path, remote_host: str) -> Path:
    node_root = root / "A"
    config = {
        "node": "A",
        "device_id": f"win-clipboard-a-{uuid.uuid5(uuid.NAMESPACE_DNS, 'wormhole-clipboard-A')}",
        "device_name": "Windows Clipboard A",
        "host": "0.0.0.0",
        "port": LOCAL_NODE_PORT,
        "peer_host": remote_host,
        "peer_port": REMOTE_NODE_PORT,
        "config_dir": str(node_root / "config"),
        "receive_dir": str(node_root / "received"),
        "log_dir": str(node_root / "logs"),
        "max_image_bytes": wh.MAX_IMAGE_BYTES,
    }
    for key in ("config_dir", "receive_dir", "log_dir"):
        Path(config[key]).mkdir(parents=True, exist_ok=True)
    path = node_root / "config" / "node.json"
    wh.write_json_file(path, config)
    return path


def remote_config(remote_root: str, local_host: str) -> dict:
    return {
        "node": "B",
        "device_id": f"mac-clipboard-b-{uuid.uuid5(uuid.NAMESPACE_DNS, 'wormhole-clipboard-B')}",
        "device_name": "macOS Clipboard B",
        "host": "0.0.0.0",
        "port": REMOTE_NODE_PORT,
        "peer_host": local_host,
        "peer_port": LOCAL_NODE_PORT,
        "config_dir": f"{remote_root}/clipboard_validation/B/config",
        "receive_dir": f"{remote_root}/clipboard_validation/B/received",
        "log_dir": f"{remote_root}/clipboard_validation/B/logs",
        "max_image_bytes": wh.MAX_IMAGE_BYTES,
    }


def deploy_remote(client: paramiko.SSHClient, remote_root: str, local_host: str) -> None:
    escaped_root = remote_root.replace("'", "'\\''")
    code, out, err = run_ssh(
        client,
        "set -e; "
        f"mkdir -p '{escaped_root}/clipboard_validation/B/config' "
        f"'{escaped_root}/clipboard_validation/B/received' "
        f"'{escaped_root}/clipboard_validation/B/logs' "
        f"'{escaped_root}/clipboard_validation/sample_data'; "
        "command -v python3",
    )
    if code != 0:
        raise RuntimeError(f"remote python3 not found: {out} {err}")
    sftp = client.open_sftp()
    try:
        sftp.put(str(Path(__file__).with_name("wormhole_validation.py")), f"{remote_root}/clipboard_validation/wormhole_validation.py")
        sftp_write_text(
            sftp,
            f"{remote_root}/clipboard_validation/B/config/node.json",
            json.dumps(remote_config(remote_root, local_host), ensure_ascii=False, indent=2),
        )
    finally:
        sftp.close()


def start_remote_node(client: paramiko.SSHClient, remote_root: str) -> None:
    command = (
        f"cd '{remote_root}/clipboard_validation' && "
        "pkill -f 'clipboard_validation/wormhole_validation.py serve --config B/config/node.json' >/dev/null 2>&1 || true; "
        "nohup python3 wormhole_validation.py serve --config B/config/node.json "
        "> B/logs/process.stdout.log 2>&1 & echo $!"
    )
    code, out, err = run_ssh(client, command)
    if code != 0:
        raise RuntimeError(f"failed to start remote node: {out} {err}")


def stop_remote_node(client: paramiko.SSHClient) -> None:
    run_ssh(client, "pkill -f 'wormhole_validation.py serve --config B/config/node.json' >/dev/null 2>&1 || true")


def start_local_node(config_path: Path) -> subprocess.Popen:
    raw = wh.read_json_file(config_path)
    log_dir = Path(raw["log_dir"])
    log_dir.mkdir(parents=True, exist_ok=True)
    stdout_file = (log_dir / "process.stdout.log").open("a", encoding="utf-8")
    return subprocess.Popen(
        [wh.PYTHON_EXE, str(Path(__file__).with_name("wormhole_validation.py")), "serve", "--config", str(config_path)],
        stdout=stdout_file,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
    )


def wait_http(host: str, port: int, timeout: float = 10.0) -> None:
    deadline = time.time() + timeout
    last = None
    while time.time() < deadline:
        try:
            status, _ = wh.request_json(host, port, "GET", "/api/state", timeout=1.0)
            if status == 200:
                return
        except Exception as exc:
            last = exc
        time.sleep(0.3)
    raise RuntimeError(f"node {host}:{port} did not answer: {last!r}")


def assert_ok(name: str, condition: bool, details: str = "") -> dict:
    if not condition:
        raise AssertionError(f"{name} failed {details}")
    return {"name": name, "ok": True, "details": details}


def make_png(path: Path) -> bytes:
    path.parent.mkdir(parents=True, exist_ok=True)
    data = base64.b64decode(
        "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFElEQVR42mNk+M9Qz0AEYBxVSFIB"
        "ADjDBAOpf7sOAAAAAElFTkSuQmCC"
    )
    path.write_bytes(data)
    return data


def remote_set_text(client: paramiko.SSHClient, text: str) -> None:
    encoded = base64.b64encode(text.encode("utf-8")).decode("ascii")
    code, out, err = run_ssh(client, f"printf %s '{encoded}' | base64 -d | LANG=en_US.UTF-8 LC_CTYPE=en_US.UTF-8 pbcopy", timeout=10)
    if code != 0:
        raise RuntimeError(out + err)


def remote_get_text(client: paramiko.SSHClient) -> str:
    code, out, err = run_ssh(client, "LANG=en_US.UTF-8 LC_CTYPE=en_US.UTF-8 pbpaste", timeout=10)
    if code != 0:
        raise RuntimeError(out + err)
    return out


def remote_set_png(client: paramiko.SSHClient, remote_root: str, png_bytes: bytes) -> None:
    remote_png = f"{remote_root}/clipboard_validation/sample_data/source.png"
    sftp = client.open_sftp()
    try:
        with sftp.file(remote_png, "wb") as f:
            f.write(png_bytes)
    finally:
        sftp.close()
    script = (
        'use framework "Foundation"\n'
        'use framework "AppKit"\n'
        'use scripting additions\n'
        f'set imagePath to "{remote_png}"\n'
        'set imageData to current application\'s NSData\'s dataWithContentsOfFile:imagePath\n'
        'set pasteboard to current application\'s NSPasteboard\'s generalPasteboard()\n'
        'pasteboard\'s clearContents()\n'
        'pasteboard\'s setData:imageData forType:(current application\'s NSPasteboardTypePNG)\n'
    )
    encoded = base64.b64encode(script.encode("utf-8")).decode("ascii")
    code, out, err = run_ssh(client, f"printf %s '{encoded}' | base64 -d | osascript", timeout=20)
    if code != 0:
        raise RuntimeError(out + err)


def run_clipboard_validation(args: argparse.Namespace) -> int:
    password = os.environ.get("WORMHOLE_MAC_PASSWORD")
    if not password:
        password = getpass.getpass("macOS SSH password: ")

    root = args.local_root
    if root.exists():
        shutil.rmtree(root)
    png_bytes = make_png(root / "sample_data" / "source.png")
    config_path = write_local_config(root, args.remote_host)

    local_proc = None
    client = None
    original_windows_text = None
    results: list[dict] = []
    try:
        original_windows_text = wh.platform_get_clipboard_text()
    except Exception:
        original_windows_text = None

    try:
        client = ssh_connect(args.remote_host, args.remote_user, password)
        deploy_remote(client, args.remote_root, args.local_host)
        start_remote_node(client, args.remote_root)
        local_proc = start_local_node(config_path)
        wait_http("127.0.0.1", LOCAL_NODE_PORT)
        wait_http(args.remote_host, REMOTE_NODE_PORT)

        status, data = wh.request_json("127.0.0.1", LOCAL_NODE_PORT, "POST", "/api/connect", {}, timeout=10.0)
        results.append(assert_ok("Windows clipboard node connects macOS", status == 200 and data["ok"], json.dumps(data, ensure_ascii=False)))
        code, out, err = run_ssh(
            client,
            f"cd '{args.remote_root}/clipboard_validation' && python3 wormhole_validation.py --root '{args.remote_root}/clipboard_validation' connect --node B",
            timeout=15,
        )
        results.append(assert_ok("macOS clipboard node connects Windows", code == 0 and '"ok": true' in out, out + err))

        win_text = "Windows真实文本剪贴板 -> macOS真实剪贴板"
        wh.platform_set_clipboard_text(win_text)
        status, data = wh.request_json("127.0.0.1", LOCAL_NODE_PORT, "POST", "/api/clipboard/system/text/read-send", {}, timeout=15.0)
        mac_text = remote_get_text(client)
        results.append(assert_ok("Windows real text to macOS real clipboard", status == 200 and data["ok"] and mac_text == win_text, json.dumps({"response": data, "mac_text": mac_text}, ensure_ascii=False)))

        status, data = wh.request_json(args.remote_host, REMOTE_NODE_PORT, "POST", "/api/clipboard/system/text/read-send", {}, timeout=15.0)
        results.append(assert_ok("macOS text loop ignored after remote write", status == 200 and data.get("ignored") is True, json.dumps(data, ensure_ascii=False)))

        mac_src_text = "macOS真实文本剪贴板 -> Windows真实剪贴板"
        remote_set_text(client, mac_src_text)
        status, data = wh.request_json(args.remote_host, REMOTE_NODE_PORT, "POST", "/api/clipboard/system/text/read-send", {}, timeout=15.0)
        win_text_after = wh.platform_get_clipboard_text().rstrip("\r\n")
        results.append(assert_ok("macOS real text to Windows real clipboard", status == 200 and data["ok"] and win_text_after == mac_src_text, json.dumps({"response": data, "win_text": win_text_after}, ensure_ascii=False)))

        wh.platform_set_clipboard_png(png_bytes, root / "A" / "config")
        status, data = wh.request_json("127.0.0.1", LOCAL_NODE_PORT, "POST", "/api/clipboard/system/image/read-send", {}, timeout=30.0)
        results.append(assert_ok("Windows real image to macOS real clipboard", status == 200 and data["ok"] and int(data.get("size", 0)) > 0, json.dumps(data, ensure_ascii=False)))

        status, data = wh.request_json(args.remote_host, REMOTE_NODE_PORT, "POST", "/api/clipboard/system/image/read-send", {}, timeout=30.0)
        results.append(assert_ok("macOS image loop ignored after remote write", status == 200 and data.get("ignored") is True, json.dumps(data, ensure_ascii=False)))

        remote_set_png(client, args.remote_root, png_bytes)
        status, data = wh.request_json(args.remote_host, REMOTE_NODE_PORT, "POST", "/api/clipboard/system/image/read-send", {}, timeout=30.0)
        win_png_after = wh.platform_get_clipboard_png(root / "A" / "config")
        results.append(
            assert_ok(
                "macOS real image to Windows real clipboard",
                status == 200 and data["ok"] and win_png_after.startswith(b"\x89PNG\r\n\x1a\n") and len(win_png_after) > 0,
                json.dumps({"response": data, "win_png_hash": wh.sha256_bytes(win_png_after), "win_png_size": len(win_png_after)}, ensure_ascii=False),
            )
        )

        status, data = wh.request_json("127.0.0.1", LOCAL_NODE_PORT, "POST", "/api/debug/set-image-limit", {"bytes": 16}, timeout=10.0)
        results.append(assert_ok("lower image limit for validation", status == 200 and data["ok"], json.dumps(data, ensure_ascii=False)))
        wh.platform_set_clipboard_png(png_bytes, root / "A" / "config")
        status, data = wh.request_json("127.0.0.1", LOCAL_NODE_PORT, "POST", "/api/clipboard/system/image/read-send", {}, timeout=30.0)
        results.append(assert_ok("image size limit event", status == 200 and data.get("ignored") is True and data.get("reason") == "too_large", json.dumps(data, ensure_ascii=False)))

        status_win, data_win = wh.request_json("127.0.0.1", LOCAL_NODE_PORT, "GET", "/api/events?limit=300", timeout=10.0)
        status_mac, data_mac = wh.request_json(args.remote_host, REMOTE_NODE_PORT, "GET", "/api/events?limit=300", timeout=10.0)
        events = []
        if status_win == 200:
            events.extend(data_win.get("events", []))
        if status_mac == 200:
            events.extend(data_mac.get("events", []))
        event_types = {event["type"] for event in events}
        needed = {"clipboard.synced", "clipboard.ignored", "clipboard.too_large"}
        results.append(assert_ok("clipboard event coverage", needed.issubset(event_types), ",".join(sorted(event_types))))

        report = {"ok": True, "local_root": str(root), "remote_root": args.remote_root, "results": results}
        wh.write_json_file(root / "clipboard-validation-result.json", report)
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 0
    finally:
        if original_windows_text is not None:
            try:
                wh.platform_set_clipboard_text(original_windows_text)
            except Exception:
                pass
        if local_proc and local_proc.poll() is None:
            local_proc.terminate()
            try:
                local_proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                local_proc.kill()
        if client:
            try:
                stop_remote_node(client)
            finally:
                client.close()


def main() -> int:
    parser = argparse.ArgumentParser(description="Clipboard-focused validation")
    parser.add_argument("--remote-host", default=DEFAULT_REMOTE_HOST)
    parser.add_argument("--remote-user", default=DEFAULT_REMOTE_USER)
    parser.add_argument("--remote-root", default=DEFAULT_REMOTE_ROOT)
    parser.add_argument("--local-host", default="192.168.1.183")
    parser.add_argument("--local-root", type=Path, default=DEFAULT_LOCAL_ROOT)
    return run_clipboard_validation(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
