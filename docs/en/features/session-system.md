# Session System

This document describes how zero-desktop lists, displays, and resumes chat sessions from the zero CLI.

## Overview

Zero persists every conversation turn to disk at `~/.local/share/zero/sessions/<session-id>/`. Each session directory contains:

- `events.jsonl` — all events (messages, tool calls, usage stats) as JSONL (one JSON object per line).
- `metadata.json` — session metadata.
- `session.lock` — concurrency lock.

zero-desktop consumes this data to:

- List sessions scoped to the active workspace (`zero sessions list --json`, filtered by `cwd`).
- Load full message history from `events.jsonl` when a session is clicked.
- Resume sessions via `zero exec --resume <sessionId>` so the model retains conversation context.

## Data Flow

```
┌─────────────────────────────┐
│  zero CLI                    │
│  ~/.local/share/zero/       │
│    sessions/<id>/            │
│      events.jsonl            │
└──────────┬──────────────────┘
           │ read by Rust
┌──────────▼──────────────────┐
│  Tauri Rust Backend          │
│  list_zero_sessions(cwd)     │
│    → zero sessions list --json + filter by cwd
│  load_session_history(id)    │
│    → read events.jsonl, parse message events
└──────────┬──────────────────┘
           │ Tauri IPC `invoke`
┌──────────▼──────────────────┐
│  Frontend (Pinia Store)     │
│  loadSessions(cwd)           │
│  openSession(sessionId)      │
│  sessions[] state            │
│  messages[] state            │
└──────────┬──────────────────┘
           │ reactive bindings
┌──────────▼──────────────────┐
│  MainLayout.vue (drawer)    │
│  ChatView.vue (messages)    │
└─────────────────────────────┘
```

## Rust Backend

### `list_zero_sessions` (`lib.rs:28`)

```
Tauri command: list_zero_sessions(cwd: PathBuf) → Vec<SessionInfo>
```

1. Spawns `zero sessions list --json`.
2. Parses the JSON array into `Vec<SessionInfo>`.
3. Filters sessions where `session.cwd == <requested cwd>`.
4. Returns the filtered list.

**SessionInfo struct:**

```rust
#[derive(Serialize)]
pub struct SessionInfo {
    pub session_id: String,   // unique ID from zero
    pub title: String,        // first user message or empty
    pub created_at: String,   // ISO 8601 timestamp
    pub cwd: String,          // workspace directory
    pub model_id: String,     // e.g. "deepseek-v4-flash"
    pub event_count: Option<i64>,
    pub kind: String,         // "" | "fork" | "child"
    pub provider: String,     // e.g. "openai-compatible"
}
```

**Serialization note:** The struct uses `#[serde(alias = "sessionId")]` (not `rename`) so zero's camelCase JSON is deserialized correctly, but the response to the frontend uses snake_case (`session_id`, `created_at`, `model_id`).

### `load_session_history` (`lib.rs:39`)

```
Tauri command: load_session_history(session_id: String) → Vec<ChatMessage>
```

1. Resolves the session directory: `<data_dir>/zero/sessions/<session_id>/events.jsonl`.
2. Reads the file line by line.
3. Filters for events where `type == "message"`.
4. Extracts `payload.role`, `payload.content`, and `createdAt`.
5. Returns an array of `ChatMessage`.

**ChatMessage struct:**

```rust
#[derive(Serialize)]
pub struct ChatMessage {
    pub role: String,       // "user" | "assistant"
    pub content: String,    // message text
    pub timestamp: String,  // ISO 8601
}
```

### `delete_session` (`lib.rs:79`)

```
Tauri command: delete_session(session_id: String) → ()
```

1. Resolves `<data_dir>/zero/sessions/<session_id>/`.
2. Removes the entire directory with `std::fs::remove_dir_all`.
3. No error if already gone (idempotent via `exists()` check).

When `start_zero_session` is called with a `session_id`, the bridge stores it:

```rust
state.start(cwd, Some(session_id)).await
```

On the first `send()`, instead of spawning a plain `zero exec`, the bridge adds `--resume <sessionId>`:

```rust
if let Some(ref id) = resume_id {
    cmd.arg("--resume").arg(id);
}
```

This causes zero to load the existing session context, so the model remembers the conversation history. The stdout reader still captures the `sessionId` from the `run_start` event for subsequent turns.

## Frontend

### `zero-store.js` — Session State

| State              | Type                                | Description                            |
| ------------------ | ----------------------------------- | -------------------------------------- |
| `currentSessionId` | `string \| null`                    | ID of the currently viewed session.    |
| `sessions`         | `Array`                             | Session list for the active workspace. |
| `messages`         | `Array<{role, content, timestamp}>` | Chat messages displayed in `ChatView`. |
| `currentWorkspace` | `string`                            | Active workspace path.                 |

### Actions

| Action | Description |
|---|---|---|
| `loadSessions(cwd)` | Calls `listZeroSessions(cwd)` and stores in `this.sessions`. Silently catches errors. |
| `openSession(sessionId)` | Calls `loadSessionHistory(sessionId)`, maps the response to `{role, content, timestamp}` objects, and sets `this.messages`. Sets `this.currentSessionId`. |
| `removeSession(sessionId)` | Calls `deleteSession(sessionId)` (Rust removes the session directory from disk), resets `currentSessionId` and messages if the deleted session was active, then refreshes the session list. |
| `startSession(cwd, sessionId?)` | Reconnects the bridge with optional session resume. Clears `messages`, sets `currentWorkspace` and `currentSessionId`. |

### Auto-Refresh

When a `run_end` event arrives (after zero finishes processing a message), the store automatically refreshes the session list:

```javascript
case 'run_end':
  // ... handle streaming response ...
  if (this.currentWorkspace) {
    this.loadSessions(this.currentWorkspace)
  }
  break
```

This ensures that newly created sessions (from the current chat) appear in the drawer immediately.

## UI Components

### MainLayout.vue — Right Panel

```
┌────────────────────────────┐
│  my-project                 │
│  ──────────────────────    │
│  Sessões (3)               │
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
- **Title:** Uses `session.title` (from the first user message) or falls back to the last 8 characters of `session.session_id`.
- **Subtitle:** `model_id` + formatted date (`DD/MM/YY HH:MM`).

### Session Click Flow

```
User clicks session item
  → onSelectSession(session)
    → zeroStore.startSession(cwd, session.session_id)
        → Bridge: stores resume_id, will use --resume on next send()
    → zeroStore.openSession(session.session_id)
        → loadSessionHistory(session_id)
        → history messages populate this.messages
        → ChatView re-renders with full conversation
    → zeroStore.loadSessions(cwd)
        → refresh session list (e.g., after external changes)
```

### Chat Message Display

Messages loaded from history use the same `q-chat-message` component as live messages:

| Role        | Name     | Background                               |
| ----------- | -------- | ---------------------------------------- |
| `user`      | "Você"   | `primary` (blue)                         |
| `assistant` | "Zero"   | `grey-3` (light) or `grey-9` (dark mode) |
| `system`    | "system" | `info`                                   |
| `event`     | "event"  | `warning`                                |

Dark mode adapts chat bubble colors automatically via `$q.dark.isActive`.

## Testing

Integration tests verify the session system end-to-end:

| Test                                              | File                        | Verifies                                                                                                                                                                                                            |
| ------------------------------------------------- | --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `test_sessions_list_filters_by_cwd`               | `tests/zero_integration.rs` | Creates a session in a temp dir, runs `zero sessions list --json`, asserts the session appears filtered by cwd.                                                                                                     |
| `test_session_info_fields`                        | `tests/zero_integration.rs` | Asserts `sessionId`, `createdAt`, `modelId`, and `cwd` fields are present and correct.                                                                                                                              |
| `test_delete_session_removes_from_list`           | `tests/zero_integration.rs` | Creates a session, verifies it exists on disk and in the session list, deletes it via `remove_dir_all`, verifies it no longer appears in the list.                                                                  |
| `test_message_history_recovery_from_events_jsonl` | `tests/zero_integration.rs` | Creates a session with a known message, reads `events.jsonl` from disk, verifies user + assistant messages are present with correct roles, and checks required fields (`id`, `sessionId`, `createdAt`, `sequence`). |
| `test_multi_turn_context_preserved_with_resume`   | `tests/zero_integration.rs` | Turn 1 sets context ("name is Alice"), turn 2 resumes via `--resume <id>` and asks "What is my name?" — asserts "Alice" appears.                                                                                    |

## References

- [Zero Stream-JSON Protocol](https://github.com/Gitlawb/zero/blob/main/docs/STREAM_JSON_PROTOCOL.md)
- [Connection Architecture](../architecture/connection.md)
- [Workspace System](./workspace-system.md)
