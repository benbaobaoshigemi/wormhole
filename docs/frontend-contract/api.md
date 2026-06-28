# Frontend API Contract

Use only `/local/*`.

Do not call `/peer/*`.

Do not call `/api/*`; it has been removed.

Do not read Rust source to infer behavior. Use this contract and the JSON mocks in this directory.

## Required Local Endpoints

- `GET /local/state`
- `GET /local/settings`
- `POST /local/settings/update`
- `POST /local/connect`
- `POST /local/transfer/send`
- `POST /local/transfer/cancel`
- `POST /local/transfer/retry`
- `GET /local/transfer/tasks`
- `GET /local/transfer/history`
- `POST /local/transfer/history/clear`
- `GET /local/clipboard/status`
- `POST /local/clipboard/enable`
- `POST /local/clipboard/disable`
- `POST /local/clipboard/system/read-send-text`
- `POST /local/clipboard/system/read-send-image`
- `GET /local/events`

## Main UI Fields

`/local/state` contains:

- `status`: connection status.
- `peer`: connected peer device or null.
- `settings`: public settings summary.
- `clipboard`: text/image enable state and image limit.
- `active_transfer_count`: number of active tasks.
- `recent_history_count`: number of recent history records.
- `tasks`: current transfer list.
- `events`: recent event replay.

No DTO contains `shared_token`, clipboard body text, PNG bytes, or sender source paths.
