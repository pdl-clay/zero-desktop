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

| Command                    | Description                                            |
| -------------------------- | ------------------------------------------------------ |
| `locate_zero_cli`          | Returns the path and version of the zero CLI.          |
| `start_zero_session`       | Starts `zero exec` in the given workspace directory.   |
| `send_zero_message`        | Sends a user message to the active session.            |
| `send_permission_decision` | Forwards a permission decision (approve/deny) to zero. |
| `stop_zero_session`        | Stops the active session.                              |

### Events

| Event         | Description                           |
| ------------- | ------------------------------------- |
| `zero:event`  | A stream-json output event from zero. |
| `zero:stderr` | A stderr line from the zero process.  |

### Dependencies added

- `tokio` — async runtime and process I/O.
- `which` — locate binaries on PATH.
- `dirs` — resolve platform-specific data directories.
- `thiserror` — error types.

## Frontend

### Files

- `src/services/zero.js` — wraps Tauri commands and event listeners.
- `src/stores/zero-store.js` — Pinia store for chat state with typed messages.
- `src/components/ChatView.vue` — main chat container with conditional rendering.
- `src/components/chat/TextMessage.vue` — user/assistant text messages.
- `src/components/chat/ThinkingBlock.vue` — collapsible model thinking display.
- `src/components/chat/ToolCallMessage.vue` — structured tool call card with states.
- `src/components/chat/PermissionRequest.vue` — permission card with approve/deny buttons.
- `src/pages/IndexPage.vue` — entry point that renders `ChatView`.

### Dependencies added

- `@tauri-apps/api` — Tauri frontend API for commands and events.

### Supported events

The store currently handles:

- `run_start`
- `reasoning` (streamed into collapsible thinking blocks)
- `text` (appended to the streaming response)
- `final`
- `run_end`
- `error`
- `tool_call` (rendered as structured cards with spinner/status)
- `tool_result` (updates the corresponding tool_call card inline)
- `permission_request` (rendered with approve/deny buttons, decision sent back to zero)

## Known limitations (alpha)

- Permission requests are now displayed with approve/deny buttons and forwarded back to zero. The decision flows through a persistent stdin channel to ensure zero processes it mid-turn.
- No multi-workspace tabbed interface yet (single active workspace per session).

## References

- [Architecture: Connection](../architecture/connection.md)
- [Zero Stream-JSON Protocol](https://github.com/Gitlawb/zero/blob/main/docs/STREAM_JSON_PROTOCOL.md)
