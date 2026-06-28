# Security Boundary

## Local API

`/local/*` is for the local UI, CLI, and debug UI only. The daemon rejects non-loopback clients before handlers run. Valid local clients are `127.0.0.1` and `::1`. Browser origins are additionally limited to `http://127.0.0.1:*` and `http://localhost:*`.

The daemon does not rewrite `bind_host`. A single listener may bind `0.0.0.0` or a LAN address so `/peer/*` remains reachable by the fixed peer device. `/local/*` remains protected by the loopback middleware even on a LAN-bound listener.

## Peer API

`/peer/*` is for the fixed peer device. It may be reachable on the LAN when `bind_host` is `0.0.0.0` or a LAN interface address. Peer write endpoints require `x-wormhole-token` when `shared_token` is configured.

Handshake is intentionally public because it returns only `PublicDevice` fields: device id, display name, platform, port, protocol version, and capabilities.

## Token Rules

`shared_token` is stored only in the local config file. It is never returned by `/local/state`, `/local/settings`, task DTOs, history DTOs, events, or any peer response.

## CORS

The daemon no longer uses permissive CORS. Browser origins are limited to loopback development origins. Allowed browser methods are `GET` and `POST`; allowed browser request headers are limited to `content-type`. LAN web pages cannot call `/local/*` from a browser context.

## Never Return To Frontend

- `shared_token`
- source file paths
- temp paths
- internal receive indexes
- clipboard text
- PNG bytes
- platform implementation details

## Never Send To Peer

- sender absolute paths
- config paths
- data directory paths
- local UI settings unrelated to transfer or clipboard protocol
- clipboard text except the explicit text clipboard receive payload
- image bytes through JSON

## Logging Privacy

Logs may include error codes, task ids, file names, sizes, connection status, and system API failures. Logs must not include tokens, clipboard text, image bytes, file contents, or private config values.

## SQLite Privacy Boundary

SQLite is a local private store. It may contain internal task records with sender `source_paths` for retry and recovery, but those records are never returned directly through Local API DTOs, events, or Peer API payloads.
