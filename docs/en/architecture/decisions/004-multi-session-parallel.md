# 004 — Multi-Session Parallel Chat (Tiling)

## Status

Accepted. Builds on [ADR 003 — Migrate to ACP](./003-migrate-to-acp.md).

## Context

ADR 003 established one `zero acp` process per session, but the GUI still kept a
**single live session at a time**: `ZeroBridge` held an `Option<AcpSession>`
slot, and both the Rust `start()` and the frontend unconditionally killed the
current session before starting a new one. Switching workspaces or sessions
meant losing any in-progress work.

The goal of this change is real parallelism: multiple sessions processing
simultaneously, visible side-by-side in resizable tiles, where opening a new
session never kills an existing one.

## Decisions (confirmed with the user)

1. **Parallelism within the same workspace** is allowed. The user accepts the
   risk of concurrent file edits without blocking — only a non-blocking warning.
2. **Tiling visualization**: each open session gets its own resizable panel.
3. **Closing a panel does NOT stop the session**: only an explicit "Stop" action
   kills the process. Reopening reconnects to the same process/state.
4. **Indicators**: badges/spinners on session list items and workspace avatars
   show which sessions are processing in the background, with a distinct
   "needs attention" state for pending permissions.
5. **No global process cap (updated)**: the backend imposes no limit on the
   number of live `zero acp` processes — the user manages them freely. The
   frontend enforces a **per-workspace** panel cap of 4 (`MAX_OPEN_PANELS` in
   `session-runtime-store.js`): each workspace can have up to 4 open panels,
   and panels from other workspaces keep running in the background without
   counting against another workspace's limit. This lets the user work with
   two workspaces simultaneously, each with up to 4 running panels.
6. **Model switch affects only the focused session**: other background sessions
   continue with their previous model.
7. **Responsive panels**: each panel adapts its content to the available width
   as more panels open (1 → 2 → 3 → 4).

## Architecture

### Rust: key-based session map

`ZeroBridge.sessions` changed from `Option<AcpSession>` to
`HashMap<String, AcpSession>`, keyed by a **frontend-owned routing key** (UUID
for new sessions, `session_id` for resumed ones). All commands (`start`, `send`,
`stop`, `cancel`, `switch_zero_model`) accept this `key: String` parameter.

Every emitted event (`zero:event`, `zero:stderr`, `zero:process-exited`,
`zero:permission-request`) now carries `sessionKey` in its payload, so frontend
listeners filter by their own key.

`start()` returns a `StartedSession { key, session_id, reattached }` struct —
this also fixes a pre-existing bug where the frontend's `currentSessionId` was
never updated to the real CLI-assigned id.

The cap is enforced in `openOrFocusSession()` in the frontend's
`session-runtime-store.js`, checking `panelCountFor(workspacePath)` against
`MAX_OPEN_PANELS`, returning `{ error: "SESSION_CAP_REACHED" }` when the
per-workspace limit is exceeded. The Rust `start()` no longer enforces any
process cap.

### Frontend: store split

The monolithic `zero-store.js` was split into:

- **`zero-store.js`** (global): `zeroPath`, `availableModels`, `activeModel`,
  `mcpBackends`, `mcpTools` — app-wide state only.
- **`zero-session-store.js`** (factory): `useZeroSessionStore(key)` creates a
  per-session Pinia store with `messages`, `currentResponse`, `runInProgress`,
  etc. All listeners filter by `sessionKey`. The factory pattern allows
  independent state for each open session.
- **`session-runtime-store.js`** (orchestrator): `openKeys` (panel display
  order), `focusedKey`, `keyMeta` (per-key metadata for badges). Provides
  `openOrFocusSession(key, cwd, sessionId)` — the single entry point for
  opening/focusing a session panel.
- **`workspaces-store.js`**: gained `sessionsByPath` to hold per-workspace
  session lists (previously on the singleton session store).

### Frontend: tiling UI

`SessionTileGrid.vue` replaces the single `<ChatView>` in `MainLayout.vue`. It
renders 1 (full), 2 (horizontal split), 3 (nested), or 4 (2×2 grid) panels using
Quasar's `QSplitter`. Each panel has a `SessionPaneHeader.vue` with distinct
"Close" (hide only) and "Stop" (kill process) buttons.

`ChatView.vue` now accepts a `sessionKey` prop, creates its own session store
instance, and `provide`s it to child components via `inject("zeroStore")`.

### Frontend: responsiveness

Each `ChatView` root element has a `ResizeObserver` that tracks the panel's
actual width (not the window width). Below a 500px threshold, the pane gets a
`pane--narrow` class that hides the PlanPanel, collapses ChatInput buttons, and
reduces padding — adapting to the shrinking space as more panels open.

### Cleanup

`.kill_on_drop(true)` on the `zero acp` `Command` plus a `RunEvent::Exit` handler
calling `ZeroBridge::kill_all()` ensures no orphan processes remain when the app
closes. File-level `std::sync::Mutex` guards on `session-titles.json` and
`session-models.json` prevent concurrent read-modify-write races.

## Consequences

- `MainLayout.vue` no longer kills sessions on workspace switch — switching
  workspaces is pure navigation (loads the session list for that workspace).
- `McpDrawer.vue` reads `editedFiles` from the focused session's store (not the
  global store, which no longer has it).
- `ChatInput.vue` calls `sessionStore.switchModel()` (per-session restart) not
  `globalStore.switchModel()` (display-only).
- The session list sidebar shows live badges (working/pending permission) by
  cross-referencing `sessionRuntime.keyMeta` with `session.session_id`.
- `list_live_sessions` command added for frontend state reconciliation.

> **Update:** Decision #3 above ("Closing a panel does NOT stop the session:
> only an explicit 'Stop' action kills the process") and the "distinct Close
> and Stop buttons" described under Architecture were later simplified:
> `SessionPaneHeader.vue` now has a single close button. `closePanel()` behaves
> conditionally — it only hides the panel while a turn is running, but also
> stops and disposes the session once idle, since with a per-workspace panel
> cap and no separate manual "stop" affordance, an idle session left running
> would otherwise waste a slot the user has no way to reclaim.
> `stopAndDispose()` (unconditional kill) still exists, but is only used when
> the user deletes the underlying session entirely, not from the panel's own
> close button. See `docs/features/session-system.md` for the current
> behavior; this ADR is left as a historical record of the original decision.
