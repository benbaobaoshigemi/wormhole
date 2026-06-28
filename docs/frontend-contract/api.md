# Frontend API Contract

Use only `/local/*`.

Do not call `/peer/*`.

Do not call `/api/*`; it has been removed.

Do not read Rust source, SQLite rows, config files, or debug UI code to infer behavior. Use this contract and the JSON mocks in this directory.

`GET /local/state` is the only supported UI startup and recovery entrypoint. Use it when the formal frontend starts, reconnects to the daemon, resumes after sleep, or needs to reconcile missed events.

`GET /local/events` is the incremental update stream. Treat it as a delta stream layered on top of `/local/state`, not as the only source of truth.

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

## Retry Request

`POST /local/transfer/retry` supports an explicit task id:

```json
{ "task_id": "task-20260628-001" }
```

If the body is empty, the daemon retries the most recent failed task. The formal frontend should pass `task_id` so the user action maps to the visible task row.


## Event Stream Consumption

`GET /local/events` uses default SSE `message` events. The frontend should consume it with:

```js
const es = new EventSource("/local/events");
es.onmessage = (msg) => {
  const event = JSON.parse(msg.data);
};
```

Do not require `addEventListener("transfer.progress", ...)`. The event type is `event.type` inside the JSON payload.
