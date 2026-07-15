# Plan System

This document describes how zero-desktop renders the agent's task plan — the structured checklist the agent emits via `update_plan` tool calls and `plan_update` ACP events.

## Overview

Zero's agent can maintain an in-memory plan of the steps it intends to take for a given task. It communicates this plan through:

- **`update_plan` tool calls**: The agent calls a tool named `update_plan` with an `args.plan` array of step entries. This is the primary mechanism — the plan replaces the previous one entirely on each call.
- **`plan` session updates**: ACP `session/update` notifications with `sessionUpdate: "plan"` carry an `entries` array directly. These are translated into `plan_update` events by the bridge.

The frontend renders the plan **inline in the chat input bar** (pinned above the textarea) so the user can see what the agent is working on without scrolling through the message history. The plan auto-hides once every item is completed.

## Data Flow

```
┌─────────────────────────────────┐
│  ACP session/update              │
│  sessionUpdate: "plan"           │
│  entries: [{content, status,     │
│             priority}]           │
└───────────────┬─────────────────┘
                │ bridge.rs translate_session_update
┌───────────────▼─────────────────┐
│  zero:event                      │
│  type: "plan_update"             │
│  entries: [...]                  │
└───────────────┬─────────────────┘
                │ frontend listener
┌───────────────▼─────────────────┐
│  zero-store.js                   │
│  currentPlan = event.entries     │
│  activePlan getter               │
│    → null if all completed       │
└───────────────┬─────────────────┘
                │ reactive binding
┌───────────────▼─────────────────┐
│  ChatInput.vue                   │
│  Plan checklist (inline)         │
│  or PlanPanel.vue (standalone)   │
└─────────────────────────────────┘
```

Additionally, `update_plan` tool calls (which carry `args.plan`) are intercepted in `addToolCall` and never rendered as tool-call cards — the plan is tracked via `currentPlan` instead:

```
tool_call (name: "update_plan")
  → addToolCall()
    → if name === "update_plan":
        currentPlan = args.plan
        return (no card rendered)
```

## Rust Backend

### ACP event translation (`bridge.rs`)

The `translate_session_update` function handles `sessionUpdate: "plan"`:

```rust
"plan" => {
    let entries = update.get("entries").cloned().unwrap_or(serde_json::json!([]));
    Some(OutputEvent::new("plan_update", serde_json::json!({ "entries": entries })))
}
```

Each entry in the `entries` array has:

```json
{
  "content": "Fix the login bug",
  "status": "in_progress",
  "priority": 0
}
```

**Status values** (as emitted by zero's agent):

- `"pending"` — not yet started
- `"in_progress"` — currently working on
- `"completed"` — done
- `"failed"` — couldn't complete

The bridge does not interpret or modify plan entries; it passes them through verbatim. Zero's agent emits `plan` updates that **replace** the entire plan each time (not incremental patches), so the frontend's `currentPlan` is always a complete snapshot.

## Frontend

### Plan State

`currentPlan` lives on the **per-session** `zero-session-store.js` (the
`useZeroSessionStore(key)` factory store), not on the global `zero-store.js` —
each open panel tracks its own agent's plan independently.

| State         | Type    | Description                                                    |
| ------------- | ------- | -------------------------------------------------------------- |
| `currentPlan` | `Array` | The agent's current plan entries (replaced whole each update). |

### `activePlan` getter

```js
activePlan(state) {
  if (!state.currentPlan || state.currentPlan.length === 0) return null;
  const allDone = state.currentPlan.every((item) => item.status === "completed");
  return allDone ? null : state.currentPlan;
}
```

Returns `null` when the plan is empty or every item is completed — the UI auto-hides. Returns the plan array otherwise.

### Event handling (`handleZeroEvent`)

```js
case "plan_update":
  if (Array.isArray(event.entries)) {
    this.currentPlan = event.entries;
  }
  break;
```

The plan is **replaced whole** on each `plan_update` event. There is no incremental patching or diffing — zero's agent sends the complete state every time.

### Tool call interception (`addToolCall`)

```js
if (event.name === "update_plan") {
  if (Array.isArray(event.args?.plan)) {
    this.currentPlan = event.args.plan;
  }
  return; // no card rendered
}
```

`update_plan` tool calls update `currentPlan` and return without pushing a message — they never appear as tool-call cards in the message history. This is reused for both live events and history replay, since both funnel through `addToolCall`.

### Plan reset

`currentPlan` is cleared to `[]` when:

- A new session starts (`startSession`).
- A session is opened from history (`openSession`).
- The process exits (`handleProcessExited`).

### `src/utils/plan.js` — Icon and color helpers

```js
export function planIcon(status) {
  // pending → "radio_button_unchecked"
  // in_progress → "autorenew"
  // completed → "check_circle"
  // failed → "cancel"
}

export function planColor(status) {
  // pending → "grey-6"
  // in_progress → "info"
  // completed → "positive"
  // failed → "negative"
}
```

## UI Components

### Inline in `ChatInput.vue`

The plan checklist is rendered directly above the textarea in `ChatInput.vue` when `activePlan` is non-null:

```
┌─────────────────────────────────┐
│  ◉ Analisar o código            │  ← in_progress (blue,
│  ○ Escrever testes              │     spinning icon)
│  ○ Atualizar documentação       │  ← pending (grey,
│  ○ Verificar build              │     empty circle)
│ ─────────────────────────────── │
│  [textarea]                  ↑  │
└─────────────────────────────────┘
```

**Behavior:**

- Each item shows a status icon (from `planIcon`) with the corresponding color (from `planColor`).
- Completed items get strikethrough text.
- Items in `in_progress` get a spinning animation.
- The checklist is compact — designed not to take significant vertical space from the chat input.
- When all items complete, `activePlan` returns `null` and the checklist auto-hides.

### Standalone `PlanPanel.vue`

An alternative rendering as a standalone panel, used in `ChatView.vue`'s right column on wider screens (≥1024px). Shows the same checklist but in a dedicated `260px` panel bordered from the chat area. Hidden on narrow screens.

## History replay

When replaying a session from history, `update_plan` tool calls in the event stream update `currentPlan` through the same `addToolCall` path — but since `currentPlan` only matters for the live UI (it's already displayed in the message list differently), and `buildMessagesFromHistory` rebuilds messages from scratch, the plan state during replay is transient. The plan panel appears in replayed sessions if a plan was active at the end, since it's purely derived from `currentPlan`.

There is no dedicated `plan_update` history event type — plans are only captured when the agent emits an `update_plan` tool call. If the agent's plan is needed for faithful history replay, the `plan` ACP update needs to be written to the local history; currently only `update_plan` tool calls (which carry the full plan in `args.plan`) are persisted, not the standalone `plan` session update.

## References

- [Chat Interface](./chat-interface.md)
- [zero-bridge: Connection to the zero CLI](./zero-bridge.md)
- [Edit Tools utility](../../src/utils/edit-tools.js) — companion to `update_plan` for diff rendering
