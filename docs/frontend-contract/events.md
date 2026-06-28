# Event Contract

Events are delivered through `GET /local/events` as server-sent events. The daemon sends all events as the default SSE `message`; the event name is inside the JSON `type` field. Frontend code should use `onmessage`, not per-event `addEventListener` handlers:

```js
const es = new EventSource("/local/events");
es.onmessage = (msg) => {
  const event = JSON.parse(msg.data);
  switch (event.type) {
    // handle event.data
  }
};
```

Each payload is a serialized `Event`:

```json
{
  "ts": "2026-06-28T10:00:00Z",
  "type": "transfer.progress",
  "data": {}
}
```

## Events

`connection.changed`

Fields: `status`, optional `peer`, optional `error_code`, optional `error`.

`transfer.scanning`

Fields: `status`, optional `task_id`, optional `file_count`, optional `total_size`.

`transfer.created`

Fields: `task`.

`transfer.queued`

Fields: `task`.

`transfer.started`

Fields: `task_id`, `direction`.

`transfer.progress`

Fields: `task_id`, `relative_path`, `direction`, `transferred_size`, `total_size`, `speed_bytes_per_sec`, `eta_seconds`.

`transfer.completed`

Fields: `task_id`, `direction`, optional `save_path`.

`transfer.failed`

Fields: `task_id`, `error_code`, optional `error`, optional `relative_path`.

`transfer.retrying`

Fields: `task_id`, `retry_count`.

`transfer.cancelled`

Fields: `task_id`.

`clipboard.synced`

Fields: `kind`, `hash`, optional `source_device_id`, optional `target`, optional `size`.

`clipboard.ignored`

Fields: `kind`, `hash`, `reason`.

`clipboard.too_large`

Fields: `kind`, `hash`, `size`.

`clipboard.failed`

Fields: `kind`, `hash`, `error_code`.

`settings.updated`

Fields: updated public settings or changed flags.

`daemon.error`

Fields: `error_code`, `error`.

Events never include UI copy. The frontend owns wording and localization.

Events never include sender source paths, `shared_token`, clipboard text, PNG bytes, temp paths, or internal receive indexes. `transfer.completed.save_path` is allowed only for the receiving side because it refers to the local save destination.
