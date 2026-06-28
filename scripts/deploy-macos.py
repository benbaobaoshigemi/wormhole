#!/usr/bin/env python3
import getpass
import json
import os
import pathlib
import socket
import subprocess
import sys
import uuid
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
REMOTE_ROOT = "/Users/benbaobaoshigemi/Desktop/hole"
REMOTE_USER = "benbaobaoshigemi"


def run(cmd, cwd=ROOT):
    completed = subprocess.run(cmd, cwd=cwd, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if completed.returncode != 0:
        raise RuntimeError(completed.stdout + completed.stderr)
    return completed.stdout


def local_ip_for(host: str) -> str:
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
        s.connect((host, 9))
        return s.getsockname()[0]


def make_source_package(package: pathlib.Path) -> None:
    if package.exists():
        package.unlink()
    ignored_parts = {
        "node_modules",
        "target",
        "target.bak",
        ".git",
        "__pycache__",
    }
    allow_dirs = ["crates", "apps", "scripts", "_verification_scripts"]
    allow_files = ["Cargo.toml", "Cargo.lock", "project.md", "AGENTS.md"]
    with zipfile.ZipFile(package, "w", compression=zipfile.ZIP_DEFLATED, allowZip64=True) as zf:
        for name in allow_files:
            path = ROOT / name
            if path.exists():
                zf.write(path, path.relative_to(ROOT).as_posix())
        for directory in allow_dirs:
            root = ROOT / directory
            if not root.exists():
                continue
            for path in root.rglob("*"):
                rel = path.relative_to(ROOT)
                if any(part in ignored_parts for part in rel.parts):
                    continue
                if path.suffix == ".bak":
                    continue
                if path.is_file():
                    zf.write(path, path.relative_to(ROOT).as_posix())
        mac_config = ROOT / ".wormhole" / "macos" / "config.json"
        zf.write(mac_config, mac_config.relative_to(ROOT).as_posix())


def fill_config_defaults(config: dict) -> dict:
    clipboard = config.setdefault("clipboard", {})
    clipboard.setdefault("poll_millis", 750)
    clipboard.setdefault("remote_hash_window", 128)
    config.setdefault(
        "transfer",
        {
            "max_concurrent_tasks": 1,
            "conflict_strategy": "rename",
            "min_free_space_bytes": 64 * 1024 * 1024,
            "verify_hash": True,
            "resume_enabled": True,
        },
    )
    transfer = config["transfer"]
    transfer.setdefault("max_concurrent_tasks", 1)
    transfer.setdefault("conflict_strategy", "rename")
    transfer.setdefault("min_free_space_bytes", 64 * 1024 * 1024)
    transfer.setdefault("verify_hash", True)
    transfer.setdefault("resume_enabled", True)
    config.setdefault("connection", {"heartbeat_millis": 5000, "reconnect_millis": 3000})
    config["connection"].setdefault("heartbeat_millis", 5000)
    config["connection"].setdefault("reconnect_millis", 3000)
    config.setdefault("history_retention_days", 30)
    config.setdefault("min_peer_protocol_version", 1)
    config.setdefault("max_peer_protocol_version", 1)
    return config


def main() -> int:
    import paramiko

    host = sys.argv[1] if len(sys.argv) > 1 else "192.168.1.180"
    password = os.environ.get("WORMHOLE_REMOTE_PASSWORD") or getpass.getpass("macOS SSH password: ")
    local_ip = local_ip_for(host)
    mac_config_path = ROOT / ".wormhole" / "macos" / "config.json"
    win_config_path = ROOT / ".wormhole" / "windows" / "config.json"
    if not mac_config_path.exists():
        run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(ROOT / "scripts" / "init-mvp-configs.ps1"), "-MacHost", host])
    win_config = fill_config_defaults(json.loads(win_config_path.read_text(encoding="utf-8-sig")))
    config = fill_config_defaults(json.loads(mac_config_path.read_text(encoding="utf-8-sig")))
    shared_token = win_config.get("shared_token") or config.get("shared_token") or str(uuid.uuid4())
    win_config["shared_token"] = shared_token
    config["shared_token"] = shared_token
    config["peer"]["host"] = local_ip
    win_config_path.write_text(json.dumps(win_config, ensure_ascii=False, indent=2), encoding="utf-8")
    mac_config_path.write_text(json.dumps(config, ensure_ascii=False, indent=2), encoding="utf-8")

    package = ROOT / ".wormhole" / "wormhole-mvp-source.zip"
    make_source_package(package)

    client = paramiko.SSHClient()
    client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    client.connect(hostname=host, username=REMOTE_USER, password=password, timeout=20)
    try:
        sftp = client.open_sftp()
        try:
            client.exec_command(f"mkdir -p '{REMOTE_ROOT}/.wormhole'")
            sftp.put(str(package), f"{REMOTE_ROOT}/.wormhole/wormhole-mvp-source.zip")
        finally:
            sftp.close()
        commands = [
            f"cd '{REMOTE_ROOT}' && ditto -x -k '.wormhole/wormhole-mvp-source.zip' '{REMOTE_ROOT}'",
            "command -v cargo >/dev/null 2>&1 || (curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal)",
            f"cd '{REMOTE_ROOT}' && PATH=\"$HOME/.cargo/bin:$PATH\" CARGO_HTTP_TIMEOUT=600 CARGO_HTTP_LOW_SPEED_LIMIT=1 cargo build --release -p wormhole-daemon -p wormhole-cli -p wormhole-desktop",
            f"mkdir -p '{REMOTE_ROOT}/.wormhole/macos/received' '{REMOTE_ROOT}/.wormhole/macos/data'",
            (
                f"cd '{REMOTE_ROOT}' && rm -rf target/product/macos/Wormhole.app && "
                "mkdir -p target/product/macos/Wormhole.app/Contents/MacOS "
                "target/product/macos/Wormhole.app/Contents/MacOS/config "
                "target/product/macos/Wormhole.app/Contents/Resources/web "
                "target/product/macos/Wormhole.app/Contents/Resources/config && "
                "cp target/release/wormhole-desktop target/product/macos/Wormhole.app/Contents/MacOS/Wormhole && "
                "cp target/release/wormhole-daemon target/product/macos/Wormhole.app/Contents/MacOS/wormhole-daemon && "
                "cp .wormhole/macos/config.json target/product/macos/Wormhole.app/Contents/MacOS/config/config.json && "
                "cp -R apps/desktop-ui/dist/. target/product/macos/Wormhole.app/Contents/MacOS/web/ && "
                "cp -R apps/desktop-ui/dist/. target/product/macos/Wormhole.app/Contents/Resources/web/ && "
                "cat > target/product/macos/Wormhole.app/Contents/Info.plist <<'PLIST'\n"
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
                "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n"
                "<plist version=\"1.0\"><dict>\n"
                "<key>CFBundleExecutable</key><string>Wormhole</string>\n"
                "<key>CFBundleIdentifier</key><string>dev.wormhole.desktop</string>\n"
                "<key>CFBundleName</key><string>Wormhole</string>\n"
                "<key>CFBundleDisplayName</key><string>Wormhole</string>\n"
                "<key>CFBundlePackageType</key><string>APPL</string>\n"
                "<key>CFBundleShortVersionString</key><string>0.1.0</string>\n"
                "<key>CFBundleVersion</key><string>0.1.0</string>\n"
                "<key>NSLocalNetworkUsageDescription</key><string>Wormhole needs local network access to connect to the paired Windows computer and transfer files and clipboard data.</string>\n"
                "<key>NSBonjourServices</key><array></array>\n"
                "</dict></plist>\n"
                "PLIST && "
                "codesign --force --deep --sign - target/product/macos/Wormhole.app"
            ),
        ]
        for command in commands:
            _, stdout, stderr = client.exec_command(command)
            code = stdout.channel.recv_exit_status()
            out = stdout.read().decode("utf-8", "replace")
            err = stderr.read().decode("utf-8", "replace")
            print(out, end="")
            print(err, end="", file=sys.stderr)
            if code != 0:
                raise RuntimeError(f"remote command failed: {command}")
    finally:
        client.close()
    print(f"macOS deployed to {REMOTE_ROOT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
