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
5. Agrupa a lista plana já filtrada numa floresta via `build_session_tree` (ver "Vínculo de sessões de subagente" abaixo) e retorna só as sessões raiz, cada uma carregando seus descendentes em `children`.

**Estrutura SessionInfo:**

```rust
pub struct SessionInfo {
    pub session_id: String,   // ID único do zero
    pub title: String,        // título do zero-desktop (auto ou definido pelo usuário)
    pub created_at: String,   // timestamp ISO 8601
    pub cwd: String,          // diretório do workspace
    pub model_id: String,     // sobreposto do session-models.json
    pub event_count: Option<i64>,
    pub kind: String,         // "" | "fork" | "child" | "spec-draft" | "spec-impl"
    pub provider: String,     // ex: "openai-compatible"
    pub parent_session_id: String, // setado pelo motor quando esta sessão foi
                                    // criada via --calling-session-id (tool
                                    // Task / membro de swarm) ou fork/spec-impl
    pub root_session_id: String,   // o ancestral de topo definitivo
    pub agent_name: String,        // ex: "advisor" pra um filho da tool Task
    pub tag: String,               // "specialist" pra filhos de Task/swarm
    pub depth: i64,
    pub task_id: String,
    pub children: Vec<SessionInfo>, // NÃO vem do JSON do motor - preenchido
                                     // localmente por build_session_tree;
                                     // sempre um Vec, nunca ausente
}
```

### Vínculo de sessões de subagente

Toda vez que o agente chama a tool `Task` (inclusive uma consulta do Modo
Advisor, que por baixo dos panos é uma chamada `Task{name:"advisor",...}`) ou
spawna um membro de swarm/team, o motor zero cria uma sessão de verdade,
persistida, separada, com o mesmo `cwd` do pai — então ela aparecia em `zero
sessions list --json`, e por consequência na sidebar, como uma linha de topo
extra e indistinguível.

O motor já tagueia essas sessões (`kind: "child"`, `parentSessionId`,
`rootSessionId`, `agentName`, `tag: "specialist"`, `depth`) sempre que um
`--calling-session-id` esteve envolvido na criação — `list_zero_sessions`
agora captura esses campos em vez de descartá-los, e `build_session_tree`
(função privada em `lib.rs`) agrupa a lista já filtrada por cwd numa
floresta: uma sessão sem `parent_session_id`, ou cujo pai não está presente
nessa mesma lista filtrada (cwd diferente, ou já apagado), vira raiz; todo o
resto aninha sob seu pai via `children`, recursivamente. O aninhamento é
guiado só por `parent_session_id`, genérico entre todo `SessionKind`
(fork/child/spec-draft/spec-impl) — sem casos especiais por kind.

**Correção necessária no motor para cobertura completa**: filhos de
specialist via tool Task (inclusive Advisor) já eram tagueados corretamente
por `internal/sessions.PrepareExec` no `my-zero`. Membros de swarm/team não
eram — `internal/swarm/tools.go`'s `policyFrom` nunca propagava o session id
do orquestrador, então membros spawnados voltavam sem tag (`kind: ""`, sem
pai/raiz) apesar de passar pelo mesmo mecanismo de subprocesso. Corrigido
propagando `Policy.SessionID` → `MemberSpec.ParentSessionID` →
`specialist.TaskRunOptions.ParentSessionID` em
`internal/swarm/{team,member,tools,launcher_specialist}.go`, espelhando o que
`internal/specialist/task_tool.go` já faz pra tool `Task`.

**UI**: `src/components/SessionListItem.vue` é um componente recursivo (SFCs
Vue 3 podem se referenciar pelo próprio nome de arquivo sem registro extra)
que renderiza uma linha por sessão mais, quando `session.children.length >
0`, um toggle "N sessões de subagente" recolhido por padrão. Linhas
aninhadas mostram uma legenda de origem (`agent_name`, com fallback pra
`kind`). As cinco ações de linha (selecionar/renomear/excluir/status) são
fornecidas por `MainLayout.vue` via `provide("sessionListActions", {...})` e
consumidas com `inject(...)` em cada nível de recursão, em vez de
prop-drilling. A contagem de sessões da sidebar (`workspace.sessions`) conta
só as raízes.

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

### Divisão de stores (multi-sessão)

O estado das sessões está dividido em três stores (ver [ADR 004](../architecture/decisions/004-multi-session-parallel.md)):

| Store                      | Tipo                               | Estado principal                                                                                                                                                                                                                     |
| -------------------------- | ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `zero-store.js`            | Singleton global                   | `zeroPath`, `availableModels`, `activeModel`, `mcpBackends`                                                                                                                                                                          |
| `zero-session-store.js`    | Fábrica `useZeroSessionStore(key)` | `sessionKey`, `sessionId`, `cwd`, `messages[]`, `currentResponse`, `currentThinking`, `currentPlan`, `sessionMode`, `runInProgress`, `isConnected`                                                                                   |
| `session-runtime-store.js` | Singleton global                   | `openKeys[]`, `focusedKeyByPath{}` (foco por workspace), `keyMeta{}` (badges, cwd, título por chave). O limite de painéis (`MAX_OPEN_PANELS = 4`) é aplicado **por workspace**, não globalmente — ver `panelCountFor`/`canOpenMore`. |
| `workspaces-store.js`      | Singleton global                   | `workspaces[]`, `activePath`, `sessionsByPath{}`                                                                                                                                                                                     |

### Ações (session store)

| Ação                                         | Descrição                                                                                                                                                           |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `startSession(cwd, sessionId?)`              | Chama `startZeroSession(key, cwd, sessionId)`, define `this.sessionId` do `StartedSession` retornado, inicia o timer de sync de 3s, sincroniza metadata do runtime. |
| `openSession(sessionId)`                     | Chama `loadSessionHistory(sessionId)`, executa `buildMessagesFromHistory` para popular `this.messages`. Inicia o timer de sync de 3s.                               |
| `sendMessage(content, file?)`                | Chama `sendZeroMessage(key, content, file)`, define `runInProgress`, sincroniza metadata do runtime.                                                                |
| `cancelRun()`                                | Chama `cancelZeroRun(key)`.                                                                                                                                         |
| `switchModel(model)`                         | Chama `switchZeroModel(key, model)` — reinicia apenas esta sessão (decisão #6). Atualiza `globalStore.activeModel`.                                                 |
| `stopSession()`                              | Chama `stopZeroSession(key)`, remove listeners, para o timer de sync.                                                                                               |
| `respondToPermission(requestId, optionId)`   | Chama a API `respondToPermission`, atualiza o status da mensagem de permissão.                                                                                      |
| `removeSession(sessionId, onRefresh)`        | Chama `deleteSession(sessionId)`, para se ativa, chama `onRefresh`.                                                                                                 |
| `renameSession(sessionId, title, onRefresh)` | Chama `renameSession(sessionId, title)`, chama `onRefresh`.                                                                                                         |

### Reprodução de histórico (`buildMessagesFromHistory`)

O frontend normaliza eventos persistidos no mesmo formato de mensagem tipada usado para eventos ao vivo:

| Tipo de evento persistido  | Produz                                              |
| -------------------------- | --------------------------------------------------- |
| `message` (role=user)      | `{ type: "text", role: "user", content, file? }`    |
| `message` (role=assistant) | `{ type: "text", role: "assistant", content }`      |
| `reasoning`                | `{ type: "thinking", content }`                     |
| `tool_call`                | `{ type: "tool_call", toolName, toolUseId, input }` |
| `tool_result`              | Atualiza status + resultado do `tool_call`          |
| `permission_request`       | `{ type: "permission_request", answerable: false }` |
| `permission_decision`      | Atualiza status do `permission_request`             |
| `error`                    | `{ type: "error", content }`                        |

Pedidos de permissão do histórico são sempre `answerable: false` — o processo que perguntou já se foi. Se existir um evento `permission_decision` correspondente, o status do pedido é atualizado para `approved` ou `denied`; caso contrário, renderiza como expirado.

### Sincronização periódica (`_sessionSyncTimer`)

Enquanto uma sessão está aberta (e nenhum turno está em andamento), a store relê `loadSessionHistory` a cada 3 segundos. Se a contagem de eventos mudou, reconstrói a lista de mensagens do zero. Isso captura:

- Novos eventos escritos pelo bridge durante o turno atual.
- Mudanças externas na sessão de outra instância do zero-desktop.
- Eventos que chegaram tarde, após o `openSession` inicial ter completado.

O timer para quando a sessão é trocada ou fechada.

## Componentes de UI

### MainLayout.vue — Painel Direito

A lista de sessões itera `workspacesStore.sessionsByPath[activePath]` (não uma
store singleton). Cada item mostra um badge ao vivo quando a sessão está
processando (spinner para trabalhando, badge `!` para permissão pendente),
derivado de `sessionRuntime.keyMeta`.

### Grid de Painéis (Tiling)

`SessionTileGrid.vue` substitui o `<ChatView>` único na área de conteúdo
principal. Renderiza 1 (tela cheia), 2 (divisão horizontal), 3 (aninhado), ou 4
(grade 2×2) painéis baseado em `sessionRuntime.visibleKeys(workspacesStore.activePath).length`
— apenas os painéis do workspace ativo, não a lista `openKeys` do app inteiro —
usando `QSplitter` do Quasar para divisórias redimensionáveis.

Cada painel tem um `SessionPaneHeader.vue` com um único botão de fechar, que
chama `runtime.closePanel(key)`. Não existe mais uma ação manual separada de
"Parar" — `closePanel` se comporta condicionalmente: se um turno está em
execução, apenas esconde o painel (a sessão continua rodando em segundo
plano); se a sessão está ociosa, também para e descarta a sessão, liberando o
slot de painel do workspace. `runtime.stopAndDispose(key)` ainda existe como
uma parada incondicional, mas só é usado quando o usuário exclui a sessão
subjacente por completo (ver `onDeleteSession` em `MainLayout.vue`), não a
partir do botão de fechar do painel.

### Fluxo ao Clicar na Sessão

```
Usuário clica no item da sessão
  → onSelectSession(session)
    → openOrFocusSession(session.session_id, cwd, session.session_id)
      → runtime.openPanel(key)        — adiciona a openKeys, define focusedKeyByPath[cwd]
      → store.startSession(cwd, id)   — Bridge: inicia zero acp com session/load
                                        (ou reconecta se já estiver viva)
```

### Fluxo de Nova Sessão

```
Usuário clica em "Nova sessão"
  → onNewSession()
    → key = crypto.randomUUID()
    → openOrFocusSession(key, cwd, null)
      → runtime.openPanel(key)
      → store.startSession(cwd, null)  — Bridge: inicia zero acp com session/new
    → se mesmo cwd já tem sessão trabalhando, mostra aviso não-bloqueante
```

## Referências

- [Arquitetura de Conexão](../architecture/connection.md)
- [ADR 004: Sessões Paralelas](../architecture/decisions/004-multi-session-parallel.md)
- [Sistema de Workspaces](./workspace-system.md)
- [zero-bridge](./zero-bridge.md)
