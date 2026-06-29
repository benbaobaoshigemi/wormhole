#!/usr/bin/env python3
import getpass
import argparse
import ipaddress
import json
import os
import pathlib
import re
import socket
import subprocess
import sys
import time
import uuid
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
REMOTE_ROOT = "/Users/benbaobaoshigemi/Desktop/hole"
REMOTE_USER = "benbaobaoshigemi"
DISALLOWED_NETWORKS = tuple(
    ipaddress.ip_network(value)
    for value in (
        "0.0.0.0/8",
        "127.0.0.0/8",
        "169.254.0.0/16",
        "198.18.0.0/15",
        "224.0.0.0/4",
    )
)
VIRTUAL_INTERFACE_MARKERS = (
    "clash",
    "mihomo",
    "tun",
    "tap",
    "wintun",
    "wireguard",
    "zerotier",
    "tailscale",
    "vpn",
    "vethernet",
    "hyper-v",
    "vmware",
    "virtualbox",
    "loopback",
)


def run(cmd, cwd=ROOT):
    completed = subprocess.run(cmd, cwd=cwd, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if completed.returncode != 0:
        raise RuntimeError(completed.stdout + completed.stderr)
    return completed.stdout


def local_ip_for(host: str, override: str | None = None) -> str:
    if override:
        return validate_windows_host(override, "--windows-host")

    host_ip = ipaddress.ip_address(host)
    candidates = discover_windows_lan_candidates()
    ranked = sorted(
        candidates,
        key=lambda candidate: candidate.score_for(host_ip),
        reverse=True,
    )
    if ranked and ranked[0].score_for(host_ip) > 0:
        selected = ranked[0]
        print(
            f"Selected Windows LAN IP for macOS peer config: {selected.ip} "
            f"({selected.adapter})"
        )
        return str(selected.ip)

    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
        s.connect((host, 9))
        routed_ip = validate_windows_host(s.getsockname()[0], "UDP route probe")
        print(f"Selected Windows LAN IP from UDP route probe: {routed_ip}")
        return routed_ip


class LanCandidate:
    def __init__(self, adapter: str, ip: ipaddress.IPv4Address, mask: str | None):
        self.adapter = adapter
        self.ip = ip
        self.mask = mask

    def score_for(self, peer_ip: ipaddress._BaseAddress) -> int:
        if not isinstance(peer_ip, ipaddress.IPv4Address):
            return 0
        if self.mask:
            try:
                if peer_ip in ipaddress.ip_network(f"{self.ip}/{self.mask}", strict=False):
                    return 100
            except ValueError:
                pass
        if self.ip.is_private and peer_ip.is_private:
            if str(self.ip).split(".")[:3] == str(peer_ip).split(".")[:3]:
                return 90
            if str(self.ip).split(".")[:2] == str(peer_ip).split(".")[:2]:
                return 50
            return 10
        return 0


def validate_windows_host(value: str, source: str) -> str:
    try:
        ip = ipaddress.ip_address(value)
    except ValueError as exc:
        raise RuntimeError(f"{source} must be an IPv4 address, got {value!r}") from exc
    if not isinstance(ip, ipaddress.IPv4Address):
        raise RuntimeError(f"{source} must be an IPv4 address, got {value!r}")
    if is_disallowed_ip(ip):
        raise RuntimeError(
            f"{source} resolved to {ip}, which is not a valid LAN peer address "
            "for Wormhole. Disable the virtual adapter or pass --windows-host "
            "with the real Windows LAN IP."
        )
    return str(ip)


def is_disallowed_ip(ip: ipaddress.IPv4Address) -> bool:
    return any(ip in network for network in DISALLOWED_NETWORKS)


def is_virtual_adapter(adapter: str) -> bool:
    lowered = adapter.lower()
    return any(marker in lowered for marker in VIRTUAL_INTERFACE_MARKERS)


def discover_windows_lan_candidates() -> list[LanCandidate]:
    if os.name != "nt":
        return []
    completed = subprocess.run(
        ["ipconfig"],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        encoding="utf-8",
        errors="replace",
    )
    if completed.returncode != 0:
        return []

    candidates: list[LanCandidate] = []
    adapter = ""
    current_ip: ipaddress.IPv4Address | None = None
    current_mask: str | None = None

    def flush() -> None:
        nonlocal current_ip, current_mask
        if (
            adapter
            and current_ip
            and not is_virtual_adapter(adapter)
            and not is_disallowed_ip(current_ip)
        ):
            candidates.append(LanCandidate(adapter, current_ip, current_mask))
        current_ip = None
        current_mask = None

    for raw_line in completed.stdout.splitlines():
        line = raw_line.strip()
        if not line:
            continue
        if raw_line and not raw_line[0].isspace() and line.endswith(":"):
            flush()
            adapter = line[:-1]
            continue
        if "IPv4" in line:
            match = re.search(r"(\d{1,3}(?:\.\d{1,3}){3})", line)
            if match:
                current_ip = ipaddress.ip_address(match.group(1))
            continue
        if "Subnet Mask" in line:
            match = re.search(r"(\d{1,3}(?:\.\d{1,3}){3})", line)
            if match:
                current_mask = match.group(1)
    flush()
    return candidates


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
    allow_dirs = ["crates", "apps", "scripts", "assets", "_verification_scripts"]
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
            "max_concurrent_tasks": 2,
            "parallel_chunk_uploads": 4,
            "chunk_size_bytes": 2 * 1024 * 1024,
            "conflict_strategy": "rename",
            "min_free_space_bytes": 64 * 1024 * 1024,
            "verify_hash": True,
            "resume_enabled": True,
        },
    )
    transfer = config["transfer"]
    transfer.setdefault("max_concurrent_tasks", 2)
    transfer["max_concurrent_tasks"] = max(int(transfer.get("max_concurrent_tasks") or 1), 2)
    transfer.setdefault("parallel_chunk_uploads", 4)
    transfer["parallel_chunk_uploads"] = max(int(transfer.get("parallel_chunk_uploads") or 1), 4)
    transfer.setdefault("chunk_size_bytes", 2 * 1024 * 1024)
    transfer["chunk_size_bytes"] = min(
        max(int(transfer.get("chunk_size_bytes") or 64 * 1024), 64 * 1024),
        2 * 1024 * 1024,
    )
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

    parser = argparse.ArgumentParser()
    parser.add_argument("host", nargs="?", default="192.168.1.180")
    parser.add_argument(
        "--windows-host",
        help="Real Windows LAN IPv4 address to write into the macOS peer config.",
    )
    args = parser.parse_args()

    host = args.host
    password = (
        os.environ.get("WORMHOLE_REMOTE_PASSWORD")
        or getpass.getpass("macOS SSH password: ")
    )
    local_ip = local_ip_for(host, args.windows_host)
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
                "cp assets/wormhole/Wormhole.icns target/product/macos/Wormhole.app/Contents/Resources/Wormhole.icns && "
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
                "<key>CFBundleIconFile</key><string>Wormhole.icns</string>\n"
                "<key>CFBundlePackageType</key><string>APPL</string>\n"
                "<key>CFBundleShortVersionString</key><string>0.1.0</string>\n"
                "<key>CFBundleVersion</key><string>0.1.0</string>\n"
                "<key>NSLocalNetworkUsageDescription</key><string>Wormhole needs local network access to connect to the paired Windows computer and transfer files and clipboard data.</string>\n"
                "<key>NSBonjourServices</key><array></array>\n"
                "</dict></plist>\n"
                "PLIST && "
                "codesign --force --deep --sign - target/product/macos/Wormhole.app && "
                "rm -f \"$HOME/Desktop/Wormhole.app\" && "
                "ln -s \"$PWD/target/product/macos/Wormhole.app\" \"$HOME/Desktop/Wormhole.app\""
            ),
        ]
        for command in commands:
            code = run_remote_command(client, command)
            if code != 0:
                raise RuntimeError(f"remote command failed: {command}")
    finally:
        client.close()
    print(f"macOS deployed to {REMOTE_ROOT}")
    return 0


def run_remote_command(client, command: str) -> int:
    transport = client.get_transport()
    if transport is None:
        raise RuntimeError("SSH transport is not connected")
    channel = transport.open_session()
    channel.set_combine_stderr(True)
    channel.exec_command(command)
    while True:
        while channel.recv_ready():
            data = channel.recv(16384)
            if data:
                print(data.decode("utf-8", "replace"), end="")
        if channel.exit_status_ready():
            while channel.recv_ready():
                data = channel.recv(16384)
                if data:
                    print(data.decode("utf-8", "replace"), end="")
            return channel.recv_exit_status()
        time.sleep(0.05)


if __name__ == "__main__":
    raise SystemExit(main())
