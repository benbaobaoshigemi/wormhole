#!/usr/bin/env python3
import pathlib
import signal
import subprocess
import sys
from typing import Optional

REMOTE_ROOT = pathlib.Path("/Users/benbaobaoshigemi/Desktop/hole")
CONFIG = REMOTE_ROOT / ".wormhole" / "macos" / "config.json"
LOG = REMOTE_ROOT / ".wormhole" / "macos" / "daemon.log"
DAEMON_PID = REMOTE_ROOT / ".wormhole" / "macos" / "daemon.pid"
SUPERVISOR_PID = REMOTE_ROOT / ".wormhole" / "macos" / "supervisor.pid"

child: Optional[subprocess.Popen] = None


def stop_child(signum, _frame):
    if child and child.poll() is None:
        child.terminate()
        try:
            child.wait(timeout=10)
        except subprocess.TimeoutExpired:
            child.kill()
    raise SystemExit(128 + signum)


def main() -> int:
    global child
    signal.signal(signal.SIGTERM, stop_child)
    signal.signal(signal.SIGINT, stop_child)

    LOG.parent.mkdir(parents=True, exist_ok=True)
    SUPERVISOR_PID.write_text(str(os_getpid()), encoding="utf-8")
    with LOG.open("ab") as log:
        child = subprocess.Popen(
            [str(REMOTE_ROOT / "target" / "release" / "wormhole-daemon"), "--config", str(CONFIG)],
            cwd=str(REMOTE_ROOT),
            stdout=log,
            stderr=subprocess.STDOUT,
        )
        DAEMON_PID.write_text(str(child.pid), encoding="utf-8")
        return child.wait()


def os_getpid() -> int:
    import os

    return os.getpid()


if __name__ == "__main__":
    raise SystemExit(main())
