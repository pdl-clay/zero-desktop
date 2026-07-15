# Session System

This document describes how zero-desktop lists, displays, resumes, and manages chat sessions from the zero CLI.

## Overview

Zero persists every conversation turn to disk at `~/.local/share/zero/sessions/<session-id>/`. Each session directory contains:

- `events.jsonl` — all events as JSONL (one JSON object per line). In ACP mode, zero only writes `message` entries here.
- `metadata.json` — session metadata.
- `session.lock` — concurrency lock.

zero-desktop maintains its **own** richer session history at `~/.local/share/zero-desktop/session-history/<sessionId>.jsonl`, which records messages, tool calls, reasoning, permission requests, and permission decisions — everything the app needs to faithfully replay a session. Two additional overlay files (`session-titles.json` and `session-models.json`) fill in data that ACP does not surface.

## Data Flow

```
┌─────────────────────────────┐
│  zero CLI                    │
│  ~/.local/share/zero/       │
│    sessions/<id>/            │
│      events.jsonl            │
└──────────┬──────────────────┘
            │ read by Rust (fallback)
┌──────────▼──────────────────┐
│  zero-desktop local data     │
│  ~/.local/share/             │
│    zero-desktop/             │
│      session-history/        │
│        <id>.jsonl (primary)  │
│      session-titles.json     │
│      session-models.json     │
└──────────┬──────────────────┘
            │ read by Rust
┌──────────▼──────────────────┐
│  Tauri Rust Backend          │
│  list_zero_sessions(cwd)     │
│    → zero sessions list --json + filter by cwd + overlay titles/models
│  load_session_history(id)    │
│    → prefer local log, fallback to zero's events.jsonl
│  delete_session(id)          │
│    → remove local log + title/model + zero's directory
│  rename_session(id, title)   │
│    → update session-titles.json
└──────────┬──────────────────┘
            │ Tauri IPC `invoke`
┌──────────▼──────────────────┐
│  Frontend (Pinia Store)     │
│  loadSessions(cwd)           │
│  openSession(sessionId)      │
│    → buildMessagesFromHistory(events)
│  _sessionSyncTimer (3s)      │
│    → periodic history re-read
└──────────┬──────────────────┘
            │ reactive bindings
┌──────────▼──────────────────┐
│  MainLayout.vue (drawer)    │
│  ChatView.vue (messages)    │
└─────────────────────────────┘
```

## Rust Backend

### `list_zero_sessions` (`lib.rs`)

```
Tauri command: list_zero_sessions(cwd: PathBuf) → Vec<SessionInfo>
```

1. Spawns `zero sessions list --json`.
2. Parses the JSON array into `Vec<SessionInfo>`.
3. Filters sessions where `session.cwd == <requested cwd>`.
4. Overlays zero-desktop's own titles (from `session-titles.json`) and model ids (from `session-models.json`), since ACP-created sessions get a generic "ACP session" title and an empty `modelId` from zero itself.
5. Returns the filtered and overlaid list.

**SessionInfo struct:**

```rust
pub struct SessionInfo {
    pub session_id: String,   // unique ID from zero
    pub title: String,        // zero-desktop's title (auto or user-set)
    pub created_at: String,   // ISO 8601 timestamp
    pub cwd: String,          // workspace directory
    pub model_id: String,     // overlaid from session-models.json
    pub event_count: Option<i64>,
    pub kind: String,         // "" | "fork" | "child"
    pub provider: String,     // e.g. "openai-compatible"
}
```

### `load_session_history` (`lib.rs`)

```
Tauri command: load_session_history(session_id: String) → Vec<SessionEvent>
```

1. First tries zero-desktop's own local log at `<data_dir>/zero-desktop/session-history/<sessionId>.jsonl`.
2. Falls back to zero's own `events.jsonl` at `<data_dir>/zero/sessions/<sessionId>/events.jsonl`.
3. Reads the file line by line, filtering for relevant event types: `message`, `reasoning`, `tool_call`, `tool_result`, `permission_request`, `permission_decision`, `error`.
4. Returns an array of `SessionEvent` with `type`, `payload` (untyped JSON), and `createdAt`.

The frontend's `buildMessagesFromHistory` normalizes these into typed messages (text, thinking, tool_call, permission_request, etc.) the same way live stream events are normalized.

### `delete_session` (`lib.rs`)

```
Tauri command: delete_session(session_id: String) → ()
```

1. Removes zero-desktop's local history file (`session-history/<id>.jsonl`).
2. Removes title and model overlay entries.
3. Removes zero's entire session directory (`<data_dir>/zero/sessions/<id>/`).
4. No error if already gone (idempotent).

### `rename_session` (`lib.rs`)

```
Tauri command: rename_session(session_id: String, title: String) → ()
```

Updates the entry in `session-titles.json`. Called automatically on the first message of a session (to derive a title from the message content), and on explicit user renames.

### Session resume

When `start_zero_session` is called with a `session_id`, the bridge opens the session via `session/load` (the ACP equivalent of `--resume`). If `session/load` fails (e.g. the session directory was deleted), it falls back to `session/new` — the conversation starts fresh rather than erroring out.

### Title derivation

On the first `send()` of a session, if no title is recorded yet:

- The first 60 characters of the user's message (collapsed whitespace) become the title.
- A file-only message (empty content) falls back to the file's name.
- The title is persisted in `session-titles.json`.

### Model snapshot

After every successful handshake (`session/new`, `session/load`, or fallback), the bridge snapshots the currently active model (from `zero config --json`) into `session-models.json`. This ensures the session list shows which model answered, even after the model is switched globally.

## Frontend

### `zero-store.js` — Session State

| State              | Type                                | Description                            |
| ------------------ | ----------------------------------- | -------------------------------------- |
| `currentSessionId` | `string \| null`                    | ID of the currently viewed session.    |
| `sessions`         | `Array`                             | Session list for the active workspace. |
| `messages`         | `Array<typed message>`              | Full message list displayed in `ChatView`. Includes text, thinking, tool_call, permission_request, permission_decision, error. |
| `currentWorkspace` | `string`                            | Active workspace path.                 |
| `isLoadingSession` | `boolean`                           | True while `openSession` is fetching history. |

### Actions

| Action | Description |
|---|---|
| `loadSessions(cwd)` | Calls `listZeroSessions(cwd)` and stores in `this.sessions`. Silently catches errors. |
| `openSession(sessionId)` | Calls `loadSessionHistory(sessionId)`, runs `buildMessagesFromHistory` to populate `this.messages` with typed message objects. Sets `this.currentSessionId` and starts the 3s sync timer. |
| `removeSession(sessionId)` | Calls `deleteSession(sessionId)`. If the deleted session was active, stops it first so the bridge stops writing to its directory. Resets state and refreshes the session list. |
| `renameSession(sessionId, title)` | Calls `renameSession(sessionId, title)` then refreshes the session list. |
| `startSession(cwd, sessionId?)` | Reconnects the bridge with optional session resume. Clears `messages`, sets `currentWorkspace` and `currentSessionId`. |

### History replay (`buildMessagesFromHistory`)

The frontend normalizes persisted events into the same typed message shape used for live events:

| Persisted event type   | Produces                                            |
| ----------------------- | --------------------------------------------------- |
| `message` (role=user)   | `{ type: "text", role: "user", content, file? }`    |
| `message` (role=assistant) | `{ type: "text", role: "assistant", content }`   |
| `reasoning`             | `{ type: "thinking", content }`                     |
| `tool_call`             | `{ type: "tool_call", toolName, toolUseId, input }` |
| `tool_result`           | Updates matching `tool_call` status + result        |
| `permission_request`    | `{ type: "permission_request", answerable: false }` |
| `permission_decision`   | Updates matching `permission_request` status        |
| `error`                 | `{ type: "error", content }`                        |

Permission requests from history are always `answerable: false` — the process that asked is gone. If a matching `permission_decision` event exists, the request's status is updated to `approved` or `denied`; otherwise it renders as expired.

### Periodic sync (`_sessionSyncTimer`)

While a session is open (and no run is in progress), the store re-reads `load_sessionHistory` every 3 seconds. If the event count changed, it rebuilds the message list from scratch. This catches:

- New events written by the bridge during the current turn.
- External changes to the session from another zero-desktop instance.
- Late-arriving events that were written after the initial `openSession` completed.

The timer stops when the session is changed or closed.

## UI Components

### MainLayout.vue — Right Panel

```
┌────────────────────────────┐
│  my-project                 │
│  ──────────────────────    │
│  Sessions (3)               │
│                            │
│  💬 oi                    │  ← title from first message
│     deepseek-v4  09/07     │
│                            │
│  💬 corrigir bug           │
│     deepseek-v4  08/07     │  ← model + date
│                            │
│  ⚡ add feature (fork)     │  ← fork icon differs
│     deepseek-v4  08/07     │
│                            │
└────────────────────────────┘
```

- **Session item:** `q-item` with `clickable` and `v-ripple`. Active session highlighted with `bg-primary-1`.
- **Icon:** `chat_bubble_outline` (default), `call_split` (fork), `subdirectory_arrow_right` (child).
- **Title:** Uses `session.title` (from zero-desktop's overlay) or falls back to the last 8 characters of `session.session_id`.
- **Subtitle:** `model_id` + formatted date (`DD/MM/YY HH:MM`).

### Session Click Flow

```
User clicks session item
  → onSelectSession(session)
    → zeroStore.startSession(cwd, session.session_id)
        → Bridge: starts zero acp with session/load
    → zeroStore.openSession(session.session_id)
        → loadSessionHistory(session_id)
        → buildMessagesFromHistory builds typed messages
        → ChatView re-renders with full conversation
    → zeroStore.loadSessions(cwd)
        → refresh session list
```

## References

- [Connection Architecture](../architecture/connection.md)
- [Workspace System](./workspace-system.md)
- [zero-bridge](./zero-bridge.md)