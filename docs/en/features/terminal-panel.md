# Terminal Panel

This document describes the embedded terminal emulator — a dockable panel at the bottom of the screen where the user can run real shell processes (dev servers, build tools, git, anything) without leaving the app, and attach a terminal's output to a focused chat panel to show the agent an error.

## Overview

The terminal panel provides:

- **Real shells, not a fake console**: each tab spawns an actual PTY-backed shell process (the user's own `$SHELL`, as a login shell), so prompts, colors, job control, and interactive programs (`vim`, `htop`, REPLs) work exactly as in a native terminal.
- **Browser-tab-style multitasking**: open/close as many terminal tabs as needed; each is an independent shell.
- **Per-workspace scoping**: terminal tabs belong to the workspace they were opened in (same model as chat session panels) — switching workspaces shows that workspace's own tabs, while tabs from other workspaces keep running in the background.
- **Citation to chat**: a toolbar button attaches the currently active terminal's visible output (or selection) to whichever chat panel currently has focus — as a small chip (like a picked file), not pasted as visible text, so the compose box doesn't get buried under however many lines were on screen.

The panel is a custom fixed-position element docked to the bottom of the window (Quasar's `q-drawer` only supports `left`/`right`, not `bottom`), toggled by a floating button and resizable via a drag handle.

## Data Flow

```
┌──────────────────────────────┐
│  User's shell ($SHELL, login) │
│  real PTY (portable-pty)      │
└────────────┬─────────────────┘
             │ raw byte reads (dedicated OS thread)
┌────────────▼─────────────────┐
│  Tauri Rust Backend           │
│  TerminalManager (terminal.rs)│
│  spawn_terminal(key,cwd,...)  │
│  write_terminal(key,data)     │
│  resize_terminal(key,cols,..) │
│  kill_terminal(key)           │
│  list_terminals()             │
│  events: terminal:data        │
│          terminal:exit        │
└────────────┬─────────────────┘
             │ Tauri IPC invoke/listen
┌────────────▼─────────────────┐
│  Frontend (Pinia stores)      │
│  terminal-runtime-store.js    │
│    openKeys, focusedKeyByPath │
│  terminal-session-store.js    │
│    xterm.js Terminal instance │
└────────────┬─────────────────┘
             │ reactive bindings
┌────────────▼─────────────────┐
│  TerminalPanel.vue            │
│  TerminalTabStrip.vue         │
│  TerminalHost.vue (xterm.js)  │
└───────────────────────────────┘
```

## Rust Backend

### `terminal.rs` — `TerminalManager`

One PTY-backed shell per open tab, tracked in `Arc<Mutex<HashMap<String, TerminalHandle>>>` keyed by a frontend-owned uuid — the same map-per-key shape `ZeroBridge` (see [zero-bridge](./zero-bridge.md)) uses for `zero acp` processes, but for real shells. Unlike `ZeroBridge`, there is no respawn-on-demand: closing a tab kills its shell for good.

Built on the [`portable-pty`](https://crates.io/crates/portable-pty) crate (the crate wezterm itself is built on) rather than a Unix-only alternative, so the app isn't locked out of a Windows PTY backend (ConPTY) later even though it currently only ships for Linux.

**Key implementation details:**

- **Default shell resolution**: `CommandBuilder::new_default_prog()` resolves `$SHELL` (falling back to the passwd database, then `/bin/sh`) and launches it as a **login shell** (argv0 prefixed with `-`), so `.bashrc`/`.zshrc`/profile files are sourced — this is what makes the user's normal `PATH`/aliases/dev-environment setup available inside the panel. The full parent environment is inherited automatically; `TERM=xterm-256color` is set explicitly (GUI apps are usually launched with no `TERM` at all).
- **Raw byte streaming, not line-based**: `portable-pty`'s `Read`/`Write`/`Child` are blocking/synchronous (no tokio equivalent), so each terminal gets a dedicated `std::thread::spawn` reader loop (not `tokio::spawn`) doing raw `read()` calls into an 8 KB buffer — unlike `ZeroBridge`'s stdout reader, which assumes UTF-8 JSON-RPC lines. A small leftover-byte buffer (`drain_utf8`, ≤3 bytes) is carried between reads so a multi-byte UTF-8 character split across two reads isn't corrupted into replacement characters.
- **Writes/resizes/kills** go through `tokio::task::spawn_blocking` since they call into the same synchronous PTY API from an async Tauri command.
- **Cleanup on exit**: when the reader loop sees EOF (shell exited, or was killed), it reaps the child (`child.wait()`), removes the terminal from the map, and emits `terminal:exit` — this is the _only_ place entries are removed, whether the shell exited on its own or was killed via `kill_terminal`/`kill_all`. `TerminalManager::kill_all()` is wired into the app's `RunEvent::Exit` handler (alongside `ZeroBridge::kill_all()`) so no orphan shells remain when the app quits.

**Data structures:**

```rust
pub struct TerminalSpawnInfo {
    pub key: String,
    pub pid: Option<u32>,
    pub shell: String,   // basename of $SHELL, for the tab label
}

pub struct LiveTerminalInfo {
    pub key: String,
    pub cwd: String,
    pub live: bool,
}
```

### Commands in `lib.rs`

| Command           | Description                                                                                               |
| ----------------- | --------------------------------------------------------------------------------------------------------- |
| `spawn_terminal`  | Opens a PTY, spawns the default shell in `cwd` with the given `cols`/`rows`. Returns `TerminalSpawnInfo`. |
| `write_terminal`  | Writes raw input (keystrokes, pasted text) to the shell's stdin.                                          |
| `resize_terminal` | Calls the PTY's `resize()` (ioctl) so the shell/apps inside reflow (`$COLUMNS`, curses UIs).              |
| `kill_terminal`   | Sends the kill signal; does not block until the process actually exits.                                   |
| `list_terminals`  | Returns `Vec<LiveTerminalInfo>` for frontend reconciliation.                                              |

### Events

| Event           | Payload             | Description                          |
| --------------- | ------------------- | ------------------------------------ |
| `terminal:data` | `{ key, data }`     | A chunk of PTY output (valid UTF-8). |
| `terminal:exit` | `{ key, exitCode }` | The shell process exited.            |

## Frontend

### `terminal-runtime-store.js`

Mirrors `session-runtime-store.js`'s shape, scoped per workspace, but simpler: no panel cap, no "hide but keep running" close mode.

| State              | Type     | Description                                                                                       |
| ------------------ | -------- | ------------------------------------------------------------------------------------------------- |
| `openKeys`         | `Array`  | Flat list of open terminal tabs, across **all** workspaces.                                       |
| `focusedKeyByPath` | `Object` | Per-workspace-path focused tab key.                                                               |
| `keyMeta`          | `Object` | Per-key metadata: `{ cwd, title, shell, pid, status }` (`status`: `spawning`/`running`/`exited`). |
| `panelOpen`        | `bool`   | Whether the bottom panel is expanded.                                                             |
| `panelHeightPx`    | `number` | Panel height in px, persisted to `localStorage`.                                                  |

| Getter                | Description                                                 |
| --------------------- | ----------------------------------------------------------- |
| `visibleKeys(path)`   | Tabs belonging to a given workspace (drives the tab strip). |
| `focusedKeyFor(path)` | The focused tab for a given workspace.                      |

| Action                | Description                                                                                                                                                |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `openTab(key, path)`  | Adds a tab, focuses it, opens the panel.                                                                                                                   |
| `focusTab(key, path)` | Focuses an already-open tab.                                                                                                                               |
| `closeTab(key)`       | Kills the shell (`terminal-session-store`'s `kill()`), then removes the tab. Always kills — there is no "close but keep running" mode, unlike chat panels. |

### `terminal-session-store.js`

Dynamic per-tab store (`useTerminalSessionStore(key)`, one Pinia instance per open tab, same factory pattern as `zero-session-store.js`), owning the live `xterm.js` `Terminal`/`FitAddon` instances (created by `TerminalHost.vue`, which needs `$q` for theming) and the backend PTY lifecycle.

| Action                   | Description                                                                                                                                                                                                                                                                                                   |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `attach(term, fitAddon)` | Stores the xterm.js instances (via `markRaw()` — they run their own render loop and must not become a Vue reactive proxy).                                                                                                                                                                                    |
| `spawn(cwd)`             | Calls `spawn_terminal`, wires `onTerminalData`/`onTerminalExit` listeners filtered by this tab's key, wires `term.onData()` to `write_terminal`.                                                                                                                                                              |
| `resize(cols, rows)`     | Calls `resize_terminal`.                                                                                                                                                                                                                                                                                      |
| `kill()`                 | Detaches listeners, calls `kill_terminal` (best-effort — already-exited is fine), disposes the xterm.js instance.                                                                                                                                                                                             |
| `extractCiteText()`      | Returns the active selection (`term.getSelection()`) or, if none, the currently visible viewport (`term.buffer.active`, line by line) as plain text — used by the "cite to chat" action. Deliberately plain text, not ANSI-preserving (`@xterm/addon-serialize` was considered and rejected for this reason). |

## UI Components (`src/components/terminal/`)

### `TerminalPanel.vue`

The bottom dock. Not a `q-drawer` (Quasar drawers only support `left`/`right`) — a custom `position: fixed` element, modeled on `McpDrawer.vue`'s floating toggle-button idiom rotated to the bottom edge.

- **Width tracks the side drawers dynamically**: rather than re-deriving the left sidebar's and `McpDrawer`'s current widths from their own local state, the panel reads back the actual `padding-left`/`padding-right` Quasar already applies to `.q-page-container` (via a `ResizeObserver`, which fires on padding-only changes too) — the same padding that already keeps `SessionTileGrid` correctly bounded between the two drawers. This is what keeps the terminal panel sitting between the drawers (in both push and overlay/mobile mode) instead of spanning the full viewport width and getting visually cut off/covered by them.
- **Resizable height** via a drag handle at the panel's top edge, persisted to `localStorage`.
- **Every open tab across every workspace stays mounted** (`v-show`, never `v-if`) so switching tabs or workspaces never tears down a live `xterm.js` instance — only `TerminalHost.vue`'s DOM visibility toggles. Only the actually-focused one for the active workspace is shown at a time.
- Contains `TerminalTabStrip.vue`, a "cite to chat" toolbar button, and the terminal hosts.

### `TerminalTabStrip.vue`

Hand-rolled closable pill/chip strip (modeled on `McpDrawer.vue`'s `.mcp-file-chip` pattern rather than Quasar's `q-tabs`, which is built for navigation tabs, not closable browser-tab semantics). Shows a status dot (spawning/running/exited), the tab title, a close button, and a trailing "+" to open a new tab in the active workspace.

### `TerminalHost.vue`

Creates the `xterm.js` `Terminal` + `FitAddon` on first mount, hands them to the tab's `terminal-session-store` via `attach()`, then calls `spawn(cwd)`. A `ResizeObserver` on its own element calls `fitAddon.fit()` and (debounced) `resize_terminal` on size changes.

### Citation ("cite to chat")

A single toolbar button in `TerminalPanel.vue` (operating on the currently focused terminal tab, not one button per tab):

1. Resolves the focused terminal tab (`terminalRuntime.focusedKeyFor(activePath)`) and the focused **chat** panel (`sessionRuntime.focusedKeyFor(activePath)` — the same resolution `McpDrawer.vue` already uses for its edited-files list).
2. Reads the terminal's visible output via `extractCiteText()`.
3. Sets it as that chat panel's `pendingAttachment` — `{ mimeType: "text/plain", data: textToBase64(text), name: "<tab-title>-output.txt" }` — the same shape and same single-attachment slot `ChatInput.vue`'s own file-picker uses.

This is deliberately an **attachment, not pasted text**: the compose box would otherwise get filled with however many lines the terminal had on screen, burying whatever the user was actually typing. `ChatInput.vue` renders it as the same small chip (name + mime type + remove button) a picked file gets, and `send_zero_message` already knows how to turn a `text/plain` attachment into agent context (`bridge.rs` wraps it in an `<attached file>` block) — no backend change needed.

This required two small prerequisites:

- **`pendingAttachment` (and `draftText`) moved from local `ref`s in `ChatInput.vue`/`ChatView.vue` into `zero-session-store.js`** — both the compose box's text and its single pending attachment now live in the per-session Pinia store instead of local component state, so another component (the terminal panel) can set either on whichever panel is focused without reaching into `ChatInput.vue`'s/`ChatView.vue`'s internals. Both are cleared automatically on panel close (`store.$reset()`, already called by `closePanel`/`stopAndDispose`). `ChatInput.vue` watches the attachment for replacement/removal to revoke an image's blob: URL (previously done on unmount — no longer correct, since the attachment now outlives a panel-count-driven remount).
- **Focus tracking fixed in `SessionTileGrid.vue`**: `session-runtime-store.js`'s `focusedKeyByPath` used to be updated only by `openPanel` (most-recently-opened panel), never by the user actually clicking into an already-open pane — `ChatView.vue` already emitted `@focus-input` on its textarea's focus event, but nothing listened for it. `SessionTileGrid.vue` now wires that emit (and a `@mousedown.capture` on the whole pane, not just the textarea) to `sessionRuntime.focusPanel(key, activePath)`, so "the panel in focus" reflects where the user is actually working.

## Behavior Notes

- **Ephemeral**: terminals are not persisted across app restarts. Closing a tab kills its shell; quitting the app kills every live terminal (`kill_all` on `RunEvent::Exit`).
- **No per-workspace cap**: unlike chat panels (`MAX_OPEN_PANELS = 4`), there's no limit on how many terminal tabs a workspace can have open.
- **`SessionTileGrid.vue`'s `gridHeight`** subtracts `terminalRuntime.panelHeightPx` when the panel is open, so the chat tiling grid doesn't render underneath the terminal panel.

## New Dependencies

- **Cargo**: `portable-pty = "0.9"`
- **npm**: `@xterm/xterm`, `@xterm/addon-fit`

## References

- [zero-bridge: Connection to the zero CLI](./zero-bridge.md) — the process-per-key pattern this feature's `TerminalManager` mirrors.
- [Session System](./session-system.md) — the per-workspace panel/focus model this feature's tab scoping mirrors.
- [MCP Panel](./mcp-panel.md) — the floating toggle-button/drawer idiom this panel's UI is modeled on.
