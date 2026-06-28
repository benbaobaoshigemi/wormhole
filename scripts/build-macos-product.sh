#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
UI_DIR="$ROOT/apps/desktop-ui"
OUT_ROOT="$ROOT/target/product/macos"
APP="$OUT_ROOT/Wormhole.app"
CONTENTS="$APP/Contents"
MACOS="$CONTENTS/MacOS"
RESOURCES="$CONTENTS/Resources"

cd "$UI_DIR"
npm install
npm run build

cd "$ROOT"
cargo build --package wormhole-daemon --package wormhole-desktop --release

rm -rf "$APP"
mkdir -p "$MACOS/config" "$RESOURCES/web" "$RESOURCES/config"
cp "$ROOT/target/release/wormhole-desktop" "$MACOS/Wormhole"
cp "$ROOT/target/release/wormhole-daemon" "$MACOS/wormhole-daemon"
cp "$ROOT/assets/wormhole/Wormhole.icns" "$RESOURCES/Wormhole.icns"
cp -R "$UI_DIR/dist/." "$RESOURCES/web/"
cp -R "$UI_DIR/dist/." "$MACOS/web/"
if [ -f "$ROOT/.wormhole/macos/config.json" ]; then
  cp "$ROOT/.wormhole/macos/config.json" "$MACOS/config/config.json"
fi
cat > "$CONTENTS/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>Wormhole</string>
  <key>CFBundleIdentifier</key><string>dev.wormhole.desktop</string>
  <key>CFBundleName</key><string>Wormhole</string>
  <key>CFBundleIconFile</key><string>Wormhole.icns</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>NSLocalNetworkUsageDescription</key><string>Wormhole needs local network access to connect to the paired computer and transfer files and clipboard data.</string>
  <key>NSBonjourServices</key><array></array>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$APP"

DESKTOP_APP="$HOME/Desktop/Wormhole.app"
rm -f "$DESKTOP_APP"
ln -s "$APP" "$DESKTOP_APP"

echo "Wormhole product output: $APP"
echo "Desktop app shortcut: $DESKTOP_APP"
