#!/usr/bin/env python3
"""Windows <-> macOS technical validation harness.

This is a temporary bridge script. It deploys the validation node to a macOS
host over SSH and runs real LAN handshake, transfer, retry, and clipboard-model
checks before formal product development starts.
"""

from __future__ import annotations

import argparse
import getpass
import json
import os
import shutil
import subprocess
import sys
import time
import uuid
from pathlib import Path

import paramiko

import wormhole_validation as wh


DEFAULT_LOCAL_ROOT = Path(__file__).resolve().parents[1] / "_verification_runtime_macos"
DEFAULT_REMOTE_ROOT = "/Users/benbaobaoshigemi/Desktop/hole"
DEFAULT_REMOTE_HOST = "Air.local"
DEFAULT_REMOTE_USER = "benbaobaoshigemi"
LOCAL_NODE_PORT = 54201
REMOTE_NODE_PORT = 54202


def write_local_config(root: Path, local_host: str, remote_host: str) -> Path:
    node_root = root / "A"
    config = {
        "node": "A",
        "device_id": f"win-node-a-{uuid.uuid5(uuid.NAMESPACE_DNS, 'wormhole-macos-link-A')}",
        "device_name": "Windows Node A",
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
    path = root / "A" / "config" / "node.json"
    wh.write_json_file(path, config)
    return path


def remote_config(remote_root: str, local_host: str) -> dict:
    return {
        "node": "B",
        "device_id": f"mac-node-b-{uuid.uuid5(uuid.NAMESPACE_DNS, 'wormhole-macos-link-B')}",
        "device_name": "macOS Node B",
        "host": "0.0.0.0",
        "port": REMOTE_NODE_PORT,
        "peer_host": local_host,
        "peer_port": LOCAL_NODE_PORT,
        "config_dir": f"{remote_root}/B/config",
        "receive_dir": f"{remote_root}/B/received",
        "log_dir": f"{remote_root}/B/logs",
        "max_image_bytes": wh.MAX_IMAGE_BYTES,
    }


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


def deploy_remote(client: paramiko.SSHClient, remote_root: str, local_host: str) -> None:
    escaped_root = remote_root.replace("'", "'\\''")
    code, out, err = run_ssh(
        client,
        "set -e; "
        f"mkdir -p '{escaped_root}/B/config' '{escaped_root}/B/received' '{escaped_root}/B/logs' "
        f"'{escaped_root}/sample_data/mac folder'; "
        "command -v python3",
    )
    if code != 0:
        raise RuntimeError(f"remote python3 not found: {out} {err}")

    sftp = client.open_sftp()
    try:
        sftp.put(str(Path(__file__).with_name("wormhole_validation.py")), f"{remote_root}/wormhole_validation.py")
        sftp_write_text(
            sftp,
            f"{remote_root}/B/config/node.json",
            json.dumps(remote_config(remote_root, local_host), ensure_ascii=False, indent=2),
        )
        sftp_write_text(sftp, f"{remote_root}/sample_data/mac-small.txt", "hello from macOS\n中文联调\n")
        sftp_write_text(sftp, f"{remote_root}/sample_data/mac folder/placeholder.txt", "folder from macOS\n")
    finally:
        sftp.close()


def start_remote_node(client: paramiko.SSHClient, remote_root: str) -> None:
    command = (
        f"cd '{remote_root}' && "
        "pkill -f 'wormhole_validation.py serve --config B/config/node.json' >/dev/null 2>&1 || true; "
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


def run_link_validation(args: argparse.Namespace) -> int:
    password = os.environ.get("WORMHOLE_MAC_PASSWORD")
    if not password:
        password = getpass.getpass("macOS SSH password: ")

    root = args.local_root
    if root.exists():
        shutil.rmtree(root)
    samples = wh.create_sample_data(root)
    config_path = write_local_config(root, args.local_host, args.remote_host)
    local_proc = None
    client = None
    results: list[dict] = []
    try:
        client = ssh_connect(args.remote_host, args.remote_user, password)
        deploy_remote(client, args.remote_root, args.local_host)
        start_remote_node(client, args.remote_root)
        local_proc = start_local_node(config_path)
        wait_http("127.0.0.1", LOCAL_NODE_PORT)
        wait_http(args.remote_host, REMOTE_NODE_PORT)

        status, data = wh.request_json("127.0.0.1", LOCAL_NODE_PORT, "POST", "/api/connect", {}, timeout=10.0)
        results.append(assert_ok("Windows A connects macOS B", status == 200 and data["ok"], json.dumps(data, ensure_ascii=False)))

        code, out, err = run_ssh(
            client,
            f"cd '{args.remote_root}' && python3 wormhole_validation.py --root '{args.remote_root}' connect --node B",
            timeout=15,
        )
        results.append(assert_ok("macOS B connects Windows A", code == 0 and '"ok": true' in out, out + err))

        status, data = wh.request_json(
            "127.0.0.1",
            LOCAL_NODE_PORT,
            "POST",
            "/api/transfer/send",
            {"paths": [str(samples["text"])]},
            timeout=30.0,
        )
        results.append(assert_ok("Windows sends file to macOS", status == 200 and data["ok"], json.dumps(data, ensure_ascii=False)))

        status, data = wh.request_json(
            "127.0.0.1",
            LOCAL_NODE_PORT,
            "POST",
            "/api/transfer/send",
            {"paths": [str(samples["folder"])]},
            timeout=30.0,
        )
        results.append(assert_ok("Windows sends folder to macOS", status == 200 and data["ok"], json.dumps(data, ensure_ascii=False)))

        code, out, err = run_ssh(
            client,
            f"test -f '{args.remote_root}/B/received/small.txt' && "
            f"test -f '{args.remote_root}/B/received/folder-root/层级一/层级二/deep.txt'",
            timeout=10,
        )
        results.append(assert_ok("macOS received Windows paths", code == 0, out + err))

        code, out, err = run_ssh(
            client,
            f"cd '{args.remote_root}' && python3 wormhole_validation.py --root '{args.remote_root}' "
            "send-file --node B sample_data/mac-small.txt",
            timeout=30,
        )
        results.append(assert_ok("macOS sends file to Windows", code == 0 and '"ok": true' in out, out + err))

        results.append(assert_ok("Windows received macOS file", (root / "A" / "received" / "mac-small.txt").exists()))

        status, data = wh.request_json(
            "127.0.0.1",
            LOCAL_NODE_PORT,
            "POST",
            "/api/clipboard/text/set",
            {"text": "Windows to macOS clipboard model 中文"},
            timeout=10.0,
        )
        results.append(assert_ok("Windows clipboard model to macOS", status == 200 and data["ok"], json.dumps(data, ensure_ascii=False)))

        code, out, err = run_ssh(
            client,
            f"cd '{args.remote_root}' && python3 wormhole_validation.py --root '{args.remote_root}' "
            "clipboard-text --node B 'macOS to Windows clipboard model 中文'",
            timeout=15,
        )
        results.append(assert_ok("macOS clipboard model to Windows", code == 0 and '"ok": true' in out, out + err))

        status, data = wh.request_json("127.0.0.1", LOCAL_NODE_PORT, "GET", "/api/events?limit=300", timeout=10.0)
        event_types = {event["type"] for event in data.get("events", [])}
        needed = {"connection.changed", "transfer.created", "transfer.started", "transfer.progress", "transfer.completed", "clipboard.synced"}
        results.append(assert_ok("Windows event coverage", needed.issubset(event_types), ",".join(sorted(event_types))))

        stop_remote_node(client)
        time.sleep(1.0)
        status, data = wh.request_json("127.0.0.1", LOCAL_NODE_PORT, "POST", "/api/connect", {}, timeout=5.0)
        results.append(assert_ok("macOS shutdown detected", status == 503 and not data["ok"], json.dumps(data, ensure_ascii=False)))

        start_remote_node(client, args.remote_root)
        wait_http(args.remote_host, REMOTE_NODE_PORT)
        status, data = wh.request_json("127.0.0.1", LOCAL_NODE_PORT, "POST", "/api/connect", {}, timeout=10.0)
        results.append(assert_ok("macOS restart reconnects", status == 200 and data["ok"], json.dumps(data, ensure_ascii=False)))

        report = {"ok": True, "local_root": str(root), "remote_root": args.remote_root, "results": results}
        wh.write_json_file(root / "macos-link-result.json", report)
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 0
    finally:
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
    parser = argparse.ArgumentParser(description="Windows <-> macOS validation")
    parser.add_argument("--remote-host", default=DEFAULT_REMOTE_HOST)
    parser.add_argument("--remote-user", default=DEFAULT_REMOTE_USER)
    parser.add_argument("--remote-root", default=DEFAULT_REMOTE_ROOT)
    parser.add_argument("--local-host", default="192.168.1.183")
    parser.add_argument("--local-root", type=Path, default=DEFAULT_LOCAL_ROOT)
    return run_link_validation(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
