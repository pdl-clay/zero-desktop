# zero-bridge: Connection to the zero CLI

This document describes the initial implementation of the connection layer between the zero-desktop GUI and the zero CLI.

## Overview

The connection follows the architecture defined in [`docs/en/architecture/connection.md`](../architecture/connection.md):

- The Rust backend spawns `zero exec --input-format stream-json --output-format stream-json`.
- The frontend sends user messages via Tauri commands.
- The backend streams JSONL events back to the frontend via Tauri events.

## Rust Backend

### Files

- `src-tauri/src/locator.rs` — locates the `zero` binary on PATH or in the isolated cache.
- `src-tauri/src/bridge.rs` — manages the child process and parses stream-json events.
- `src-tauri/src/lib.rs` — registers Tauri commands and state.

### Commands

| Command | Description |
|---|---|
| `locate_zero_cli` | Returns the path and version of the zero CLI. |
| `start_zero_session` | Starts `zero exec` in the given workspace directory. |
| `send_zero_message` | Sends a user message to the active session. |
| `stop_zero_session` | Stops the active session. |

### Events

| Event | Description |
|---|---|
| `zero:event` | A stream-json output event from zero. |
| `zero:stderr` | A stderr line from the zero process. |

### Dependencies added

- `tokio` — async runtime and process I/O.
- `which` — locate binaries on PATH.
- `dirs` — resolve platform-specific data directories.
- `thiserror` — error types.

## Frontend

### Files

- `src/services/zero.js` — wraps Tauri commands and event listeners.
- `src/stores/zero-store.js` — Pinia store for chat state.
- `src/components/ChatView.vue` — basic chat UI.
- `src/pages/IndexPage.vue` — entry point that renders `ChatView`.

### Dependencies added

- `@tauri-apps/api` — Tauri frontend API for commands and events.

### Supported events

The store currently handles:

- `run_start`
- `text` (appended to the streaming response)
- `final`
- `run_end`
- `error`
- `tool_call`, `tool_result`, `permission_request` (displayed as event messages)

## Known limitations (alpha)

- Permission requests are displayed as raw events; interactive approval is not implemented yet.
- No multi-workspace tabbed interface yet (single active workspace per session).

## References

- [Architecture: Connection](../architecture/connection.md)
- [Zero Stream-JSON Protocol](https://github.com/Gitlawb/zero/blob/main/docs/STREAM_JSON_PROTOCOL.md)
