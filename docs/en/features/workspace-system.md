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

When `activePath` changes (via `workspacesStore.select()`), a `watch` in `MainLayout.vue` triggers:

```
watch(activePath)
  → zeroStore.stopSession()      // disconnect from previous workspace
  → zeroStore.startSession(cwd)  // connect to new workspace
  → zeroStore.loadSessions(cwd)  // refresh session list
```

The `startSession` action:

1. Clears messages from the previous workspace.
2. Calls `setupListeners()` to attach `zero:event` and `zero:stderr` listeners.
3. Calls the Tauri command `start_zero_session(cwd)` which tells the Rust bridge the workspace directory.
4. Sets `isConnected = true`.

The first message sent after selection causes the Rust bridge to spawn `zero exec --cwd <path>`, and subsequent messages use `--resume <sessionId>` for the same session.

## Rust Backend

### Tauri Commands

| Command                                | File        | Description                                                       |
| -------------------------------------- | ----------- | ----------------------------------------------------------------- |
| `locate_zero_cli`                      | `lib.rs:59` | Finds zero binary and returns path + version.                     |
| `start_zero_session(cwd, session_id?)` | `lib.rs:65` | Tells the bridge the workspace. Optional `session_id` for resume. |
| `send_zero_message(content)`           | `lib.rs:73` | Sends a user message. Bridge spawns `zero exec` if needed.        |
| `stop_zero_session`                    | `lib.rs:78` | Kills the current zero process and clears state.                  |

### Bridge State (`bridge.rs`)

The `ZeroBridge` holds a `SessionState` per active connection:

```rust
struct SessionState {
    cwd: PathBuf,                           // workspace directory
    session_id: Arc<Mutex<Option<String>>>, // captured from run_start
    child: Option<Child>,                   // current zero process
}
```

- `start(cwd, resume_id)` — stores the workspace and optional resume session ID.
- `send(event)` — spawns a new `zero exec` process (with `--resume` if session_id is set), writes the message, and spawns stdout/stderr readers that emit Tauri events.
- `stop()` — kills the child process and clears state.

## Dependencies

- **Native folder picker:** `tauri-plugin-dialog` (Rust) + `@tauri-apps/plugin-dialog` (JS).
- **Persistence:** `localStorage` (no additional deps).
- **Avatar colors:** Pure JavaScript hash function, no library.

## References

- [Connection Architecture](../architecture/connection.md)
- [ADR 001: Connection via stream-json](../architecture/decisions/001-connection-via-stream-json.md)
