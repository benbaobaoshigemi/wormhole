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
import socket


ROOT = Path(__file__).resolve().parents[1]
RUNTIME = ROOT / "_verification_runtime" / "backend_contract"
TOKEN = "contract-token"
PYTHON = Path("C:/Users/zhang/miniconda3/python.exe")


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")


def config(name, port, peer_port, bind_host="127.0.0.1"):
    base = RUNTIME / name
    return {
        "device_id": f"{name}-device",
        "device_name": name,
        "platform": "windows",
        "bind_host": bind_host,
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
            "parallel_chunk_uploads": 4,
            "chunk_size_bytes": 262144,
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


def request(port, method, path, body=None, token=None, raw=None, host="127.0.0.1"):
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
        f"http://{host}:{port}{path}", data=data, method=method, headers=headers
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


def local_lan_ip():
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        sock.connect(("192.168.1.180", 9))
        return sock.getsockname()[0]


def wait_task(port, task_id, wanted_status, timeout=10.0):
    deadline = time.time() + timeout
    last = None
    while time.time() < deadline:
        status, state = request(port, "GET", "/local/state")
        if status == 200:
            for task in state["tasks"]:
                if task["task_id"] == task_id:
                    last = task
                    if task["status"] == wanted_status:
                        return task
        time.sleep(0.1)
    raise AssertionError(f"task {task_id} did not reach {wanted_status}: {last}")


def main():
    shutil.rmtree(RUNTIME, ignore_errors=True)
    cfg_a = RUNTIME / "A" / "config.json"
    cfg_b = RUNTIME / "B" / "config.json"
    write_json(cfg_a, config("A", 55317, 55318))
    write_json(cfg_b, config("B", 55318, 55317, bind_host="0.0.0.0"))

    exe = ROOT / "target" / "debug" / "wormhole-daemon.exe"
    subprocess.run(["cargo", "build", "-p", "wormhole-daemon"], cwd=ROOT, check=True)

    procs = [
        subprocess.Popen([str(exe), "--config", str(cfg_a)], cwd=ROOT),
        subprocess.Popen([str(exe), "--config", str(cfg_b)], cwd=ROOT),
    ]
    try:
        wait_ready(55317)
        wait_ready(55318)

        lan_ip = local_lan_ip()
        assert_true(not lan_ip.startswith("127."), f"local LAN IP probe returned loopback: {lan_ip}")
        status, _ = request(55318, "GET", "/peer/handshake", host=lan_ip)
        if status == 200:
            status, _ = request(55318, "GET", "/local/state", host=lan_ip)
            assert_true(status == 403, "local API accepted non-loopback client")
        else:
            print(f"skip non-loopback local API boundary check; {lan_ip}:55318 is not reachable in this network profile")

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
        duplicate_manifest = {
            "task_id": "dup-task",
            "root_name": "dup",
            "files": [
                {"relative_path": "dup.txt", "size": 1, "sha256": sha},
                {"relative_path": "dup.txt", "size": 1, "sha256": sha},
            ],
            "total_size": 2,
        }
        status, body = request(55318, "POST", "/peer/transfer/prepare", duplicate_manifest, token=TOKEN)
        assert_true(status == 400 and body["error_code"] == "duplicate_path", "duplicate path was not rejected")
        total_mismatch_manifest = {
            "task_id": "bad-total-task",
            "root_name": "bad-total",
            "files": [{"relative_path": "a.txt", "size": 3, "sha256": sha}],
            "total_size": 2,
        }
        status, body = request(55318, "POST", "/peer/transfer/prepare", total_mismatch_manifest, token=TOKEN)
        assert_true(status == 400 and body["error_code"] == "total_size_mismatch", "total_size mismatch was not rejected")
        unsafe_manifest = {
            "task_id": "unsafe-task",
            "root_name": "unsafe",
            "files": [{"relative_path": "../escape.txt", "size": 1, "sha256": sha}],
            "total_size": 1,
        }
        status, body = request(55318, "POST", "/peer/transfer/prepare", unsafe_manifest, token=TOKEN)
        assert_true(status == 400 and body["error_code"] == "unsafe_path", "unsafe path was not rejected")

        status, _ = request(55318, "POST", "/peer/transfer/prepare", manifest, token="wrong")
        assert_true(status == 401, "wrong peer token did not return 401")

        status, body = request(55318, "POST", "/peer/transfer/prepare", manifest, token=TOKEN)
        assert_true(status == 200 and body["ok"], "peer prepare failed")
        status, _ = request(
            55318,
            "GET",
            "/peer/transfer/upload-status/contract-task?path=missing.txt",
            token=TOKEN,
        )
        assert_true(status == 404, "upload path outside manifest was not rejected")
        wrong_sha = "0" * 64
        status, body = request(
            55318,
            "GET",
            f"/peer/transfer/upload-status/contract-task?path=contract.txt&sha256={wrong_sha}",
            token=TOKEN,
        )
        assert_true(status == 409 and body["error_code"] == "sha256_mismatch", "sha mismatch was not rejected")
        status, body = request(
            55318,
            "GET",
            "/peer/transfer/upload-status/contract-task?path=contract.txt",
            token=TOKEN,
        )
        assert_true(status == 200 and body["offset"] == 0 and body["parallel_upload"], "upload status failed")
        split = 5
        query = urllib.parse.urlencode(
            {"path": "contract.txt", "offset": split, "final_chunk": "true", "sha256": sha}
        )
        status, body = request(
            55318,
            "POST",
            f"/peer/transfer/upload-chunk/contract-task?{query}",
            token=TOKEN,
            raw=data[split:],
        )
        assert_true(status == 200 and body["received"] == len(data) - split, "out-of-order chunk failed")
        status, body = request(
            55318,
            "GET",
            "/peer/transfer/upload-status/contract-task?path=contract.txt",
            token=TOKEN,
        )
        assert_true(
            status == 200 and body["offset"] == 0 and body["received_ranges"] == [[split, len(data)]],
            f"out-of-order status did not expose received range: {body}",
        )
        query = urllib.parse.urlencode(
            {"path": "contract.txt", "offset": 0, "final_chunk": "false", "sha256": sha}
        )
        status, body = request(
            55318,
            "POST",
            f"/peer/transfer/upload-chunk/contract-task?{query}",
            token=TOKEN,
            raw=data[:split],
        )
        assert_true(status == 200 and body["received"] == split and body["complete"], "chunk upload failed")
        assert_true(
            (RUNTIME / "B" / "received" / "contract.txt").read_bytes() == data,
            "received file content mismatch",
        )
        task = wait_task(55318, "contract-task", "completed")
        assert_true(task["transferred_size"] == len(data), "final chunk did not complete receive task")

        progress_data = b"x" * (512 * 1024)
        progress_sha = hashlib.sha256(progress_data).hexdigest()
        progress_manifest = {
            "task_id": "progress-task",
            "root_name": "progress.bin",
            "files": [{"relative_path": "progress.bin", "size": len(progress_data), "sha256": progress_sha}],
            "total_size": len(progress_data),
        }
        status, body = request(55318, "POST", "/peer/transfer/prepare", progress_manifest, token=TOKEN)
        assert_true(status == 200 and body["ok"], "progress prepare failed")
        first = progress_data[:262144]
        second = progress_data[262144:]
        query = urllib.parse.urlencode(
            {"path": "progress.bin", "offset": 0, "final_chunk": "false", "sha256": progress_sha}
        )
        status, body = request(
            55318,
            "POST",
            f"/peer/transfer/upload-chunk/progress-task?{query}",
            token=TOKEN,
            raw=first,
        )
        assert_true(status == 200, "first progress chunk failed")
        status, state = request(55318, "GET", "/local/state")
        progress_task = next(task for task in state["tasks"] if task["task_id"] == "progress-task")
        assert_true(progress_task["transferred_size"] == len(first), "local state did not expose in-memory transferred_size")
        query = urllib.parse.urlencode(
            {"path": "progress.bin", "offset": len(first), "final_chunk": "true", "sha256": progress_sha}
        )
        status, body = request(
            55318,
            "POST",
            f"/peer/transfer/upload-chunk/progress-task?{query}",
            token=TOKEN,
            raw=second,
        )
        assert_true(status == 200, "final progress chunk failed")

        bad_data = b"bad"
        expected_sha = hashlib.sha256(b"expected").hexdigest()
        mismatch_manifest = {
            "task_id": "hash-mismatch-task",
            "root_name": "hash.txt",
            "files": [{"relative_path": "hash.txt", "size": len(bad_data), "sha256": expected_sha}],
            "total_size": len(bad_data),
        }
        status, body = request(55318, "POST", "/peer/transfer/prepare", mismatch_manifest, token=TOKEN)
        assert_true(status == 200 and body["ok"], "hash mismatch prepare failed")
        query = urllib.parse.urlencode(
            {"path": "hash.txt", "offset": 0, "final_chunk": "true", "sha256": expected_sha}
        )
        status, body = request(
            55318,
            "POST",
            f"/peer/transfer/upload-chunk/hash-mismatch-task?{query}",
            token=TOKEN,
            raw=bad_data,
        )
        assert_true(status == 422 and body["error_code"] == "integrity", "final hash mismatch was not rejected")
        task = wait_task(55318, "hash-mismatch-task", "failed")
        assert_true(task["error_code"] == "integrity", "hash mismatch was not observable as failed task")

        source_dir = RUNTIME / "A" / "source"
        source_dir.mkdir(parents=True, exist_ok=True)
        large_parallel = source_dir / "parallel-large.bin"
        large_parallel.write_bytes(hashlib.sha256(b"parallel-large").digest() * 262144)
        status, body = request(55317, "POST", "/local/transfer/send", {"paths": [str(large_parallel)]})
        assert_true(status == 200 and body["ok"], f"parallel large send was not accepted: {body}")
        parallel_task = wait_task(55317, body["task_id"], "completed", timeout=20.0)
        assert_true(parallel_task["transferred_size"] == large_parallel.stat().st_size, "parallel large transfer size mismatch")
        assert_true(
            (RUNTIME / "B" / "received" / "parallel-large.bin").read_bytes() == large_parallel.read_bytes(),
            "parallel large received file content mismatch",
        )

        file_a = source_dir / "a.txt"
        file_b = source_dir / "b.txt"
        file_a.write_bytes(b"a" * 17)
        file_b.write_bytes(b"b" * 23)
        total_size = file_a.stat().st_size + file_b.stat().st_size
        status, _ = request(55317, "POST", "/local/settings/update", {"peer_port": 55999})
        assert_true(status == 200, "failed to point A at unavailable peer")
        status, body = request(55317, "POST", "/local/transfer/send", {"paths": [str(file_a), str(file_b)]})
        assert_true(status == 200 and body["ok"], "failed to create retry test transfer")
        retry_task_id = body["task_id"]
        failed_task = wait_task(55317, retry_task_id, "failed")
        assert_true(failed_task["item_count"] == 2 and failed_task["total_size"] == total_size, "failed multi-file task metadata shrank before retry")
        status, _ = request(55317, "POST", "/local/transfer/cancel", {"task_id": retry_task_id})
        assert_true(status == 200, "cancel before retry failed")
        status, _ = request(55317, "POST", "/local/settings/update", {"peer_port": 55318})
        assert_true(status == 200, "failed to restore A peer port")
        status, body = request(55317, "POST", "/local/transfer/retry", {"task_id": retry_task_id})
        assert_true(status == 200 and body["task_id"] == retry_task_id, "specified retry did not preserve task id")
        retried_task = wait_task(55317, retry_task_id, "completed", timeout=15.0)
        assert_true(retried_task["retry_count"] == 1, "retry_count did not increment")
        assert_true(retried_task["item_count"] == 2, "retry item_count shrank")
        assert_true(retried_task["total_size"] == total_size, "retry total_size changed")
        assert_true((RUNTIME / "B" / "received" / "a.txt").exists(), "retried file a not received")
        assert_true((RUNTIME / "B" / "received" / "b.txt").exists(), "retried file b not received")

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
