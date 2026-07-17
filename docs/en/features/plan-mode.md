# Plan Mode

Plan Mode is zero-desktop's native equivalent of Claude Code's Plan Mode: the
agent is restricted to read-only exploration and, instead of making changes,
drafts an implementation plan and stops for the user to review it in a dialog
with **Approve** / **Request changes** actions. Approving lets the agent
proceed to implement the plan (either automatically or with per-edit
confirmation); requesting changes keeps the agent in read-only mode while it
revises the plan based on feedback.

## Overview

The zero engine (`zero acp`) already implements the underlying mechanism
natively as the ACP permission mode `"spec-draft"` (advertised to clients as
`"Plan"`). While a session is in this mode:

- Only read-only tools, `ask_user`, and a special `submit_spec` tool are
  advertised to the model — enforced entirely **server-side** by the engine's
  tool-advertisement gate. zero-desktop does not implement any client-side
  tool blocking.
- When the model finishes exploring, it calls `submit_spec(title, plan)`,
  which writes a markdown file to `<cwd>/.zero/specs/<date>-<slug>.md` and
  ends the turn.
- The engine emits an ACP `session/update` notification with
  `sessionUpdate: "_zero/spec_review_required"` (a ZERO vendor extension)
  carrying the spec's id, title, and file path.

Unlike `zero exec --use-spec` (CLI) or the TUI, the ACP path does not fork a
separate "spec-impl" session on approval — `zero spec approve`/`reject` are
CLI-only commands that cannot be used against ACP-created sessions (the
session metadata they require is never recorded over ACP). Approval and
rejection are therefore implemented as a **continuation of the same session**:
approving switches the ACP mode back to `"auto"` or `"ask"` and sends a normal
follow-up prompt instructing the agent to implement the already-submitted
spec; requesting changes simply sends the user's feedback as the next prompt
while the mode stays `"spec-draft"`. This is also a closer match to Claude
Code's own Plan Mode, which never forks the conversation.

## Data Flow

```
┌───────────────────────────────┐
│ User picks "Plan" in the        │
│ execution mode dropdown         │
│ ChatInput.vue → setMode()       │
└───────────────┬─────────────────┘
                │ session/set_mode "spec-draft"
┌───────────────▼─────────────────┐
│ zero acp (Go engine)             │
│ read-only tools + submit_spec    │
│ only advertised to the model     │
└───────────────┬─────────────────┘
                │ submit_spec(title, plan)
                │ → writes .zero/specs/<id>.md
                │ → session/update
                │   "_zero/spec_review_required"
┌───────────────▼─────────────────┐
│ bridge.rs                        │
│ translate_session_update         │
│  → zero:event "spec_review_required" │
│ spawn_stdout_reader              │
│  → persists PendingSpec to       │
│    session-plan-state.json       │
└───────────────┬─────────────────┘
                │ listener frontend
┌───────────────▼─────────────────┐
│ zero-session-store.js            │
│ _loadPlanReview(event)           │
│  → readSpecFile() → pendingPlanReview │
└───────────────┬─────────────────┘
                │ reactive binding
┌───────────────▼─────────────────┐
│ PlanReviewDialog.vue             │
│ markdown plan + Approve /        │
│ Request changes                  │
└─────────────────────────────────┘
```

## Backend Rust (`src-tauri/src/bridge.rs`, `lib.rs`)

### Persisted per-session plan state

Every fresh `zero acp` process — whether a mid-run respawn or a full app
restart — registers its ACP session pinned to `PermissionModeAuto` on the Go
side (`registerSession` in `my-zero`'s `internal/acp/agent.go`); the engine
itself has no memory of a session having been in `spec-draft`. zero-desktop
therefore keeps its own record, mirroring how `session-models.json` already
solves the same problem for per-session model choice:

`~/.local/share/zero-desktop/session-plan-state.json` — a
`HashMap<session_id, SessionPlanState>`:

```rust
struct SessionPlanState {
    mode: String, // "auto" | "ask" | "spec-draft" - absent = "auto"
    pending_spec: Option<PendingSpec>, // spec_id, title, file_path, relative_path
}
```

- `spawn_and_handshake` reapplies `spec-draft` via `session/set_mode` right
  after every handshake if the persisted mode calls for it — the same
  respawn-reapply pattern the model map already uses. `"auto"`/`"ask"` need no
  reapplication (`"auto"` is the engine's own default).
- `spawn_stdout_reader` persists a `PendingSpec` the moment
  `_zero/spec_review_required` arrives, resolving the stable `session_id` from
  the shared `sessions` map (the reader loop only has the panel's `session_key`
  in scope). Not written to the chat-replay history log — the durable record
  is the `.md` file plus this JSON entry; `submit_spec`'s own `tool_call`/
  `tool_call_update` (always emitted separately by the engine) already leaves
  a normal trace in the transcript.
- `delete_session` removes the session's plan-state entry alongside its title
  and model records.

### Tauri commands

| Command                                         | Purpose                                                                  |
| ----------------------------------------------- | ------------------------------------------------------------------------ |
| `switch_zero_mode(key, mode)`                   | Live `session/set_mode` push (session must be connected) + disk persist. |
| `set_zero_session_mode_by_id(session_id, mode)` | Disk-only persist, for a panel that hasn't (re)connected yet.            |
| `get_zero_session_plan_state(session_id)`       | Pure disk read — mode + pending spec, no live session required.          |
| `clear_zero_pending_spec(session_id)`           | Clears the persisted pending spec after approve/request-changes.         |
| `read_spec_file(path)`                          | Reads a spec markdown file's content for the review dialog.              |

## Frontend (`src/stores/zero-session-store.js`)

| State               | Type             | Description                                                                          |
| ------------------- | ---------------- | ------------------------------------------------------------------------------------ |
| `sessionMode`       | `string`         | This session's live ACP permission mode: `"auto"` \| `"ask"` \| `"spec-draft"`.      |
| `pendingPlanReview` | `Object \| null` | `{ specId, title, filePath, relativePath, content }` awaiting a decision, or `null`. |
| `_sessionModeDirty` | `bool`           | Internal: a mode change made before this panel had a `sessionId` at all.             |

Key actions:

- `setMode(mode)` — pushes live if connected, persists by id if a `sessionId`
  exists but isn't connected, otherwise marks the choice dirty for
  `_syncPlanStateFromDisk` to flush once `startSession` connects. Drives all
  three modes (`"auto"` / `"ask"` / `"spec-draft"`), not just Plan Mode.
- `_syncPlanStateFromDisk()` — restores `sessionMode`/`pendingPlanReview` from
  the backend's persisted state (or flushes a dirty mode change). Called from
  both `openSession` (browsing history, no live connection needed — covers
  session recovery) and `startSession`'s success path (covers reconnecting
  after a full app restart).
- `_loadPlanReview(event)` — fetches the spec markdown when
  `spec_review_required` arrives live during the current app run.
- `approvePlanReview(mode, comment)` — clears the review, switches mode to
  `"auto"` or `"ask"`, sends the implementation instruction as a normal
  follow-up message.
- `requestPlanChanges(feedback)` — clears the review (mode stays
  `"spec-draft"`), sends the feedback as the next message.

`sessionMode` is reset to `"auto"` and `pendingPlanReview` to `null` on a
brand-new session, on `startSession`/`openSession`'s pre-connect reset, and on
`removeSession` (a truly deleted session). `handleProcessExited` (a live
process crash, not a session switch) clears only `pendingPlanReview` — the
mode is left untouched, since the Rust bridge already reapplies `spec-draft`
on the next respawn, and resetting it locally would show "auto" in the
dropdown while the engine is still read-only underneath.

## UI Components

- **`ChatInput.vue`** — Plan Mode is one of three options in a single
  execution-mode dropdown (auto / ask / plan), replacing what used to be a
  separate two-state auto/ask toggle. All three options call the same
  `sessionStore.setMode(mode)` action and are driven by the real ACP
  `session/set_mode` — there is no client-side approximation left; the old
  global, `localStorage`-backed `auto_allow` shortcut (which auto-clicked
  "allow" on every permission request) was removed in favor of it. See
  [Chat UI Components](./chat-interface.md) for the dropdown's place among
  the input bar's other controls.
- **`PlanReviewDialog.vue`** — the app's first `q-dialog` (persistent, no
  Esc/backdrop dismiss). Renders the spec through the existing
  `renderMarkdown()` utility (`src/utils/markdown.js`) and offers three
  actions:
  - **Approve and implement automatically** → `approvePlanReview("auto")`
  - **Approve and review each edit** → `approvePlanReview("ask")`
  - **Request changes** → opens an inline feedback textarea, then
    `requestPlanChanges(feedback)`

  This mirrors Claude Code's own distinction between auto-accepting edits and
  reviewing them one at a time when exiting plan mode — both map directly to
  the engine's existing `"auto"`/`"ask"` ACP modes.

- **`ChatView.vue`** — mounts `<PlanReviewDialog />` and adds
  `!store.pendingPlanReview` to the `canSend` gate, blocking the main composer
  while a review is pending (feedback is typed inside the dialog itself).

## Persistence and Session Recovery

Both the session's execution mode and a pending plan review survive:

- **A `zero acp` process crash/respawn** — the Rust bridge reapplies
  `spec-draft` automatically before the next turn.
- **Closing and reopening the whole app** — `session-plan-state.json` is read
  back the next time the session connects or is opened from history.
- **Reopening a session from history without reconnecting yet** —
  `openSession` restores the selected mode and, if the spec file is still
  readable, the pending review dialog, before any live process exists.

If a pending spec's `.md` file has since been deleted from disk, the frontend
self-heals by clearing the orphaned persisted record instead of surfacing a
broken review dialog.

## References

- [Plan System](./plan-system.md) — the unrelated, always-on inline
  todo-checklist (`update_plan` tool / ACP `plan` updates) — not to be
  confused with Plan Mode.
- [Advisor Mode](./advisor-mode.md) — the closest prior art for a per-session
  toggleable feature with its own Tauri commands and store state.
- [zero-bridge](./zero-bridge.md)
- [Model Switching](./model-switching.md) — the `session-models.json`
  respawn-reapply pattern this feature's persistence follows.
