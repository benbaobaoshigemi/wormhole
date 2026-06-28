#!/usr/bin/env python3
import getpass
import os
import sys

import paramiko

REMOTE_ROOT = "/Users/benbaobaoshigemi/Desktop/hole"
REMOTE_USER = "benbaobaoshigemi"


def main() -> int:
    host = sys.argv[1] if len(sys.argv) > 1 else "192.168.1.180"
    password = os.environ.get("WORMHOLE_MAC_PASSWORD") or getpass.getpass("macOS SSH password: ")
    client = paramiko.SSHClient()
    client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    client.connect(hostname=host, username=REMOTE_USER, password=password, timeout=20)
    try:
        command = f"cd '{REMOTE_ROOT}' && sh scripts/start-macos-screen-daemon.sh"
        _, stdout, stderr = client.exec_command(command)
        code = stdout.channel.recv_exit_status()
        out = stdout.read().decode("utf-8", "replace")
        err = stderr.read().decode("utf-8", "replace")
        if code != 0:
            raise RuntimeError(out + err)
        print(out.strip())
    finally:
        client.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
