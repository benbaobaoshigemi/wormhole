#!/usr/bin/env python3
import getpass
import os
import sys

import paramiko

REMOTE_ROOT = "/Users/benbaobaoshigemi/Desktop/hole"
REMOTE_USER = "benbaobaoshigemi"


def main() -> int:
    if len(sys.argv) < 3:
        print("usage: remote_macos_exec.py <host> <command>", file=sys.stderr)
        return 2
    host = sys.argv[1]
    command = sys.argv[2]
    password = os.environ.get("WORMHOLE_MAC_PASSWORD") or getpass.getpass("macOS SSH password: ")
    client = paramiko.SSHClient()
    client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    client.connect(hostname=host, username=REMOTE_USER, password=password, timeout=20)
    try:
        full_command = f"cd '{REMOTE_ROOT}' && {command}"
        _, stdout, stderr = client.exec_command(full_command)
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
