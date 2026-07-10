# chat-interface: Chat UI Components

This document describes the chat interface component architecture and the multi-type message rendering system.

## Overview

The chat UI renders a heterogeneous list of typed messages. Each message has a `type` field that determines which Vue component renders it. This replaces the earlier flat `{ role, content }` model where all non-text events were dumped as raw JSON strings.

## Message model

All messages share common fields and add type-specific fields:

```js
{
  id: string,          // unique identifier
  type: 'text' | 'thinking' | 'tool_call' | 'permission_request',
  timestamp: number,
  // type-specific fields below
}
```

### `text` messages

```js
{ type: 'text', role: 'user' | 'assistant' | 'system', content: string }
```

Rendered by `TextMessage.vue` using Quasar's `<q-chat-message>` with role-based colors.

### `thinking` messages

```js
{ type: 'thinking', content: string }
```

Rendered by `ThinkingBlock.vue` in two modes:

- **Streaming** (`streaming=true`): A thin amber-tinted bar with a spinner and "Thinking..." label. Not expandable — the content is still arriving. This appears at the bottom of the message list alongside the streaming text bubble.
- **Finalized** (`streaming=false`): A collapsible `q-expansion-item` with a check icon and "Thinking" label. Click to reveal the full reasoning text in italic.

The `WorkingIndicator.vue` component (a slim status bar at the top of the chat) also displays the current activity state, providing a second visual cue that the agent is working:

| State                 | Color  | Label                   |
| --------------------- | ------ | ----------------------- |
| `thinking`            | Amber  | "Thinking..."           |
| `tool` (running tool) | Blue   | "running {toolName}..." |
| `writing`             | Green  | "Writing response..."   |
| (idle)                | Hidden | —                       |

### `tool_call` messages

```js
{
  type: 'tool_call',
  toolName: string,
  toolUseId: string,
  input: object,
  status: 'running' | 'completed',
  result: string | null,
}
```

Rendered by `ToolCallMessage.vue` as a card with two visual states:

- **running**: spinner + tool icon + tool name + "running..." label. Input params shown in a tooltip.
- **completed**: check icon + tool name + "completed" label. Expandable result area with a "Show more/Less" toggle (truncated at 25 lines) and a copy-to-clipboard button.

Tool calls are updated inline: when a `tool_result` event arrives, the store finds the matching `tool_call` by `toolUseId` and sets `status = 'completed'` and `result`.

### `permission_request` messages

```js
{
  type: 'permission_request',
  permissionId: string,
  toolName: string,
  proposedCommand: string,
  status: 'pending' | 'approved' | 'denied',
}
```

Rendered by `PermissionRequest.vue` as a warning-bordered card showing the tool name and proposed command in monospace. When `status === 'pending'`, it displays Approve (green) and Deny (red) buttons. After a decision, the icon and colors update to reflect the outcome.

## Component tree

```
ChatView.vue
├── WorkingIndicator.vue     (global status bar: thinking/tool/writing)
├── TextMessage.vue          (type: text)
├── ThinkingBlock.vue        (type: thinking — compact bar or expandable)
├── ToolCallMessage.vue      (type: tool_call — running/completed)
├── PermissionRequest.vue    (type: permission_request)
├── ThinkingBlock            (streaming — currentThinking, thin bar)
└── q-chat-message           (streaming — currentResponse)
```

All components live under `src/components/chat/`.

## Permission flow

1. Zero emits a `permission_request` event via stream-json.
2. Rust bridge receives it and emits `zero:event` to the frontend.
3. The store creates a `permission_request` message with `status: 'pending'`.
4. `PermissionRequest.vue` renders the card with Approve/Deny buttons.
5. User clicks a button → store calls `approvePermission(id)` or `denyPermission(id)`.
6. Store updates `status` to `'approved'` or `'denied'` and invokes the Tauri command `send_permission_decision`.
7. Rust bridge receives the decision through an mpsc channel and writes it as a `permission_decision` JSONL event to zero's persistent stdin.
8. Zero processes the decision and continues or aborts the tool call.

The stdin channel is kept open by a background tokio task spawned alongside each `zero exec` process. When the session is stopped or a new message is sent, the channel sender is dropped, which closes stdin.

## State management

The `zero-store.js` Pinia store maintains:

- `messages[]` — typed message list
- `currentResponse` — streaming text buffer
- `currentThinking` — streaming thinking buffer
- `workingStatus` getter — returns `'thinking'`, `{ type: 'tool', toolName }`, `'writing'`, or `null`. Used by `WorkingIndicator.vue` to display the current activity.

Streaming is finalized into permanent messages when:

- Thinking: the next non-`reasoning` event arrives (`text`, `tool_call`, `permission_request`, `final`, `run_end`, `error`).
- Text: `final` or `run_end` events arrive.

## i18n

New chat translation keys added in `src/i18n/`:

| Key                       | pt-BR                  | en-US               |
| ------------------------- | ---------------------- | ------------------- |
| `chat.thinking`           | Pensamento             | Thinking            |
| `chat.thinkingRunning`    | Pensando...            | Thinking...         |
| `chat.toolRunning`        | em execução...         | running...          |
| `chat.toolCompleted`      | concluído              | completed           |
| `chat.writing`            | Escrevendo resposta... | Writing response... |
| `chat.showMore`           | Mostrar mais           | Show more           |
| `chat.showLess`           | Mostrar menos          | Show less           |
| `chat.copy`               | Copiar                 | Copy                |
| `chat.permissionRequired` | Permissão necessária   | Permission required |
| `chat.approve`            | Aprovar                | Approve             |
| `chat.deny`               | Recusar                | Deny                |
