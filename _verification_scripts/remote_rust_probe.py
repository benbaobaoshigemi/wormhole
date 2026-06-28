#!/usr/bin/env python3
import getpass
import os
import sys
from pathlib import Path

import paramiko

REMOTE_ROOT = "/Users/benbaobaoshigemi/Desktop/hole"
REMOTE_USER = "benbaobaoshigemi"


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: remote_rust_probe.py <host> <target-host:port>", file=sys.stderr)
        return 2

    host = sys.argv[1]
    target = sys.argv[2]
    password = os.environ.get("WORMHOLE_REMOTE_PASSWORD") or getpass.getpass("macOS SSH password: ")

    client = paramiko.SSHClient()
    client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    client.connect(hostname=host, username=REMOTE_USER, password=password, timeout=20)
    try:
        sftp = client.open_sftp()
        local_root = Path(__file__).resolve().parent / "rust_net_probe"
        for rel in ("Cargo.toml", "src/main.rs"):
            remote_path = f"{REMOTE_ROOT}/_verification_scripts/rust_net_probe/{rel}"
            with sftp.file(remote_path, "w") as remote_file:
                remote_file.write((local_root / rel).read_text(encoding="utf-8"))
        sftp.close()

        command = f"cd '{REMOTE_ROOT}/_verification_scripts/rust_net_probe' && cargo run --quiet -- {target}"
        _, stdout, stderr = client.exec_command(command)
        code = stdout.channel.recv_exit_status()
        out = stdout.read().decode("utf-8", "replace")
        err = stderr.read().decode("utf-8", "replace")
        if out:
            print(out, end="")
        if err:
            print(err, end="", file=sys.stderr)
        return code
    finally:
        client.close()


if __name__ == "__main__":
    raise SystemExit(main())
