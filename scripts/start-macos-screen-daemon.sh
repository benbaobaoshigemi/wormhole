#!/bin/sh
set -eu

ROOT="/Users/benbaobaoshigemi/Desktop/hole"
LOG="$ROOT/.wormhole/macos/screen-start.log"

cd "$ROOT"
{
  date
  for pid in $(pgrep -x wormhole-daemon || true); do
    kill "$pid" >/dev/null 2>&1 || true
  done
  /usr/bin/screen -S wormhole-daemon -X quit >/dev/null 2>&1 || true
  TERM=xterm /usr/bin/screen -dmS wormhole-daemon /bin/sh -lc \
    'cd /Users/benbaobaoshigemi/Desktop/hole && unset http_proxy; unset https_proxy; unset all_proxy; unset HTTP_PROXY; unset HTTPS_PROXY; unset ALL_PROXY; exec ./target/release/wormhole-daemon --config .wormhole/macos/config.json >> .wormhole/macos/daemon.log 2>&1'
  sleep 1
  /usr/bin/screen -ls || true
  pgrep -x wormhole-daemon || true
} >> "$LOG" 2>&1

pgrep -x wormhole-daemon | head -n 1
