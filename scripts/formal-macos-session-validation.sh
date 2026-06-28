#!/bin/sh
set -eu

ROOT="/Users/benbaobaoshigemi/Desktop/hole"
LOG="$ROOT/.wormhole/macos/formal-session-validation.log"
PNG_B64="iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFUlEQVR4nGP8z8Dwn4GBgYEJRIAwAB8XAgICR7MUAAAAAElFTkSuQmCC"

cd "$ROOT"
mkdir -p sample_data/mac-folder .wormhole/macos
printf 'macOS formal file -> Windows\n' > sample_data/macos-formal-file.txt
printf 'macOS formal folder -> Windows\n' > sample_data/mac-folder/nested.txt
printf '%s' "$PNG_B64" | base64 -d > sample_data/macos-clipboard.png

for pid in $(pgrep -x wormhole-daemon || true); do
  kill "$pid" >/dev/null 2>&1 || true
done

./target/release/wormhole-daemon --config .wormhole/macos/config.json >> "$LOG" 2>&1 &
daemon_pid=$!
trap 'kill "$daemon_pid" >/dev/null 2>&1 || true' EXIT INT TERM
sleep 2

./target/release/wormhole-cli --api http://127.0.0.1:53318 connect
./target/release/wormhole-cli --api http://127.0.0.1:53318 send sample_data/macos-formal-file.txt
./target/release/wormhole-cli --api http://127.0.0.1:53318 send sample_data/mac-folder

printf 'macOS formal text clipboard -> Windows' | LANG=en_US.UTF-8 LC_CTYPE=en_US.UTF-8 pbcopy
./target/release/wormhole-cli --api http://127.0.0.1:53318 clipboard-text

osascript <<'APPLESCRIPT'
use framework "Foundation"
use framework "AppKit"
use scripting additions
set imagePath to "/Users/benbaobaoshigemi/Desktop/hole/sample_data/macos-clipboard.png"
set imageData to current application's NSData's dataWithContentsOfFile:imagePath
set pasteboard to current application's NSPasteboard's generalPasteboard()
pasteboard's clearContents()
pasteboard's setData:imageData forType:(current application's NSPasteboardTypePNG)
APPLESCRIPT
./target/release/wormhole-cli --api http://127.0.0.1:53318 clipboard-image

sleep 3
