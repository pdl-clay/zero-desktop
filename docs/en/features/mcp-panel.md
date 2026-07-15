# MCP Panel

This document describes the MCP (Model Context Protocol) backend panel — the right-side drawer that shows configured MCP servers, their health status, and the files the agent has edited during the current session.

## Overview

The MCP panel provides visibility into:

- **MCP backends**: all servers configured in zero's config (`zero backends --json`), with live health-check status (OK/error/unknown), transport type (stdio/http), and tool counts.
- **Edited files**: files the agent has touched via `edit_file`/`write_file` during the current session, with inline diff previews.
- **Status cache**: persisted on-disk so the drawer renders immediately with last-known data, before any live check completes.

The panel lives in a `q-drawer` on the right side of the chat view, toggled by a floating button on the right screen edge.

## Data Flow

```
┌─────────────────────────────────┐
│  zero CLI                        │
│  zero backends --json            │
│  zero mcp check --json           │
│  zero mcp tools list --json      │
└────────────┬────────────────────┘
             │ read by Rust (live)
┌────────────▼────────────────────┐
│  zero-desktop local data         │
│  ~/.local/share/zero-desktop/    │
│    mcp-status-cache.json         │
└────────────┬────────────────────┘
             │ read by Rust (cached)
┌────────────▼────────────────────┐
│  Tauri Rust Backend              │
│  list_mcp_backends()             │
│    → zero backends --json        │
│    → overlay cached statuses     │
│  check_mcp_backend(name)         │
│    → zero mcp check --json       │
│    → persist to cache            │
│  load_mcp_status_cache()         │
│    → raw cache read              │
│  list_mcp_tools()                │
│    → zero mcp tools list --json  │
└────────────┬────────────────────┘
             │ Tauri IPC invoke
┌────────────▼────────────────────┐
│  Frontend (Pinia Store)          │
│  loadMcpBackends()               │
│  checkMcpBackend(name)           │
│  mcpBackends[], mcpTools[]       │
│  editedFiles getter              │
└────────────┬────────────────────┘
             │ reactive bindings
┌────────────▼────────────────────┐
│  McpDrawer.vue                   │
│  Backend cards + edited files    │
└─────────────────────────────────┘
```

## Rust Backend

### `mcp_cache.rs`

Persistent on-disk cache of MCP backend health-check statuses at `<app_data_dir>/zero-desktop/mcp-status-cache.json`.

**Data structures:**

```rust
pub struct CachedStatus {
    pub status: String,       // "ok" | "error"
    pub tool_count: i64,
    pub error: Option<String>,
    pub checked_at: Option<u64>,  // unix timestamp (seconds)
}

pub struct McpStatusCache {
    pub servers: HashMap<String, CachedStatus>,
    pub generated_at: Option<u64>,
}
```

**Operations:**

| Function              | Description                                                |
| --------------------- | ---------------------------------------------------------- |
| `load()`              | Reads cache from disk, returns empty cache if missing/corrupt. |
| `save(cache)`         | Writes cache to disk, creating parent dirs if needed.      |
| `set_status(name, status, tool_count, error)` | Updates a single server's entry and persists. |
| `remove(name)`        | Removes a server from the cache.                           |
| `clear()`             | Empties the entire cache.                                  |
| `get(name)`           | Returns a clone of a server's cached status, if any.       |
| `all()`               | Returns a clone of all cached statuses.                    |

The cache uses a `thread_local!` override path for tests, so test code can point the cache at a temp directory without interfering with the real cache.

### Commands in `lib.rs`

| Command                    | Description                                                                                       |
| -------------------------- | ------------------------------------------------------------------------------------------------- |
| `list_mcp_backends`        | Reads `zero backends --json` and overlays cached status/tool_count/error. Returns `Vec<McpBackendInfo>`. |
| `check_mcp_backend`        | Live-checks a single server (`zero mcp check --json`), persists result to cache, returns `McpCheckResult`. |
| `check_mcp_backend_cached` | Returns cached status if present; falls back to live check otherwise.                             |
| `load_mcp_status_cache`    | Reads the raw cache from disk for fast initial rendering. Returns `McpStatusCache`.               |
| `list_mcp_tools`           | Lists tools from all enabled MCP servers (`zero mcp tools list --json`). Returns `Vec<McpToolInfo>`. |

**McpBackendInfo struct:**

```rust
pub struct McpBackendInfo {
    pub name: String,
    pub backend_type: String,   // "stdio" | "http"
    pub url: Option<String>,
    pub arg_count: i64,
    pub env_key_count: i64,
    pub header_count: i64,
    pub tool_count: i64,
    pub allow_granted: i64,
    pub deny_granted: i64,
    pub status: Option<String>,   // overlaid from cache
    pub error: Option<String>,    // overlaid from cache
}
```

## Frontend

### `zero-store.js` — MCP State

| State          | Type    | Description                                        |
| -------------- | ------- | -------------------------------------------------- |
| `mcpBackends`  | `Array` | Configured MCP servers with health status.         |
| `mcpTools`     | `Array` | All tools from enabled MCP servers.                |
| `isLoadingMcp` | `bool`  | True while backends are being fetched.             |
| `_mcpLoaded`   | `bool`  | Guard to avoid repeated fetches.                   |

### Actions

| Action                       | Description                                                                                      |
| ---------------------------- | ------------------------------------------------------------------------------------------------ |
| `loadMcpBackends({ force })` | Loads cache first, then fetches backends + tools in parallel. Overlays cached statuses. If `force` or no cache exists, runs live checks on all backends. |
| `checkMcpBackend(name)`      | Live-checks a single backend. Updates `mcpBackends[name]` with status/tools/error inline. Uses per-backend tool data from the check result when available, falling back to the global `mcpTools` list. |
| `checkAllMcpBackends()`      | Runs `checkMcpBackend` for every backend in parallel.                                             |
| `loadMcpTools({ force })`    | Refresh only the global tools list.                                                              |

### `editedFiles` getter

The `zero-store.js` Pinia store exposes an `editedFiles` getter that groups `edit_file`/`write_file` tool calls by file path, preserving both file-encounter order and per-file edit order. This is derived purely from `state.messages`, which is reset/rebuilt on session switch, so no manual watcher is needed.

Each entry:
```js
{
  path: "/full/path/to/file.ts",
  edits: [
    { id, toolName, input, status, timestamp },  // chronologically
  ]
}
```

The `isEditTool()` utility (from `src/utils/edit-tools.js`) matches:
- `edit_file` / `write_file` (zero native)
- `*_edit_file` / `*_write_file` (MCP-backed variants, e.g. `mcp_filesystem_edit_file`)

## UI Components

### `McpDrawer.vue`

```
┌─────────────────────────────────┐
│  dns  MCP Panel          🔄  ✕  │  ← header
│ ─────────────────────────────── │
│  EDITED FILES                    │
│  [📄 app.rs] [📄 lib.rs] [📄 C…] │  ← file chips with edit counts
│  ┌─────────────────────────┐    │
│  │ lib.rs                  ✕ │    │  ← expanded diff panel
│  │ - old line               │    │
│  │ + new line               │    │
│  └─────────────────────────┘    │
│ ─────────────────────────────── │
│  ┌─ terminal  filesystem ──┐    │
│  │ stdio            ✓ ok   │    │  ← backend card
│  └─────────────────────────┘    │
│  ┌─ language  brave-search ─┐   │
│  │ http     search.brave.com│    │  ← http backend
│  │               ⬤ idle    │    │
│  └─────────────────────────┘    │
│ ─────────────────────────────── │
│  Backends configured in         │  ← footer hint
│  ~/.config/zero/config.json     │
└─────────────────────────────────┘
```

**Behavior:**

- **Collapse/expand**: On desktop (≥1024px), the toggle button switches the drawer width between 0 and 320px. On mobile, it toggles overlay visibility via `modelValue`.
- **Edited files strip**: Horizontal scrollable row of file chips. Each chip shows the file's base name and a badge with the edit count (if >1). Clicking a chip expands an inline diff panel below.
- **Diff panel**: Renders each edit as a monospace diff block with removed lines in red and added lines in green, using `getEditStrings` from `edit-tools.js` to extract `old_str`/`new_str` pairs.
- **Backend cards**: Each card shows the server name, transport type badge (stdio=orange, http=blue), URL (truncated to hostname), and a status icon (spinner while checking, check_circle when OK, error icon on failure, cloud when unknown).
- **Refresh all**: The header refresh button calls `loadMcpBackends({ force: true })` then `checkAllMcpBackends()`.
- **Expanded file** resets on session switch (watched via `zeroStore.currentSessionId`).

## Caching Strategy

1. On first drawer open: `loadMcpStatusCache()` loads the persisted cache immediately — the drawer shows last-known statuses with no network wait.
2. Simultaneously: `listMcpBackends()` fetches the config snapshot and overlays cached data.
3. If no cache exists at all (first ever open), all backends get live-checked.
4. On explicit refresh (force=true): all backends get live-checked and the cache is updated.
5. Each `checkMcpBackend` call persists its result to the cache via `mcp_cache::set_status`.

## References

- [zero-bridge: Connection to the zero CLI](./zero-bridge.md)
- [Connection Architecture](../architecture/connection.md)
- [Session System](./session-system.md)
