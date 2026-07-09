# Sistema de Sessões

Este documento descreve como o zero-desktop lista, exibe e resume sessões de chat do zero CLI.

## Visão Geral

O zero persiste cada turno de conversa em disco em `~/.local/share/zero/sessions/<session-id>/`. Cada diretório de sessão contém:

- `events.jsonl` — todos os eventos (mensagens, chamadas de ferramenta, estatísticas de uso) como JSONL (um objeto JSON por linha).
- `metadata.json` — metadados da sessão.
- `session.lock` — lock de concorrência.

O zero-desktop consome esses dados para:

- Listar sessões com escopo do workspace ativo (`zero sessions list --json`, filtrado por `cwd`).
- Carregar histórico completo de mensagens do `events.jsonl` quando uma sessão é clicada.
- Retomar sessões via `zero exec --resume <sessionId>` para que o modelo retenha o contexto da conversa.

## Fluxo de Dados

```
┌─────────────────────────────┐
│  zero CLI                    │
│  ~/.local/share/zero/       │
│    sessions/<id>/            │
│      events.jsonl            │
└──────────┬──────────────────┘
           │ lido pelo Rust
┌──────────▼──────────────────┐
│  Backend Rust (Tauri)        │
│  list_zero_sessions(cwd)     │
│    → zero sessions list --json + filtrar por cwd
│  load_session_history(id)    │
│    → ler events.jsonl, parse eventos message
└──────────┬──────────────────┘
           │ IPC Tauri `invoke`
┌──────────▼──────────────────┐
│  Frontend (Pinia Store)     │
│  loadSessions(cwd)           │
│  openSession(sessionId)      │
│  state sessions[]            │
│  state messages[]            │
└──────────┬──────────────────┘
           │ bindings reativos
┌──────────▼──────────────────┐
│  MainLayout.vue (drawer)    │
│  ChatView.vue (mensagens)   │
└─────────────────────────────┘
```

## Backend Rust

### `list_zero_sessions` (`lib.rs:28`)

```
Comando Tauri: list_zero_sessions(cwd: PathBuf) → Vec<SessionInfo>
```

1. Spawna `zero sessions list --json`.
2. Faz parse do array JSON em `Vec<SessionInfo>`.
3. Filtra sessões onde `session.cwd == <cwd solicitado>`.
4. Retorna a lista filtrada.

**Estrutura SessionInfo:**

```rust
#[derive(Serialize)]
pub struct SessionInfo {
    pub session_id: String,   // ID único do zero
    pub title: String,        // primeira mensagem do usuário ou vazio
    pub created_at: String,   // timestamp ISO 8601
    pub cwd: String,          // diretório do workspace
    pub model_id: String,     // ex: "deepseek-v4-flash"
    pub event_count: Option<i64>,
    pub kind: String,         // "" | "fork" | "child"
    pub provider: String,     // ex: "openai-compatible"
}
```

**Nota de serialização:** A struct usa `#[serde(alias = "sessionId")]` (não `rename`) para que o JSON camelCase do zero seja desserializado corretamente, mas a resposta para o frontend use snake_case (`session_id`, `created_at`, `model_id`).

### `load_session_history` (`lib.rs:39`)

```
Comando Tauri: load_session_history(session_id: String) → Vec<ChatMessage>
```

1. Resolve o diretório da sessão: `<data_dir>/zero/sessions/<session_id>/events.jsonl`.
2. Lê o arquivo linha por linha.
3. Filtra eventos onde `type == "message"`.
4. Extrai `payload.role`, `payload.content` e `createdAt`.
5. Retorna um array de `ChatMessage`.

**Estrutura ChatMessage:**

```rust
#[derive(Serialize)]
pub struct ChatMessage {
    pub role: String,       // "user" | "assistant"
    pub content: String,    // texto da mensagem
    pub timestamp: String,  // ISO 8601
}
```

### `delete_session` (`lib.rs:79`)

```
Comando Tauri: delete_session(session_id: String) → ()
```

1. Resolve `<data_dir>/zero/sessions/<session_id>/`.
2. Remove o diretório inteiro com `std::fs::remove_dir_all`.
3. Sem erro se já removido (idempotente via verificação `exists()`).

### Resume de Sessão na Bridge

Quando `start_zero_session` é chamado com um `session_id`, a bridge o armazena:

```rust
state.start(cwd, Some(session_id)).await
```

No primeiro `send()`, em vez de spawnar um `zero exec` simples, a bridge adiciona `--resume <sessionId>`:

```rust
if let Some(ref id) = resume_id {
    cmd.arg("--resume").arg(id);
}
```

Isso faz o zero carregar o contexto da sessão existente, então o modelo se lembra do histórico da conversa. O leitor de stdout ainda captura o `sessionId` do evento `run_start` para os turnos subsequentes.

## Frontend

### `zero-store.js` — Estado das Sessões

| Estado | Tipo | Descrição |
|---|---|---|
| `currentSessionId` | `string \| null` | ID da sessão atualmente visualizada. |
| `sessions` | `Array` | Lista de sessões do workspace ativo. |
| `messages` | `Array<{role, content, timestamp}>` | Mensagens do chat exibidas no `ChatView`. |
| `currentWorkspace` | `string` | Caminho do workspace ativo. |

### Ações

| Ação | Descrição |
|---|---|---|
| `loadSessions(cwd)` | Chama `listZeroSessions(cwd)` e armazena em `this.sessions`. Erros são silenciosamente ignorados. |
| `openSession(sessionId)` | Chama `loadSessionHistory(sessionId)`, mapeia a resposta para objetos `{role, content, timestamp}` e define `this.messages`. Define `this.currentSessionId`. |
| `removeSession(sessionId)` | Chama `deleteSession(sessionId)` (Rust remove o diretório da sessão do disco), reseta `currentSessionId` e mensagens se a sessão excluída estava ativa, então atualiza a lista de sessões. |
| `startSession(cwd, sessionId?)` | Reconecta a bridge com opção de resume da sessão. Limpa `messages`, define `currentWorkspace` e `currentSessionId`. |

### Atualização Automática

Quando um evento `run_end` chega (após o zero terminar de processar uma mensagem), a store atualiza automaticamente a lista de sessões:

```javascript
case 'run_end':
  // ... trata resposta em streaming ...
  if (this.currentWorkspace) {
    this.loadSessions(this.currentWorkspace)
  }
  break
```

Isso garante que sessões recém-criadas (do chat atual) apareçam no drawer imediatamente.

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
- **Título:** Usa `session.title` (da primeira mensagem do usuário) ou os últimos 8 caracteres de `session.session_id`.
- **Subtítulo:** `model_id` + data formatada (`DD/MM/AA HH:MM`).

### Fluxo ao Clicar na Sessão

```
Usuário clica no item da sessão
  → onSelectSession(session)
    → zeroStore.startSession(cwd, session.session_id)
        → Bridge: armazena resume_id, usará --resume no próximo send()
    → zeroStore.openSession(session.session_id)
        → loadSessionHistory(session_id)
        → mensagens do histórico populam this.messages
        → ChatView renderiza a conversa completa
    → zeroStore.loadSessions(cwd)
        → atualiza lista de sessões (ex: após mudanças externas)
```

### Exibição das Mensagens

Mensagens carregadas do histórico usam o mesmo componente `q-chat-message` das mensagens ao vivo:

| Papel | Nome | Fundo |
|---|---|---|
| `user` | "Você" | `primary` (azul) |
| `assistant` | "Zero" | `grey-3` (claro) ou `grey-9` (modo escuro) |
| `system` | "system" | `info` |
| `event` | "event" | `warning` |

O modo escuro adapta as cores das bolhas automaticamente via `$q.dark.isActive`.

## Testes

Testes de integração verificam o sistema de sessões ponta a ponta:

| Teste | Arquivo | Verifica |
|---|---|---|
| `test_sessions_list_filters_by_cwd` | `tests/zero_integration.rs` | Cria uma sessão em dir temporário, roda `zero sessions list --json`, valida que a sessão aparece filtrada por cwd. |
| `test_session_info_fields` | `tests/zero_integration.rs` | Valida que os campos `sessionId`, `createdAt`, `modelId` e `cwd` estão presentes e corretos. |
| `test_delete_session_removes_from_list` | `tests/zero_integration.rs` | Cria uma sessão, verifica que existe em disco e na lista, remove via `remove_dir_all`, verifica que não aparece mais na lista. |
| `test_message_history_recovery_from_events_jsonl` | `tests/zero_integration.rs` | Cria uma sessão com uma mensagem conhecida, lê `events.jsonl` do disco, valida que as mensagens de user + assistant estão presentes com os papéis corretos e verifica campos obrigatórios (`id`, `sessionId`, `createdAt`, `sequence`). |
| `test_multi_turn_context_preserved_with_resume` | `tests/zero_integration.rs` | Turno 1 define contexto ("nome é Alice"), turno 2 retoma via `--resume <id>` e pergunta "Qual é o meu nome?" — valida que "Alice" aparece. |

## Referências

- [Protocolo Stream-JSON do Zero](https://github.com/Gitlawb/zero/blob/main/docs/STREAM_JSON_PROTOCOL.md)
- [Arquitetura: Conexão](../architecture/connection.md)
- [Sistema de Workspaces](./workspace-system.md)
