# Workspace System

This document describes how zero-desktop manages project directories (workspaces).

## Overview

Workspaces are directories on the user's filesystem where the zero coding agent operates. Each workspace is a project folder — zero reads, writes, and executes commands within that directory.

The workspace system provides:

- Persistent workspace list across app restarts via `localStorage`.
- Native folder picker to add new workspaces.
- Visual avatars with deterministic colors based on the directory name.
- Automatic connection to zero when a workspace is selected.
- Workspace-scoped session listing (sessions are filtered by `cwd`).

## Data Model

### `workspaces-store.js` (Pinia)

**File:** `src/stores/workspaces-store.js`

| State        | Type                             | Description                               |
| ------------ | -------------------------------- | ----------------------------------------- |
| `workspaces` | `Array<{ path, name, addedAt }>` | All saved workspaces.                     |
| `activePath` | `string \| null`                 | Path of the currently selected workspace. |

| Action         | Description                                                                                                                                           |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `add(path)`    | Normalizes the path, extracts the directory name, deduplicates, pushes to array, saves to `localStorage`. If no workspace is active, auto-selects it. |
| `remove(path)` | Filters out the path, saves to `localStorage`. If the removed workspace was active, selects the first remaining one (or sets to `null`).              |
| `select(path)` | Sets `activePath`.                                                                                                                                    |

| Getter      | Description                                                   |
| ----------- | ------------------------------------------------------------- |
| `active`    | Returns the full workspace object for `activePath` or `null`. |
| `hasActive` | `true` if a workspace is selected.                            |

### Persistence

Workspaces are stored in `localStorage` under the key `zero-desktop-workspaces`. The JSON format:

```json
[
  {
    "path": "/home/user/my-project",
    "name": "my-project",
    "addedAt": 1752019200000
  }
]
```

Persistence calls:

- `loadWorkspaces()` — called at store creation (synchronous, blocks nothing).
- `saveWorkspaces()` — called after every `add()` and `remove()`.

## UI Components

### MainLayout.vue — Drawer (left column)

```
┌──────┐
│  🎯  │  zero logo
│ ──── │
│ [M]╳ │  my-project (active)
│ [T]╳ │  test
│  [+] │  add workspace (opens native folder picker)
│ ──── │
│  ⚙   │  settings
│  ☀   │  dark/light toggle
└──────┘
```

- **Avatar:** A circular `<div>` styled with `border-radius: 50%`. Background color is derived from a hash of the directory name (10 predefined colors). The letter is the first character of the directory name, uppercase. Active workspace gets a larger size (40px vs 34px) and a double ring `box-shadow`.
- **Remove button:** A small `X` icon (`q-btn round size="xs"`) positioned at the bottom-right corner of the avatar. Hidden by default (`opacity: 0; transform: scale(0.4)`), shown on hover with a scale animation.
- **Add button:** A `+` icon (`q-btn round icon="add"`) that opens the native folder picker directly — no dialog, no confirmation. Click → native file explorer opens → select folder → workspace added.
- **Tooltip:** Each avatar has a tooltip showing the directory name (bold) and full path.
- **Dark mode:** The left column background adapts via `:class="$q.dark.isActive ? 'bg-dark' : 'bg-grey-3'"`.

### Adding a Workspace

Flow:

1. User clicks the `+` button.
2. `onBrowseAndAdd()` calls `open({ directory: true, multiple: false })` from `@tauri-apps/plugin-dialog`.
3. The native OS folder picker opens (GTK on Linux, Finder on macOS, Explorer on Windows).
4. If a folder is selected, `workspacesStore.add(selectedPath)` is called immediately.
5. The store normalizes the path, deduplicates, saves to `localStorage`, and auto-selects if it's the first workspace.

### Workspace Selection

Since multi-session parallel chat (see [ADR 004](../architecture/decisions/004-multi-session-parallel.md)), switching workspaces no longer kills or starts any session — it is pure navigation. A `watch` on `workspacesStore.activePath` in `MainLayout.vue` only refreshes the session list for the newly active workspace:

```
watch(activePath)
  → workspacesStore.loadSessions(newPath)  // refresh session list for the new workspace
```

Panels belonging to other workspaces keep running in the background (up to the per-workspace cap of `MAX_OPEN_PANELS`); switching workspaces just changes which panels `SessionTileGrid.vue` renders, via `sessionRuntime.visibleKeys(workspacePath)`.

Selecting a workspace also opens or focuses a session for it, via `onSelectWorkspace()`:

1. `workspacesStore.select(ws.path)` sets `activePath`.
2. If no panel is already open for that workspace's `cwd`, a new key is generated and `openOrFocusSession(key, ws.path, null)` opens a panel for it.
3. Opening a panel only prepares it (loads history if resuming); the real `zero acp` process is spawned lazily, the first time the user sends a message (see [Session System](./session-system.md)).

## Rust Backend

### Tauri Commands

| Command                                     | File     | Description                                                                                        |
| ------------------------------------------- | -------- | -------------------------------------------------------------------------------------------------- |
| `locate_zero_cli`                           | `lib.rs` | Finds zero binary and returns path + version.                                                      |
| `start_zero_session(key, cwd, session_id?)` | `lib.rs` | Spawns `zero acp` for the given routing key + workspace. Optional `session_id` for `session/load`. |
| `send_zero_message(key, content, file?)`    | `lib.rs` | Sends a user message (with optional file attachment) to the session identified by `key`.           |
| `stop_zero_session(key)`                    | `lib.rs` | Kills the process for `key` and clears its session state.                                          |
| `cancel_zero_run(key)`                      | `lib.rs` | Kills the process for `key` but preserves session id/history for reattach.                         |
| `switch_zero_model(key, model)`             | `lib.rs` | Updates the active model globally, then kills only the session identified by `key`.                |
| `list_zero_sessions(cwd)`                   | `lib.rs` | Lists sessions filtered by workspace directory.                                                    |

### Bridge State (`bridge.rs`)

`ZeroBridge.sessions` is a `HashMap<String, AcpSession>` keyed by a frontend-owned
routing key (UUID for new sessions, `session_id` for resumed ones) — not by
workspace. A single workspace can have several `AcpSession`s live at once (one
per open panel), and each is independent of the others:

```rust
struct AcpSession {
    cwd: PathBuf,             // workspace directory
    session_id: String,       // captured from session/new or session/load response
    history_path: PathBuf,    // zero-desktop's local history file for this session
    live: Option<LiveProcess>, // the running zero acp child process + AcpPeer
}
```

- `start(key, cwd, resume_id)` — spawns a new `zero acp` for this key, completes the `initialize` handshake, and opens the session (`session/new` or `session/load`). Does not touch any other key's session.
- `send(key, content, file?)` — persists the user message to the local history, then fires `session/prompt` in a background task. Returns immediately; progress streams via `zero:event`.
- `cancel(key)` — kills the live process for this key but keeps its `session_id` and `history_path` so the next `send()` respawns and `session/load`s back in.
- `stop(key)` — kills the process for this key and clears its session state.

## Dependencies

- **Native folder picker:** `tauri-plugin-dialog` (Rust) + `@tauri-apps/plugin-dialog` (JS).
- **Persistence:** `localStorage` (no additional deps).
- **Avatar colors:** Pure JavaScript hash function, no library.

## References

- [Connection Architecture](../architecture/connection.md)
- [Session System](./session-system.md)
