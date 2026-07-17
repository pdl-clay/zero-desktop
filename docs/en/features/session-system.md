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
5. Groups the flat, filtered list into a forest via `build_session_tree` (see "Subagent session linking" below) and returns only the root sessions, each carrying its descendants under `children`.

**SessionInfo struct:**

```rust
pub struct SessionInfo {
    pub session_id: String,   // unique ID from zero
    pub title: String,        // zero-desktop's title (auto or user-set)
    pub created_at: String,   // ISO 8601 timestamp
    pub cwd: String,          // workspace directory
    pub model_id: String,     // overlaid from session-models.json
    pub event_count: Option<i64>,
    pub kind: String,         // "" | "fork" | "child" | "spec-draft" | "spec-impl"
    pub provider: String,     // e.g. "openai-compatible"
    pub parent_session_id: String, // set by the engine when this session was
                                    // spawned via `--calling-session-id` (Task
                                    // tool / swarm member) or fork/spec-impl
    pub root_session_id: String,   // the ultimate top-level ancestor
    pub agent_name: String,        // e.g. "advisor" for a Task-tool child
    pub tag: String,               // "specialist" for Task-tool/swarm children
    pub depth: i64,
    pub task_id: String,
    pub children: Vec<SessionInfo>, // NOT from the engine's JSON - populated
                                     // locally by build_session_tree; always a
                                     // Vec, never omitted
}
```

### Subagent session linking

Whenever the agent calls the `Task` tool (including an Advisor Mode
consultation, which is a `Task{name:"advisor",...}` call under the hood) or
spawns a swarm/team member, the zero engine creates a genuinely separate,
persisted session for it, sharing the same `cwd` as its parent — so it used
to show up in `zero sessions list --json`, and therefore in the sidebar, as
an indistinguishable extra top-level row.

The engine already tags these sessions (`kind: "child"`, `parentSessionId`,
`rootSessionId`, `agentName`, `tag: "specialist"`, `depth`) whenever a
`--calling-session-id` was involved in spawning them — `list_zero_sessions`
now captures those fields instead of discarding them, and
`build_session_tree` (private fn in `lib.rs`) groups the already-cwd-filtered
list into a forest: a session with no `parent_session_id`, or whose parent
isn't present in this same filtered list (different cwd, or already
deleted), becomes a root; everything else nests under its parent via
`children`, recursively. Nesting is driven purely by `parent_session_id`,
generic across every `SessionKind` (fork/child/spec-draft/spec-impl alike) —
no kind-specific cases.

**Engine-side fix required for full coverage**: Task-tool specialist
children (including Advisor) were already tagged correctly by
`internal/sessions.PrepareExec` in `my-zero`. Swarm/team members were not —
`internal/swarm/tools.go`'s `policyFrom` never threaded the orchestrator's
own session id through, so spawned members came back untagged
(`kind: ""`, no parent/root) despite going through the identical subprocess
mechanism. Fixed by threading `Policy.SessionID` →
`MemberSpec.ParentSessionID` → `specialist.TaskRunOptions.ParentSessionID` in
`internal/swarm/{team,member,tools,launcher_specialist}.go`, mirroring what
`internal/specialist/task_tool.go` already does for the `Task` tool.

**UI**: `src/components/SessionListItem.vue` is a recursive component
(Vue 3 SFCs can reference themselves by filename with no extra registration)
rendering one row per session plus, when `session.children.length > 0`, a
collapsed-by-default "N subagent sessions" toggle. Nested rows show an origin
caption (`agent_name`, falling back to `kind`). The five row actions
(select/rename/delete/status lookups) are provided by `MainLayout.vue` via
`provide("sessionListActions", {...})` and consumed with `inject(...)` at
every recursion depth, instead of prop-drilling. The sidebar's session count
(`workspace.sessions`) counts roots only.

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

### Store split (multi-session)

The session state is split across three stores (see [ADR 004](../architecture/decisions/004-multi-session-parallel.md)):

| Store                      | Type                               | Key state                                                                                                                                                                                                              |
| -------------------------- | ---------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `zero-store.js`            | Global singleton                   | `zeroPath`, `availableModels`, `activeModel`, `mcpBackends`                                                                                                                                                            |
| `zero-session-store.js`    | Factory `useZeroSessionStore(key)` | `sessionKey`, `sessionId`, `cwd`, `messages[]`, `currentResponse`, `currentThinking`, `currentPlan`, `sessionMode`, `runInProgress`, `isConnected`                                                                     |
| `session-runtime-store.js` | Global singleton                   | `openKeys[]`, `focusedKeyByPath{}` (per-workspace focus), `keyMeta{}` (badges, cwd, title per key). Panel cap (`MAX_OPEN_PANELS = 4`) is enforced **per workspace**, not globally — see `panelCountFor`/`canOpenMore`. |
| `workspaces-store.js`      | Global singleton                   | `workspaces[]`, `activePath`, `sessionsByPath{}`                                                                                                                                                                       |

### Actions (session store)

| Action                                       | Description                                                                                                                                                |
| -------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `startSession(cwd, sessionId?)`              | Calls `startZeroSession(key, cwd, sessionId)`, sets `this.sessionId` from the returned `StartedSession`, starts the 3s sync timer, syncs runtime metadata. |
| `openSession(sessionId)`                     | Calls `loadSessionHistory(sessionId)`, runs `buildMessagesFromHistory` to populate `this.messages`. Starts the 3s sync timer.                              |
| `sendMessage(content, file?)`                | Calls `sendZeroMessage(key, content, file)`, sets `runInProgress`, syncs runtime metadata.                                                                 |
| `cancelRun()`                                | Calls `cancelZeroRun(key)`.                                                                                                                                |
| `switchModel(model)`                         | Calls `switchZeroModel(key, model)` — restarts only this session (decision #6). Updates `globalStore.activeModel`.                                         |
| `stopSession()`                              | Calls `stopZeroSession(key)`, removes listeners, stops sync timer.                                                                                         |
| `respondToPermission(requestId, optionId)`   | Calls `respondToPermission` API, updates the permission message status.                                                                                    |
| `removeSession(sessionId, onRefresh)`        | Calls `deleteSession(sessionId)`, stops if active, calls `onRefresh`.                                                                                      |
| `renameSession(sessionId, title, onRefresh)` | Calls `renameSession(sessionId, title)`, calls `onRefresh`.                                                                                                |

### History replay (`buildMessagesFromHistory`)

The frontend normalizes persisted events into the same typed message shape used for live events:

| Persisted event type       | Produces                                            |
| -------------------------- | --------------------------------------------------- |
| `message` (role=user)      | `{ type: "text", role: "user", content, file? }`    |
| `message` (role=assistant) | `{ type: "text", role: "assistant", content }`      |
| `reasoning`                | `{ type: "thinking", content }`                     |
| `tool_call`                | `{ type: "tool_call", toolName, toolUseId, input }` |
| `tool_result`              | Updates matching `tool_call` status + result        |
| `permission_request`       | `{ type: "permission_request", answerable: false }` |
| `permission_decision`      | Updates matching `permission_request` status        |
| `error`                    | `{ type: "error", content }`                        |

Permission requests from history are always `answerable: false` — the process that asked is gone. If a matching `permission_decision` event exists, the request's status is updated to `approved` or `denied`; otherwise it renders as expired.

### Periodic sync (`_sessionSyncTimer`)

While a session is open (and no run is in progress), the store re-reads `load_sessionHistory` every 3 seconds. If the event count changed, it rebuilds the message list from scratch. This catches:

- New events written by the bridge during the current turn.
- External changes to the session from another zero-desktop instance.
- Late-arriving events that were written after the initial `openSession` completed.

The timer stops when the session is changed or closed.

## UI Components

### MainLayout.vue — Right Panel

The session list iterates `workspacesStore.sessionsByPath[activePath]` (not a
singleton store). Each item shows a live badge when the session is processing
(spinner for working, `!` badge for pending permission), derived from
`sessionRuntime.keyMeta`.

### Session Tile Grid

`SessionTileGrid.vue` replaces the single `<ChatView>` in the main content area.
It renders 1 (full), 2 (horizontal split), 3 (nested), or 4 (2×2 grid) panels
based on `sessionRuntime.visibleKeys(workspacesStore.activePath).length` — only
the panels belonging to the active workspace, not the app-wide `openKeys` list
— using Quasar's `QSplitter` for resizable dividers.

Each panel has a `SessionPaneHeader.vue` with a single close button, which calls
`runtime.closePanel(key)`. There is no separate manual "Stop" action anymore —
`closePanel` behaves conditionally: if a turn is actively running it only hides
the panel (the session keeps running in the background); if the session is
idle, it also stops and disposes it, freeing the workspace's panel slot.
`runtime.stopAndDispose(key)` still exists as an unconditional stop, but it is
only used when the user deletes the underlying session entirely (see
`MainLayout.vue`'s `onDeleteSession`), not from the panel's own close button.

### Session Click Flow

```
User clicks session item
  → onSelectSession(session)
    → openOrFocusSession(session.session_id, cwd, session.session_id)
      → runtime.openPanel(key)        — adds to openKeys, sets focusedKeyByPath[cwd]
      → store.startSession(cwd, id)   — Bridge: starts zero acp with session/load
                                        (or reattaches if already live)
```

### New Session Flow

```
User clicks "New session"
  → onNewSession()
    → key = crypto.randomUUID()
    → openOrFocusSession(key, cwd, null)
      → runtime.openPanel(key)
      → store.startSession(cwd, null)  — Bridge: starts zero acp with session/new
    → if same cwd already has a working session, shows non-blocking warning
```

## References

- [Connection Architecture](../architecture/connection.md)
- [ADR 004: Multi-Session Parallel Chat](../architecture/decisions/004-multi-session-parallel.md)
- [Workspace System](./workspace-system.md)
- [zero-bridge](./zero-bridge.md)
