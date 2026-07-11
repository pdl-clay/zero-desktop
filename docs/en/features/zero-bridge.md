# zero-bridge: Connection to the zero CLI

This document describes the connection layer between the zero-desktop GUI and the zero CLI.

## Overview

The connection follows the architecture defined in [`docs/en/architecture/connection.md`](../architecture/connection.md) and [ADR 003](../architecture/decisions/003-migrate-to-acp.md):

- The Rust backend spawns `zero acp` (Agent Client Protocol over stdio) - one process per active session, kept alive for the whole conversation.
- The frontend sends user messages via Tauri commands.
- The backend translates ACP's `session/update` notifications into the same event shape the frontend already renders, and streams them back via Tauri events.
- Permission requests from the agent (`session/request_permission`) are forwarded to the frontend and answered for real over the same JSON-RPC connection.

## Rust Backend

### Files

- `src-tauri/src/locator.rs` — locates the `zero` binary on PATH or in the isolated cache.
- `src-tauri/src/acp.rs` — minimal hand-rolled JSON-RPC 2.0 peer for the Agent Client Protocol (sends requests, receives requests, receives notifications - not a client-only or server-only implementation).
- `src-tauri/src/bridge.rs` — `ZeroBridge`: owns the per-session `zero acp` process, translates ACP events into the app's internal event shape, and writes the local session-history log.
- `src-tauri/src/lib.rs` — registers Tauri commands and state.

### Commands

| Command                 | Description                                                                                                |
| ----------------------- | ---------------------------------------------------------------------------------------------------------- |
| `locate_zero_cli`       | Returns the path and version of the zero CLI.                                                              |
| `start_zero_session`    | Spawns `zero acp` for the given workspace and opens (or loads) a session.                                  |
| `send_zero_message`     | Sends a `session/prompt`, streaming progress back via events.                                              |
| `respond_to_permission` | Answers a pending `session/request_permission` with a chosen option.                                       |
| `cancel_zero_run`       | Kills the current session's process (no `session/cancel` method exists).                                   |
| `stop_zero_session`     | Stops the active session.                                                                                  |
| `list_zero_sessions`    | Lists sessions for a workspace (`zero sessions list --json`).                                              |
| `load_session_history`  | Loads a session's history - prefers zero-desktop's own local log, falls back to zero's own `events.jsonl`. |
| `delete_session`        | Deletes a session's data, including zero-desktop's local history file.                                     |

### Events

| Event                     | Description                                                                                   |
| ------------------------- | --------------------------------------------------------------------------------------------- |
| `zero:event`              | A translated ACP event (`text`, `reasoning`, `tool_call`, `tool_result`, `run_end`, `error`). |
| `zero:permission-request` | A real permission request from the agent, awaiting a reply via `respond_to_permission`.       |
| `zero:stderr`             | A stderr line from the zero process (or an unparseable stdout line, logged for visibility).   |
| `zero:process-exited`     | The session's process's stdout stream closed.                                                 |

### Dependencies

- `tokio` — async runtime and process I/O.
- `which` — locate binaries on PATH.
- `dirs` — resolve platform-specific data directories (also used for the local session-history log).
- `thiserror` — error types.

No JSON-RPC crate was added - `acp.rs` hand-rolls the newline-delimited framing directly on top of `tokio` + `serde_json`, since ACP requires acting as both a request-sender and a request-receiver on the same connection, which most JSON-RPC crates don't cleanly support.

## Frontend

### Files

- `src/services/zero.js` — wraps Tauri commands and event listeners.
- `src/stores/zero-store.js` — Pinia store for chat state with typed messages.
- `src/components/ChatView.vue` — main chat container with conditional rendering.
- `src/components/chat/ChatInput.vue` — message input, working-status indicator, and pinned plan checklist.
- `src/components/chat/TextMessage.vue` — user/assistant text messages (markdown-rendered).
- `src/components/chat/ThinkingBlock.vue` — collapsible model thinking display.
- `src/components/chat/ToolCallMessage.vue` — structured tool call card with running/completed/error states, a real diff view for `edit_file`, and a checklist view for `update_plan`.
- `src/components/chat/PendingPermissionPanel.vue` — pinned above the input while a permission request is pending; renders whatever options ACP actually offered (not a fixed approve/deny pair).
- `src/components/chat/PermissionDecisionBadge.vue` — inline badge for informational auto-decisions and resolved permission requests in history.
- `src/components/chat/ErrorMessage.vue` — inline error bubble (e.g. lost connection).
- `src/pages/IndexPage.vue` — entry point that renders `ChatView`.

### Dependencies

- `@tauri-apps/api` — Tauri frontend API for commands and events.

### Supported events

The store currently handles, via `zero:event`:

- `text` (appended to the streaming response)
- `reasoning` (streamed into collapsible thinking blocks)
- `tool_call` / `tool_result` (rendered as structured cards with spinner/status; `update_plan` calls are tracked separately and pinned above the input instead of appearing as a card)
- `run_end`
- `error`

And, via the dedicated `zero:permission-request` event, a real permission ask that `respondToPermission` answers.

## Known limitations (alpha)

- No `session/cancel` in the underlying protocol: cancelling a turn kills that session's process; the next message respawns it and reattaches via `session/load`.
- Network access (e.g. `web_fetch`) gets denied by zero's own sandbox regardless of the permission answered - a hard limit of the current sandbox policy, not something this bridge controls.
- No multi-workspace tabbed interface yet (single active workspace per session).

## References

- [Architecture: Connection](../architecture/connection.md)
- [ADR 003: Migrate to ACP](../architecture/decisions/003-migrate-to-acp.md)
- [Agent Client Protocol](https://agentclientprotocol.com)
