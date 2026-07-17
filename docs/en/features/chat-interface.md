# chat-interface: Chat UI Components

This document describes the chat interface component architecture and the multi-type message rendering system.

## Overview

The chat UI renders a heterogeneous list of typed messages. Each message has a `type` field that determines which Vue component renders it. This replaces the earlier flat `{ role, content }` model where all non-text events were dumped as raw JSON strings.

## Message model

All messages share common fields and add type-specific fields:

```js
{
  id: string,          // unique identifier
  type: 'text' | 'thinking' | 'tool_call' | 'permission_request' | 'permission_decision' | 'error',
  timestamp: number,
  // type-specific fields below
}
```

### `text` messages

```js
{
  type: 'text',
  role: 'user' | 'assistant' | 'system',
  content: string,
  file?: { mimeType: string, data: string, name: string }  // user messages only
}
```

Rendered by `TextMessage.vue` using Quasar's `<q-chat-message>` with role-based colors. User messages with an attached file show the file preview (image thumbnail or file chip) above the text.

### `thinking` messages

```js
{ type: 'thinking', content: string }
```

Rendered by `ThinkingBlock.vue` in two modes:

- **Streaming** (`streaming=true`): A thin amber-tinted bar with a spinner and "Thinking..." label. Not expandable — the content is still arriving. This appears at the bottom of the message list alongside the streaming text bubble.
- **Finalized** (`streaming=false`): A collapsible `q-expansion-item` with a check icon and "Thinking" label. Click to reveal the full reasoning text in italic.

### `tool_call` messages

```js
{
  type: 'tool_call',
  toolName: string,
  toolUseId: string,
  input: object,
  status: 'running' | 'completed' | 'error',
  result: string | null,
}
```

Rendered by `ToolCallMessage.vue` as a card with states:

- **running**: spinner + tool icon + tool name + "running..." label. Input params shown in a tooltip.
- **completed**: check icon + tool name + "completed" label. Expandable result area with a "Show more/Less" toggle (truncated at 25 lines) and a copy-to-clipboard button.
- **error**: error icon + tool name + "error" label. Result shown in red.

Special rendering for known tools:

- **`edit_file` / `write_file`**: Shows a unified diff view (oldStr in red, newStr in green) with monospace font.
- **`update_plan`**: Not rendered as a card at all — the store captures the plan entries separately and they appear pinned above the chat input via `activePlan`.

Tool calls are updated inline: when a `tool_result` event arrives, the store finds the matching `tool_call` by `toolUseId` and sets `status` and `result`.

### `permission_request` messages

```js
{
  type: 'permission_request',
  requestId: string,
  toolName: string,
  reason: string,
  options: Array<{ optionId: string, name: string, kind: string }>,
  answerable: boolean,
  status: 'pending' | 'approved' | 'denied',
}
```

Two rendering paths depending on `answerable`:

- **Live (answerable=true)**: Rendered by `PendingPermissionPanel.vue` pinned above the chat input. Shows the tool name, reason, and whatever options ACP actually offered (e.g. "Allow", "Allow for session", "Reject" — not a fixed pair). The user clicks an option and `respondToPermission` delivers the JSON-RPC reply.
- **History (answerable=false)**: Rendered inline in the message list as a read-only card by `PendingPermissionPanel.vue` or as a badge by `PermissionDecisionBadge.vue`. Shows the outcome if a matching `permission_decision` exists, otherwise shows "expired".

### `permission_decision` messages

```js
{
  type: 'permission_decision',
  toolName: string,
  action: 'allow' | 'deny',
  reason: string,
}
```

Rendered by `PermissionDecisionBadge.vue` as a compact inline badge. These come from informational auto-decisions the model makes without asking, or from the user's decision being persisted and then replayed from history.

### `error` messages

```js
{ type: 'error', content: string }
```

Rendered by `ErrorMessage.vue` as an inline error bubble with a warning icon. Typically shown when the zero process crashes unexpectedly.

## Component tree

```
ChatView.vue
├── WorkingIndicator.vue          (global status bar — not used directly; status is now inline in ChatInput)
├── TextMessage.vue               (type: text)
├── ThinkingBlock.vue             (type: thinking — compact bar or expandable)
├── ToolCallMessage.vue           (type: tool_call — running/completed/error with diff view)
├── PendingPermissionPanel.vue    (type: permission_request — answerable or read-only)
├── PermissionDecisionBadge.vue   (type: permission_decision — compact badge)
├── ErrorMessage.vue              (type: error)
└── q-chat-message                (streaming — currentResponse)
```

Plus, above/below the message list:

```
ChatView.vue / IndexPage.vue
├── PendingPermissionPanel.vue    (pinned above input while a live permission is pending)
└── ChatInput.vue
    ├── Plan checklist             (inline: pinned above input while plan is active)
    ├── Working status bar         (colored bar with thinking/tool/writing/sending status)
    ├── File attachment preview    (image thumbnail or file chip with remove button)
    ├── Attach button              (native file picker → read_file_attachment)
    ├── Execution mode selector    (auto / ask / plan — see Plan Mode doc)
    ├── Model picker               (dropdown with search, recent models, active indicator)
    └── Send / Cancel button       (arrow_upward when idle, pause when running)
```

All components live under `src/components/chat/`.

## Permission flow

1. The agent sends a `session/request_permission` JSON-RPC request via ACP.
2. The Rust bridge translates it, assigns a `correlation_id`, persists the request to the local history, and emits `zero:permission-request` to the frontend.
3. The store creates a `permission_request` message with `status: 'pending'` and `answerable: true`.
4. `PendingPermissionPanel.vue` renders it pinned above the chat input with the options ACP offered.
5. User clicks an option → store calls `respondToPermission(requestId, optionId)`.
6. The store updates the message's `status` and `chosenOptionId`, and invokes the Tauri command.
7. The Rust bridge looks up the pending request by `correlation_id`, persists a `permission_decision` to the local history, and sends the JSON-RPC reply to the agent.
8. The agent receives the decision and continues or aborts the tool call.

There is no client-side auto-allow shortcut anymore: the three-way execution mode selector (`auto` / `ask` / `plan`) is the real ACP `session/set_mode`, enforced by the engine itself — in `auto` mode the engine already runs safe tools without asking and only prompts for genuinely risky ones, so every `session/request_permission` the frontend receives is one the engine itself decided was worth asking about. See [Plan Mode](./plan-mode.md) for the full mode-switching mechanism and its per-session persistence.

## Plan system

The `update_plan` tool call is treated specially: instead of rendering a tool-call card, the store updates `currentPlan` with the entries from the `plan_update` event. The `activePlan` getter returns `null` when all items are completed, auto-hiding the checklist.

The plan checklist is rendered:

- **Inline in `ChatInput.vue`**: Pinned above the textarea while active. Shows each item with a status icon (pending → `radio_button_unchecked`, in_progress → `hourglass_empty`, completed → `check_circle` with strikethrough text).
- **In `PlanPanel.vue`**: Standalone panel component, used in alternative layouts.

## State management

Since multi-session parallel chat (see [ADR 004](../architecture/decisions/004-multi-session-parallel.md)), this per-conversation state lives on the **per-session** `zero-session-store.js` (the `useZeroSessionStore(key)` factory store) — one instance per open panel — not on the global `zero-store.js`:

- `messages[]` — typed message list
- `currentResponse` — streaming text buffer
- `currentThinking` — streaming thinking buffer
- `currentPlan` — the agent's plan entries (replaced whole on each `plan_update`)
- `workingStatus` getter — returns `'thinking'`, `{ type: 'tool', toolName }`, `'writing'`, `'sending'`, or `null`. Used by `ChatInput.vue`'s status bar.

The global `zero-store.js` singleton only holds app-wide state: `activeModel` / `availableModels`, which seed each session's own picker. The execution mode (`auto` / `ask` / `plan`) is per-session state on `zero-session-store.js` — see [Plan Mode](./plan-mode.md).

Streaming is finalized into permanent messages when:

- Thinking: the next non-`reasoning` event arrives (`text`, `tool_call`, `permission_request`, `run_end`, `error`).
- Text: `run_end` event arrives (or process exits).

## File attachments

The chat input includes an attach button that opens the native file dialog filtered to supported extensions (images: png/jpg/gif/webp; text/code: txt, md, csv, json, yaml, xml, html, css, js, ts, jsx, tsx, py, go, rs, java, kt, swift, c, cpp, rb, php, sh, sql, dockerfile).

After selection:

1. `readFileAttachment(path)` is called on the Rust side, which reads the file, validates size (max 10 MB), detects image vs. text by extension, and returns it base64-encoded.
2. The frontend renders a preview: image thumbnail for images, file chip (icon + name + MIME type) for text/code.
3. On send, the attachment is passed alongside the message content to `send_zero_message`.
4. The bridge builds ACP prompt blocks: images become `{"type":"image","mimeType":...,"data":...}`, text files become `{"type":"text","text":"<attached file name=...>\n...\n</attached file>"}`.

## i18n

Chat translation keys in `src/i18n/`:

| Key                       | pt-BR                    | en-US               |
| ------------------------- | ------------------------ | ------------------- |
| `chat.thinking`           | Pensamento               | Thinking            |
| `chat.thinkingRunning`    | Pensando...              | Thinking...         |
| `chat.toolRunning`        | em execução...           | running...          |
| `chat.toolCompleted`      | concluído                | completed           |
| `chat.writing`            | Escrevendo resposta...   | Writing response... |
| `chat.sending`            | Enviando...              | Sending...          |
| `chat.showMore`           | Mostrar mais             | Show more           |
| `chat.showLess`           | Mostrar menos            | Show less           |
| `chat.copy`               | Copiar                   | Copy                |
| `chat.permissionRequired` | Permissão necessária     | Permission required |
| `chat.approve`            | Aprovar                  | Approve             |
| `chat.deny`               | Recusar                  | Deny                |
| `chat.cancelRun`          | Cancelar execução        | Cancel run          |
| `chat.attachFile`         | Anexar arquivo           | Attach file         |
| `chat.removeAttachment`   | Remover anexo            | Remove attachment   |
| `chat.modelLabel`         | Modelo                   | Model               |
| `chat.switchModel`        | Trocar modelo            | Switch model        |
| `chat.searchModel`        | Buscar modelo...         | Search model...     |
| `chat.recentModels`       | Recentes                 | Recent              |
| `chat.loadingModels`      | Carregando modelos...    | Loading models...   |
| `chat.noModelsMatch`      | Nenhum modelo encontrado | No models match     |
| `chat.autoAllow`          | Auto                     | Auto                |
| `chat.ask`                | Perguntar                | Ask                 |
