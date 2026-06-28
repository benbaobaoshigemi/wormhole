# Current Implementation Status

The project is now in the formal Rust prototype stage. The old all-in-one daemon entrypoint has been split into state, error, auth, DTO, API, service, and transport modules.

## Completed In This Pass

- Removed `/api/*` from product code.
- Added separate `/local/*` and `/peer/*` namespaces.
- Restricted Local API to loopback clients.
- Removed permissive CORS.
- Added peer token validation for peer write APIs.
- Stopped returning `shared_token` through state and settings DTOs.
- Split local manifest from wire manifest so peer manifests cannot contain `source_path`.
- Moved receive path resolution to prepare-time locked receive indexes.
- Added chunk size, offset, size, path membership, final size, and hash checks.
- Removed async progress spawn race on receive chunks.
- Added sender-side chunk progress events.
- Changed scan manifest to fast path scan without pre-hashing every file.
- Preserved retry as the same user-visible task id.
- Fixed cancelled marker cleanup on retry.
- Added non-Windows/macOS in-memory clipboard fallback for CI tests.
- Updated CLI and debug UI to `/local/*`.
- Added frontend contract docs and mock data.

## Frontend May Depend On

- `/local/state`
- `/local/settings`
- `/local/settings/update`
- `/local/connect`
- `/local/transfer/*`
- `/local/clipboard/*`
- `/local/events`
- DTO and mock files under `docs/frontend-contract/`

## Frontend Must Not Depend On

- Rust internal structs
- `/peer/*`
- removed `/api/*`
- debug UI HTML
- config files containing `shared_token`

## UI Rebuild Readiness

The backend/API/event boundary is stable enough for formal frontend reconstruction. The remaining work should be UI-only unless a new frontend need reveals an explicit missing local DTO field.

## Build And Test Results

Validated on Windows host:

- `cargo fmt --check`: passed.
- `cargo check --workspace`: passed.
- `cargo test --workspace`: passed.
- `C:/Users/zhang/miniconda3/python.exe .\_verification_scripts\backend_contract_validation.py`: passed.

Validated on macOS host `192.168.1.180` over SSH:

- `cargo build --release -p wormhole-daemon -p wormhole-cli`: passed.
- `cargo check --workspace`: passed.
- `cargo test --workspace`: passed.

macOS build currently emits warnings from the existing `objc` / `cocoa` clipboard adapter dependencies: unexpected `cargo-clippy` cfg checks, deprecated `cocoa::base::id` / `nil`, deprecated `NSString`, and a future-incompatibility notice for `block v0.1.6`. These warnings do not block the current prototype build, but they are recorded here so the future platform-adapter modernization work has a precise starting point.
