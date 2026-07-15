# zero-bridge: Connection to the zero CLI

This document describes the connection layer between the zero-desktop GUI and the zero CLI.

## Overview

The connection follows the architecture defined in [`docs/en/architecture/connection.md`](../architecture/connection.md) and [ADR 003](../architecture/decisions/003-migrate-to-acp.md):

- The Rust backend spawns `zero acp` (Agent Client Protocol over stdio) - one process per active session, kept alive for the whole conversation.
- The frontend sends user messages (with optional file attachments) via Tauri commands.
- The backend translates ACP's `session/update` notifications into the same event shape the frontend already renders, and streams them back via Tauri events.
- Permission requests from the agent (`session/request_permission`) are forwarded to the frontend and answered for real over the same JSON-RPC connection.
- The backend also proxies several read-only zero CLI queries (sessions, models, MCP backends/tools) that are independent of any live session.

## Rust Backend

### Files

- `src-tauri/src/locator.rs` — locates the `zero` binary on PATH or in the isolated cache.
- `src-tauri/src/acp.rs` — minimal hand-rolled JSON-RPC 2.0 peer for the Agent Client Protocol (sends requests, receives requests, receives notifications - not a client-only or server-only implementation).
- `src-tauri/src/bridge.rs` — `ZeroBridge`: owns the per-session `zero acp` process, translates ACP events into the app's internal event shape, writes the local session-history log, and manages session title/model overlays.
- `src-tauri/src/mcp_cache.rs` — persistent on-disk cache of MCP backend health-check statuses, so the drawer renders immediately with last-known data before live checks complete.
- `src-tauri/src/lib.rs` — registers Tauri commands and all state types (`SessionInfo`, `SessionEvent`, `FileAttachment`, `McpBackendInfo`, `McpCheckResult`, `McpToolInfo`).

### Commands

#### Session commands (via `zero acp`)

| Command                 | Description                                                                                                                                           |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `locate_zero_cli`       | Returns the path and version of the zero CLI.                                                                                                         |
| `start_zero_session`    | Spawns `zero acp` for the given workspace and opens (or loads) a session.                                                                             |
| `send_zero_message`     | Sends a `session/prompt`, optionally with a file attachment, streaming progress back via events.                                                      |
| `respond_to_permission` | Answers a pending `session/request_permission` with a chosen option.                                                                                  |
| `cancel_zero_run`       | Kills the current session's process (no `session/cancel` method exists). Session id and history are preserved; next `send()` respawns and reattaches. |
| `stop_zero_session`     | Stops the active session and tears down all state.                                                                                                    |

#### Session management commands (via zero CLI)

| Command                | Description                                                                                                                                                                                                                                                                    |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `list_zero_sessions`   | Lists sessions for a workspace (`zero sessions list --json`, filtered by `cwd`). Overlays zero-desktop's own titles and model ids.                                                                                                                                             |
| `load_session_history` | Loads a session's rich history — prefers zero-desktop's own local log (`session-history/<id>.jsonl`), falls back to zero's own `events.jsonl`. Returns typed events: `message`, `reasoning`, `tool_call`, `tool_result`, `permission_request`, `permission_decision`, `error`. |
| `delete_session`       | Deletes a session's data: zero-desktop's local history file, title/model overlays, and zero's own session directory.                                                                                                                                                           |
| `rename_session`       | Sets (or overwrites) a session's title in zero-desktop's local title map. Used for both auto-derived titles on first message and explicit user renames.                                                                                                                        |

#### File commands

| Command                | Description                                                                                                                                                                                                       |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `read_file_attachment` | Reads a file from disk (up to 10 MB), validates the extension, detects image vs. text, rejects binary in text files, and returns it base64-encoded with its MIME type. Used before attaching a file to a message. |

#### Model commands

| Command             | Description                                                                                                                                                                               |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `list_zero_models`  | Probes the active provider's model-listing endpoint via `zero providers models --json` and returns the full list plus which model is currently active. Not instant — a real network call. |
| `switch_zero_model` | Updates the active provider's model globally via `zero providers add --model <x> --set-active`, then kills the live session process so the next message picks up the change.              |

#### MCP commands

| Command                    | Description                                                                                                                               |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `list_mcp_backends`        | Reads configured MCP servers from zero's config (`zero backends --json`) and overlays cached health statuses.                             |
| `check_mcp_backend`        | Live-checks a single MCP server (`zero mcp check --json`): connects, lists tools, reports status. Persists the result to the local cache. |
| `check_mcp_backend_cached` | Returns the cached status for a server if present; falls back to a live check otherwise.                                                  |
| `load_mcp_status_cache`    | Reads the raw MCP status cache from disk for fast initial rendering.                                                                      |
| `list_mcp_tools`           | Lists all tools exposed by enabled MCP servers (`zero mcp tools list --json`). Returns `{ name, description }` for each tool.             |

### Events

| Event                     | Description                                                                                                 |
| ------------------------- | ----------------------------------------------------------------------------------------------------------- |
| `zero:event`              | A translated ACP event: `text`, `reasoning`, `tool_call`, `tool_result`, `plan_update`, `run_end`, `error`. |
| `zero:permission-request` | A real permission request from the agent, awaiting a reply via `respond_to_permission`.                     |
| `zero:stderr`             | A stderr line from the zero process (or an unparseable stdout line, logged for visibility).                 |
| `zero:process-exited`     | The session's process's stdout stream closed.                                                               |

#### Event types within `zero:event`

| Type          | Description                                                                      |
| ------------- | -------------------------------------------------------------------------------- |
| `text`        | Streaming assistant reply delta (`{ delta: string }`).                           |
| `reasoning`   | Streaming agent thought chunk (`{ delta: string }`).                             |
| `tool_call`   | Agent started a tool call (`{ id, name, args }`).                                |
| `tool_result` | Tool call completed or failed (`{ id, status: "ok"                               | "error", output }`). |
| `plan_update` | Agent's plan checklist updated (`{ entries: [{ content, status, priority }] }`). |
| `run_end`     | Turn finished (`{ status, stopReason }`).                                        |
| `error`       | Fatal error from the bridge (`{ message }`).                                     |

### Dependencies

- `tokio` — async runtime and process I/O.
- `which` — locate binaries on PATH.
- `dirs` — resolve platform-specific data directories (also used for all local caches and history).
- `thiserror` — error types.
- `base64` — encode/decode file attachments.

No JSON-RPC crate was added - `acp.rs` hand-rolls the newline-delimited framing directly on top of `tokio` + `serde_json`, since ACP requires acting as both a request-sender and a request-receiver on the same connection, which most JSON-RPC crates don't cleanly support.

## Frontend

### Files

- `src/services/zero.js` — wraps every Tauri command and event listener.
- `src/stores/zero-store.js` — Pinia store for chat state, session management, model list, MCP backends, permission mode, plan state, and session sync.
- `src/components/ChatView.vue` — main chat container with conditional rendering.
- `src/components/chat/ChatInput.vue` — message input with attach-file button, permission-mode toggle, model picker dropdown, inline plan checklist, working-status indicator, and cancel button.
- `src/components/chat/TextMessage.vue` — user/assistant text messages (markdown-rendered).
- `src/components/chat/ThinkingBlock.vue` — collapsible model thinking display.
- `src/components/chat/ToolCallMessage.vue` — structured tool call card with running/completed/error states, a real diff view for `edit_file`, and a checklist view for `update_plan`.
- `src/components/chat/PendingPermissionPanel.vue` — pinned above the input while a permission request is pending; renders whatever options ACP actually offered (not a fixed approve/deny pair).
- `src/components/chat/PermissionDecisionBadge.vue` — inline badge for informational auto-decisions and resolved permission requests in history.
- `src/components/chat/ErrorMessage.vue` — inline error bubble (e.g. lost connection).
- `src/components/chat/PlanPanel.vue` — standalone panel rendering the current plan; also embedded inline in `ChatInput.vue`.
- `src/components/McpDrawer.vue` — right-side panel: MCP backend cards with live health status, edited-files strip with inline diff previews.
- `src/pages/IndexPage.vue` — entry point that renders `ChatView`.

### Dependencies

- `@tauri-apps/api` — Tauri frontend API for commands and events.
- `pinia` — state management.
- `vue-i18n` — internationalization.

### Supported events

The store handles, via `zero:event`:

- `text` (appended to the streaming response)
- `reasoning` (streamed into collapsible thinking blocks)
- `tool_call` / `tool_result` (rendered as structured cards with spinner/status; `update_plan` calls are tracked separately and pinned above the input instead of appearing as a card)
- `plan_update` (replaces `currentPlan` in the store; rendered inline in `ChatInput.vue`)
- `run_end`
- `error`

And, via the dedicated `zero:permission-request` event, a real permission ask that `respondToPermission` answers.

### Store architecture

The Pinia store (`zero-store.js`) manages:

- `messages[]` — typed message list (text, thinking, tool_call, permission_request, permission_decision, error).
- `currentResponse` / `currentThinking` — streaming buffers finalized into permanent messages on the next event boundary.
- `currentPlan` — the agent's current plan checklist (replaced whole on each `plan_update`).
- `activePlan` getter — returns `null` when all items are completed, so the plan panel auto-hides.
- `editedFiles` getter — groups `edit_file`/`write_file` tool calls by file path, preserving encounter order.
- `workingStatus` getter — returns `thinking`, `{ type: "tool", toolName }`, `writing`, `sending`, or `null`.
- `availableModels` / `activeModel` — populated by `list_zero_models` (network call to provider).
- `mcpBackends` / `mcpTools` — populated by `list_mcp_backends` + `list_mcp_tools`, with cached status overlay.
- `permissionMode` — `"ask"` (default) or `"auto_allow"` (auto-approves permission requests).
- `_sessionSyncTimer` — periodic (3s) history re-read while a session is open, catching external changes.

## Known limitations (alpha)

- No `session/cancel` in the underlying protocol: cancelling a turn kills that session's process; the next message respawns it and reattaches via `session/load`.
- Network access (e.g. `web_fetch`) gets denied by zero's own sandbox regardless of the permission answered - a hard limit of the current sandbox policy, not something this bridge controls.
- No multi-workspace tabbed interface yet (single active workspace per session).

## References

- [Architecture: Connection](../architecture/connection.md)
- [ADR 003: Migrate to ACP](../architecture/decisions/003-migrate-to-acp.md)
- [Agent Client Protocol](https://agentclientprotocol.com)
