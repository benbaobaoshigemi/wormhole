#!/usr/bin/env python3
import os
import pathlib
import stat

REMOTE_ROOT = pathlib.Path("/Users/benbaobaoshigemi/Desktop/hole")
APP_ROOT = REMOTE_ROOT / "WormholeDaemon.app"
CONTENTS = APP_ROOT / "Contents"
MACOS = CONTENTS / "MacOS"

INFO_PLIST = """<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key>
  <string>com.wormholelink.daemon</string>
  <key>CFBundleName</key>
  <string>Wormhole Daemon</string>
  <key>CFBundleDisplayName</key>
  <string>Wormhole Link</string>
  <key>CFBundleExecutable</key>
  <string>wormhole-daemon-launcher</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1.0</string>
  <key>CFBundleVersion</key>
  <string>0.1.0</string>
  <key>LSBackgroundOnly</key>
  <true/>
  <key>NSLocalNetworkUsageDescription</key>
  <string>Wormhole Link needs local network access to connect your two computers and transfer files and clipboard data.</string>
</dict>
</plist>
"""

LAUNCHER = """#!/bin/sh
cd /Users/benbaobaoshigemi/Desktop/hole || exit 1
exec ./target/release/wormhole-daemon --config .wormhole/macos/config.json >> .wormhole/macos/daemon.log 2>&1
"""


def main() -> int:
    MACOS.mkdir(parents=True, exist_ok=True)
    (CONTENTS / "Info.plist").write_text(INFO_PLIST, encoding="utf-8")
    launcher = MACOS / "wormhole-daemon-launcher"
    launcher.write_text(LAUNCHER, encoding="utf-8")
    launcher.chmod(launcher.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    os.utime(APP_ROOT, None)
    print(APP_ROOT)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
