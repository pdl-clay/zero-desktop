# Model Switching

This document describes how zero-desktop lets users switch the active AI model and how that change propagates through the system.

## Overview

Zero supports multiple AI providers and models. zero-desktop exposes the active provider's model list and lets the user switch models from the chat input bar. The model switch is a **global, persisted zero CLI/config change** — ACP has no per-session model switching method (`session/set_model` and `session/models` both return "method not found"), so switching affects every `zero` process on the machine, not just the current session.

## Data Flow

```
┌──────────────────────────────┐
│  Frontend                     │
│  ChatInput.vue                │
│  Model picker dropdown        │
│    → loadAvailableModels()    │
│    → switchModel(id)          │
└──────────────┬───────────────┘
               │ Tauri invoke
┌──────────────▼───────────────┐
│  Rust: list_zero_models       │
│    → active_provider_entry()  │
│      → zero config --json     │
│        → reads activeProvider │
│        → reads provider.model │
│    → zero providers models    │
│      <provider> --json        │
│        → network call to      │
│          provider's /v1/models │
│    → returns { models, active }│
└──────────────────────────────┘

┌──────────────────────────────┐
│  Rust: switch_zero_model      │
│    → active_provider_entry()  │
│    → zero providers add       │
│      <provider>               │
│      --name <provider>        │
│      --model <new-model>      │
│      --set-active             │
│    → bridge.cancel()          │
│      (kills live process)     │
└──────────────┬───────────────┘
               │ next send() respawns
               │ with session/load,
               │ new model takes effect
```

## Rust Backend

### `list_zero_models` (`bridge.rs` → `lib.rs`)

```
Tauri command: list_zero_models() → AvailableModels
```

**Steps:**

1. `active_provider_entry()` runs `zero config --json` to find the active provider's name and current model.
2. Runs `zero providers models <provider> --json` — a real network probe against the provider's own `/v1/models`-style endpoint. Not instant, not cached.
3. Parses the `models[].id` fields into a string array.
4. Returns `AvailableModels { models: Vec<String>, active: String }`.

**active_provider_entry() internals:**

- Parses `zero config --json` output.
- Reads `activeProvider` (e.g. `"opencode-go"`) and the matching provider's `model` field.
- The provider `name` doubles as the `<catalog-id>` for `zero providers add` — verified live that updating an existing profile with the same `--name` updates in place rather than creating a duplicate.

**Limitation:** Neither `zero config --json`, `zero providers current --json`, nor `zero providers list --json` expose the `catalogID` field. If a renamed profile ever breaks this `name == catalog-id` assumption, the fallback is reading `~/.config/zero/config.json` directly.

### `switch_zero_model` (`bridge.rs` → `lib.rs`)

```
Tauri command: switch_zero_model(key: String, model: String) → ()
```

**Steps:**

1. Resolves the active provider name via `active_provider_entry()`.
2. Runs `zero providers add <provider> --name <provider> --model <model> --set-active` — updates the provider's model in zero's config and marks it active. This part is global — it changes the config file every `zero` process reads.
3. Calls `bridge.cancel(key)` to kill **only** the live `zero acp` process for the given session key (see [ADR 004](../architecture/decisions/004-multi-session-parallel.md), decision #6). The session id and history are preserved, and every other open session's process keeps running under its previously snapshotted model.
4. On the next `send()` for that same key, the bridge respawns the process via `spawn_and_handshake` with `session/load`, and re-snapshots the model into `session-models.json` — the new model takes effect for the next turn of that session.

**Why kill the process?** ACP has no method to change the model mid-session. The only way for a running `zero acp` process to pick up a config change is to restart it. Killing and respawning via `session/load` is effectively a session reconnect with the new model.

### Session model snapshot

After every successful handshake (`session/new`, `session/load`, or fallback), the bridge snapshots the currently active model:

```rust
if let Some(model_id) = active_model_id().await {
    let _ = set_session_model(&session_id, &model_id);
}
```

This is stored in `~/.local/share/zero-desktop/session-models.json` (`{ sessionId: modelId }`) and overlaid onto `list_zero_sessions` output, since ACP reports an empty `modelId` in `zero sessions list --json`. The snapshot happens after _every_ handshake, not just `session/new`, so a model switch mid-session captures the new model correctly.

## Frontend

### `zero-store.js` — Model State

| State             | Type       | Description                                 |
| ----------------- | ---------- | ------------------------------------------- |
| `availableModels` | `string[]` | List of model IDs from the active provider. |
| `activeModel`     | `string`   | Currently active model ID.                  |
| `isLoadingModels` | `bool`     | True while fetching models.                 |
| `_modelsLoaded`   | `bool`     | Guard to avoid repeated fetches.            |

### Actions

| Action                           | Description                                                                                 |
| -------------------------------- | ------------------------------------------------------------------------------------------- |
| `loadAvailableModels({ force })` | Calls `listZeroModels()`. Caches result in `_modelsLoaded`; pass `force: true` to re-fetch. |

`switchModel(model)` itself is **not** an action on `zero-store.js` — since multi-session parallel chat (ADR 004), it lives on the per-session `zero-session-store.js` (`useZeroSessionStore(key)`). It guards against no-op (same model) and run-in-progress, then branches on connection state before touching the backend:

- **Connected** (`isConnected`): calls `switchZeroModel(key, model)` to restart only that session's live process, exactly as before.
- **Has a `sessionId` but isn't connected yet** (e.g. a session just reopened from history, before the user resumed it): `switch_zero_model`/`switch_session_model` require a `sessions` entry keyed by the panel's `key`, which the Rust bridge only registers once a session has actually connected at least once — calling it earlier used to throw `"No active session for key: <uuid>"`. Fixed by calling `set_zero_session_model_by_id(sessionId, model)` instead, a disk-only write (`bridge::set_session_model`) that requires no live registration. It's picked up automatically on the next connect via the same model-reapply block in `spawn_and_handshake` that already handles reconnects.
- **Brand-new panel with no `sessionId` at all yet**: nothing is persisted synchronously; `this.activeModel` is still updated locally, and `_realignModelBeforeSend` (called from `sendMessage`, right after `startSession` connects) pushes it once a real session exists.

In every case, `this.activeModel` (this panel's own choice) and the global store's `activeModel` (the default for panels that haven't connected yet) are updated immediately, regardless of which branch persists it.

### `ChatInput.vue` — Model Picker

The model picker is a dropdown (`q-btn-dropdown` or similar) in the chat input bar that shows:

- **Current model** as the button label (truncated with ellipsis for long names).
- **Search/filter input** at the top of the dropdown.
- **Active indicator** (check or dot) next to the currently active model.
- **Model list** in a scrollable area.

The picker is disabled while `runInProgress` is true, since the model can only take effect on the next turn (the process restart happens on the next `send()`).

## UX Considerations

- **Model list is a network call**: `zero providers models` probes the provider's live API. On first open it may take a moment; subsequent opens use the cached list unless forced.
- **Switch is global**: Changing the model affects every `zero` invocation on the machine, including CLI usage outside zero-desktop. This is a hard constraint of zero's architecture — the user is warned via the tooltip/documentation.
- **Switch mid-turn is blocked**: The picker is disabled while a run is in progress. The model change only takes effect on the next message.
- **Session history preserves the model**: `session-models.json` records which model answered each session, so reopening an old session still shows the model that was active at the time.

## References

- [zero-bridge: Connection to the zero CLI](./zero-bridge.md)
- [Connection Architecture](../architecture/connection.md)
- [Session System](./session-system.md)
