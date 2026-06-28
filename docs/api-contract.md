# API Contract

The daemon exposes only two namespaces:

- `/local/*`: local UI, CLI, and debug UI.
- `/peer/*`: the fixed peer device.

There is no `/api/*` compatibility layer.

## Error Format

```json
{
  "ok": false,
  "error_code": "offset_mismatch",
  "error": "upload offset mismatch"
}
```

## Local API

`GET /local/state`

Returns the complete frontend recovery state: device, connection status, peer, public settings, clipboard status, active transfer count, recent history count, task DTOs, and recent events.

`GET /local/settings`

Returns `PublicSettingsDto`. It never contains `shared_token`.

`POST /local/settings/update`

Accepts partial settings fields: `device_name`, `peer_name`, `peer_host`, `peer_port`, `receive_dir`, `auto_connect`, `clipboard_enabled`, `clipboard_text_enabled`, `clipboard_image_enabled`, `max_image_bytes`, `retry_limit`.

`POST /local/connect`

Starts a peer handshake.

`POST /local/disconnect`

Marks the peer offline locally.

`POST /local/transfer/send`

Body:

```json
{ "paths": ["C:/Users/zhang/Desktop/a.txt"] }
```

Returns `{ "ok": true, "task_id": "..." }`.

`POST /local/transfer/cancel`

Body:

```json
{ "task_id": "..." }
```

`POST /local/transfer/retry`

Retries the latest failed task while preserving the user-visible `task_id`.

`GET /local/transfer/tasks`

Returns `TransferTaskDto[]`.

`GET /local/transfer/history`

Returns `TransferHistoryDto[]`.

`POST /local/transfer/history/clear`

Clears transfer history.

`GET /local/clipboard/status`

Returns clipboard status flags and image limit.

`POST /local/clipboard/enable`

Enables clipboard sync.

`POST /local/clipboard/disable`

Disables clipboard sync.

`POST /local/clipboard/system/read-send-text`

Reads current system text clipboard and sends it to the peer. The response and events never include clipboard text.

`POST /local/clipboard/system/read-send-image`

Reads current system image clipboard and sends PNG chunks to the peer.

`GET /local/events`

Server-sent events. First replays recent events, then streams new ones.

## Peer API

Frontends must not call `/peer/*`.

`GET /peer/handshake`

Returns public device information.

`POST /peer/transfer/prepare`

Accepts `WireTransferManifest`. Locks final and temporary receive paths during prepare.

`GET /peer/transfer/upload-status/:task_id`

Checks the locked receive index for one relative path.

`POST /peer/transfer/upload-chunk/:task_id`

Uploads a binary chunk. Requires exact offset, size limit, path membership, and final hash validation.

`POST /peer/transfer/touch/:task_id`

Creates an empty file from the locked receive index.

`POST /peer/clipboard/text/receive`

Receives text clipboard payload with hash and source device id.

`POST /peer/clipboard/image/prepare`

Returns a strong response:

```json
{
  "accepted": true,
  "reason": null,
  "offset": 0,
  "max_image_bytes": 20971520
}
```

If `accepted` is false, the sender must not upload chunks.

`POST /peer/clipboard/image/chunk`

Uploads PNG chunks with hash, source device id, final flag, and offset.

## Removed API

`/api/*` has been removed. New frontend, CLI, tests, and peer flows must not use it.
