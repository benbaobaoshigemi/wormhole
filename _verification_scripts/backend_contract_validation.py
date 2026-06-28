import base64
import hashlib
import json
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUNTIME = ROOT / "_verification_runtime" / "backend_contract"
TOKEN = "contract-token"
PYTHON = Path("C:/Users/zhang/miniconda3/python.exe")


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")


def config(name, port, peer_port):
    base = RUNTIME / name
    return {
        "device_id": f"{name}-device",
        "device_name": name,
        "platform": "windows",
        "bind_host": "127.0.0.1",
        "port": port,
        "peer": {"name": "peer", "host": "127.0.0.1", "port": peer_port},
        "receive_dir": str(base / "received"),
        "data_dir": str(base / "data"),
        "auto_connect": False,
        "clipboard": {
            "enabled": False,
            "text_enabled": True,
            "image_enabled": True,
            "max_image_bytes": 1024,
            "poll_millis": 750,
            "remote_hash_window": 128,
        },
        "shared_token": TOKEN,
        "transfer": {
            "max_concurrent_tasks": 1,
            "conflict_strategy": "rename",
            "min_free_space_bytes": 0,
            "verify_hash": True,
            "resume_enabled": True,
        },
        "connection": {"heartbeat_millis": 5000, "reconnect_millis": 3000},
        "history_retention_days": 30,
        "min_peer_protocol_version": 1,
        "max_peer_protocol_version": 1,
        "retry_limit": 3,
    }


def request(port, method, path, body=None, token=None, raw=None):
    data = None
    headers = {}
    if body is not None:
        data = json.dumps(body).encode("utf-8")
        headers["content-type"] = "application/json"
    if raw is not None:
        data = raw
        headers["content-type"] = "application/octet-stream"
    if token is not None:
        headers["x-wormhole-token"] = token
    req = urllib.request.Request(
        f"http://127.0.0.1:{port}{path}", data=data, method=method, headers=headers
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            text = resp.read().decode("utf-8")
            return resp.status, json.loads(text) if text else None
    except urllib.error.HTTPError as exc:
        text = exc.read().decode("utf-8")
        try:
            return exc.code, json.loads(text) if text else None
        except json.JSONDecodeError:
            return exc.code, {"raw": text}


def wait_ready(port):
    for _ in range(80):
        try:
            status, _ = request(port, "GET", "/local/state")
            if status == 200:
                return
        except Exception:
            pass
        time.sleep(0.1)
    raise RuntimeError(f"daemon on {port} did not become ready")


def assert_true(condition, message):
    if not condition:
        raise AssertionError(message)


def main():
    shutil.rmtree(RUNTIME, ignore_errors=True)
    cfg_a = RUNTIME / "A" / "config.json"
    cfg_b = RUNTIME / "B" / "config.json"
    write_json(cfg_a, config("A", 55317, 55318))
    write_json(cfg_b, config("B", 55318, 55317))

    exe = ROOT / "target" / "debug" / "wormhole-daemon.exe"
    if not exe.exists():
        subprocess.run(["cargo", "build", "-p", "wormhole-daemon"], cwd=ROOT, check=True)

    procs = [
        subprocess.Popen([str(exe), "--config", str(cfg_a)], cwd=ROOT),
        subprocess.Popen([str(exe), "--config", str(cfg_b)], cwd=ROOT),
    ]
    try:
        wait_ready(55317)
        wait_ready(55318)

        status, state = request(55317, "GET", "/local/state")
        assert_true(status == 200, "local state failed")
        encoded_state = json.dumps(state)
        assert_true("shared_token" not in encoded_state, "local state leaked shared_token")
        assert_true("/api/" not in encoded_state, "state referenced removed api namespace")

        data = b"hello-contract"
        sha = hashlib.sha256(data).hexdigest()
        manifest = {
            "task_id": "contract-task",
            "root_name": "contract.txt",
            "files": [{"relative_path": "contract.txt", "size": len(data), "sha256": sha}],
            "total_size": len(data),
        }
        status, _ = request(55318, "POST", "/peer/transfer/prepare", manifest, token="wrong")
        assert_true(status == 401, "wrong peer token did not return 401")

        status, body = request(55318, "POST", "/peer/transfer/prepare", manifest, token=TOKEN)
        assert_true(status == 200 and body["ok"], "peer prepare failed")
        status, body = request(
            55318,
            "GET",
            "/peer/transfer/upload-status/contract-task?path=contract.txt",
            token=TOKEN,
        )
        assert_true(status == 200 and body["offset"] == 0, "upload status failed")
        query = urllib.parse.urlencode(
            {"path": "contract.txt", "offset": 0, "final_chunk": "true", "sha256": sha}
        )
        status, body = request(
            55318,
            "POST",
            f"/peer/transfer/upload-chunk/contract-task?{query}",
            token=TOKEN,
            raw=data,
        )
        assert_true(status == 200 and body["received"] == len(data), "chunk upload failed")
        assert_true(
            (RUNTIME / "B" / "received" / "contract.txt").read_bytes() == data,
            "received file content mismatch",
        )

        too_large = {"hash": sha, "source_device_id": "A-device", "size": 4096}
        status, body = request(
            55318, "POST", "/peer/clipboard/image/prepare", too_large, token=TOKEN
        )
        assert_true(status == 200 and not body["accepted"], "oversized image was accepted")

        png = base64.b64decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
        )
        png_hash = hashlib.sha256(png).hexdigest()
        status, body = request(
            55318,
            "POST",
            "/peer/clipboard/image/prepare",
            {"hash": png_hash, "source_device_id": "A-device", "size": len(png)},
            token=TOKEN,
        )
        assert_true(status == 200 and body["accepted"], "image prepare rejected valid image")
        query = urllib.parse.urlencode(
            {
                "hash": png_hash,
                "source_device_id": "A-device",
                "offset": 0,
                "final_chunk": "true",
            }
        )
        status, body = request(
            55318,
            "POST",
            f"/peer/clipboard/image/chunk?{query}",
            token=TOKEN,
            raw=png,
        )
        assert_true(status == 200 and body["hash"] == png_hash, f"image chunk failed: {status} {body}")

        print("backend contract validation ok")
    finally:
        for proc in procs:
            proc.terminate()
        for proc in procs:
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()


if __name__ == "__main__":
    main()
