# Sistema de Sessões

Este documento descreve como o zero-desktop lista, exibe, retoma e gerencia sessões de chat do zero CLI.

## Visão Geral

O zero persiste cada turno de conversa em disco em `~/.local/share/zero/sessions/<session-id>/`. Cada diretório de sessão contém:

- `events.jsonl` — todos os eventos como JSONL (um objeto JSON por linha). Em modo ACP, o zero grava apenas entradas `message` aqui.
- `metadata.json` — metadados da sessão.
- `session.lock` — lock de concorrência.

O zero-desktop mantém seu **próprio** histórico de sessão mais rico em `~/.local/share/zero-desktop/session-history/<sessionId>.jsonl`, que registra mensagens, chamadas de ferramenta, pensamentos, pedidos de permissão e decisões de permissão — tudo que o app precisa para reproduzir fielmente uma sessão. Dois arquivos adicionais de overlay (`session-titles.json` e `session-models.json`) suprem dados que o ACP não expõe.

## Fluxo de Dados

```
┌─────────────────────────────┐
│  zero CLI                    │
│  ~/.local/share/zero/       │
│    sessions/<id>/            │
│      events.jsonl            │
└──────────┬──────────────────┘
            │ lido pelo Rust (fallback)
┌──────────▼──────────────────┐
│  dados locais zero-desktop   │
│  ~/.local/share/             │
│    zero-desktop/             │
│      session-history/        │
│        <id>.jsonl (primário) │
│      session-titles.json     │
│      session-models.json     │
└──────────┬──────────────────┘
            │ lido pelo Rust
┌──────────▼──────────────────┐
│  Backend Rust (Tauri)        │
│  list_zero_sessions(cwd)     │
│    → zero sessions list --json + filtrar por cwd + overlay de títulos/modelos
│  load_session_history(id)    │
│    → prefere log local, fallback pro events.jsonl do zero
│  delete_session(id)          │
│    → remove log local + título/modelo + diretório do zero
│  rename_session(id, title)   │
│    → atualiza session-titles.json
└──────────┬──────────────────┘
            │ IPC Tauri `invoke`
┌──────────▼──────────────────┐
│  Frontend (Pinia Store)     │
│  loadSessions(cwd)           │
│  openSession(sessionId)      │
│    → buildMessagesFromHistory(events)
│  _sessionSyncTimer (3s)      │
│    → releitura periódica do histórico
└──────────┬──────────────────┘
            │ bindings reativos
┌──────────▼──────────────────┐
│  MainLayout.vue (drawer)    │
│  ChatView.vue (mensagens)   │
└─────────────────────────────┘
```

## Backend Rust

### `list_zero_sessions` (`lib.rs`)

```
Comando Tauri: list_zero_sessions(cwd: PathBuf) → Vec<SessionInfo>
```

1. Spawna `zero sessions list --json`.
2. Faz parse do array JSON em `Vec<SessionInfo>`.
3. Filtra sessões onde `session.cwd == <cwd solicitado>`.
4. Sobrepõe os títulos próprios do zero-desktop (de `session-titles.json`) e ids de modelo (de `session-models.json`), já que sessões criadas via ACP recebem um título genérico "ACP session" e um `modelId` vazio do próprio zero.
5. Retorna a lista filtrada e com overlay.

**Estrutura SessionInfo:**

```rust
pub struct SessionInfo {
    pub session_id: String,   // ID único do zero
    pub title: String,        // título do zero-desktop (auto ou definido pelo usuário)
    pub created_at: String,   // timestamp ISO 8601
    pub cwd: String,          // diretório do workspace
    pub model_id: String,     // sobreposto do session-models.json
    pub event_count: Option<i64>,
    pub kind: String,         // "" | "fork" | "child"
    pub provider: String,     // ex: "openai-compatible"
}
```

### `load_session_history` (`lib.rs`)

```
Comando Tauri: load_session_history(session_id: String) → Vec<SessionEvent>
```

1. Primeiro tenta o log local do zero-desktop em `<data_dir>/zero-desktop/session-history/<sessionId>.jsonl`.
2. Fallback para o `events.jsonl` do próprio zero em `<data_dir>/zero/sessions/<sessionId>/events.jsonl`.
3. Lê o arquivo linha por linha, filtrando tipos de evento relevantes: `message`, `reasoning`, `tool_call`, `tool_result`, `permission_request`, `permission_decision`, `error`.
4. Retorna array de `SessionEvent` com `type`, `payload` (JSON não tipado), e `createdAt`.

O `buildMessagesFromHistory` do frontend normaliza esses eventos em mensagens tipadas (text, thinking, tool_call, permission_request, etc.) da mesma forma que eventos do stream ao vivo são normalizados.

### `delete_session` (`lib.rs`)

```
Comando Tauri: delete_session(session_id: String) → ()
```

1. Remove o arquivo de histórico local do zero-desktop (`session-history/<id>.jsonl`).
2. Remove entradas de overlay de título e modelo.
3. Remove o diretório inteiro da sessão do zero (`<data_dir>/zero/sessions/<id>/`).
4. Sem erro se já removido (idempotente).

### `rename_session` (`lib.rs`)

```
Comando Tauri: rename_session(session_id: String, title: String) → ()
```

Atualiza a entrada no `session-titles.json`. Chamado automaticamente na primeira mensagem de uma sessão (para derivar título do conteúdo da mensagem), e em renomeações explícitas do usuário.

### Retomada de sessão

Quando `start_zero_session` é chamado com um `session_id`, o bridge abre a sessão via `session/load` (o equivalente ACP do `--resume`). Se `session/load` falhar (ex: diretório da sessão foi deletado), faz fallback para `session/new` — a conversa começa do zero em vez de gerar erro.

### Derivação de título

No primeiro `send()` de uma sessão, se nenhum título foi gravado ainda:

- Os primeiros 60 caracteres da mensagem do usuário (com whitespace colapsado) viram o título.
- Uma mensagem só com arquivo (conteúdo vazio) usa o nome do arquivo.
- O título é persistido no `session-titles.json`.

### Snapshot de modelo

Após todo handshake bem-sucedido (`session/new`, `session/load`, ou fallback), o bridge faz snapshot do modelo atualmente ativo (de `zero config --json`) no `session-models.json`. Isso garante que a lista de sessões mostre qual modelo respondeu, mesmo após o modelo ser trocado globalmente.

## Frontend

### `zero-store.js` — Estado das Sessões

| Estado             | Tipo                                | Descrição                                 |
| ------------------ | ----------------------------------- | ----------------------------------------- |
| `currentSessionId` | `string \| null`                    | ID da sessão atualmente visualizada.      |
| `sessions`         | `Array`                             | Lista de sessões do workspace ativo.      |
| `messages`         | `Array<mensagem tipada>`            | Lista completa de mensagens exibida no `ChatView`. Inclui text, thinking, tool_call, permission_request, permission_decision, error. |
| `currentWorkspace` | `string`                            | Caminho do workspace ativo.               |
| `isLoadingSession` | `boolean`                           | True enquanto `openSession` busca histórico. |

### Ações

| Ação | Descrição |
|---|---|
| `loadSessions(cwd)` | Chama `listZeroSessions(cwd)` e armazena em `this.sessions`. Erros são silenciosamente ignorados. |
| `openSession(sessionId)` | Chama `loadSessionHistory(sessionId)`, executa `buildMessagesFromHistory` para popular `this.messages` com objetos de mensagem tipados. Define `this.currentSessionId` e inicia o timer de sync de 3s. |
| `removeSession(sessionId)` | Chama `deleteSession(sessionId)`. Se a sessão excluída estava ativa, para ela primeiro. Reseta estado e atualiza a lista de sessões. |
| `renameSession(sessionId, title)` | Chama `renameSession(sessionId, title)` e então atualiza a lista de sessões. |
| `startSession(cwd, sessionId?)` | Reconecta o bridge com opção de resume da sessão. Limpa `messages`, define `currentWorkspace` e `currentSessionId`. |

### Reprodução de histórico (`buildMessagesFromHistory`)

O frontend normaliza eventos persistidos no mesmo formato de mensagem tipada usado para eventos ao vivo:

| Tipo de evento persistido | Produz                                              |
| ------------------------- | --------------------------------------------------- |
| `message` (role=user)     | `{ type: "text", role: "user", content, file? }`    |
| `message` (role=assistant)| `{ type: "text", role: "assistant", content }`      |
| `reasoning`               | `{ type: "thinking", content }`                     |
| `tool_call`               | `{ type: "tool_call", toolName, toolUseId, input }` |
| `tool_result`             | Atualiza status + resultado do `tool_call`          |
| `permission_request`      | `{ type: "permission_request", answerable: false }` |
| `permission_decision`     | Atualiza status do `permission_request`             |
| `error`                   | `{ type: "error", content }`                        |

Pedidos de permissão do histórico são sempre `answerable: false` — o processo que perguntou já se foi. Se existir um evento `permission_decision` correspondente, o status do pedido é atualizado para `approved` ou `denied`; caso contrário, renderiza como expirado.

### Sincronização periódica (`_sessionSyncTimer`)

Enquanto uma sessão está aberta (e nenhum turno está em andamento), a store relê `loadSessionHistory` a cada 3 segundos. Se a contagem de eventos mudou, reconstrói a lista de mensagens do zero. Isso captura:

- Novos eventos escritos pelo bridge durante o turno atual.
- Mudanças externas na sessão de outra instância do zero-desktop.
- Eventos que chegaram tarde, após o `openSession` inicial ter completado.

O timer para quando a sessão é trocada ou fechada.

## Componentes de UI

### MainLayout.vue — Painel Direito

```
┌────────────────────────────┐
│  meu-projeto                │
│  ──────────────────────    │
│  Sessões (3)               │
│                            │
│  💬 oi                    │  ← título da primeira mensagem
│     deepseek-v4  09/07     │
│                            │
│  💬 corrigir bug           │
│     deepseek-v4  08/07     │  ← modelo + data
│                            │
│  ⚡ add feature (fork)     │  ← ícone difere para fork
│     deepseek-v4  08/07     │
│                            │
└────────────────────────────┘
```

- **Item de sessão:** `q-item` com `clickable` e `v-ripple`. Sessão ativa destacada com `bg-primary-1`.
- **Ícone:** `chat_bubble_outline` (padrão), `call_split` (fork), `subdirectory_arrow_right` (child).
- **Título:** Usa `session.title` (do overlay do zero-desktop) ou fallback para os últimos 8 caracteres de `session.session_id`.
- **Subtítulo:** `model_id` + data formatada (`DD/MM/AA HH:MM`).

### Fluxo ao Clicar na Sessão

```
Usuário clica no item da sessão
  → onSelectSession(session)
    → zeroStore.startSession(cwd, session.session_id)
        → Bridge: inicia zero acp com session/load
    → zeroStore.openSession(session.session_id)
        → loadSessionHistory(session_id)
        → buildMessagesFromHistory constrói mensagens tipadas
        → ChatView renderiza a conversa completa
    → zeroStore.loadSessions(cwd)
        → atualiza lista de sessões
```

## Referências

- [Arquitetura de Conexão](../architecture/connection.md)
- [Sistema de Workspaces](./workspace-system.md)
- [zero-bridge](./zero-bridge.md)
