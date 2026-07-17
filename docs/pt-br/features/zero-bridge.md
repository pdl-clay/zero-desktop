# zero-bridge: Conexão com o zero CLI

Este documento descreve a camada de conexão entre a GUI do zero-desktop e o zero CLI.

## Visão Geral

A conexão segue a arquitetura definida em [`docs/pt-br/architecture/connection.md`](../architecture/connection.md) e no [ADR 003](../architecture/decisions/003-migrate-to-acp.md):

- O backend Rust faz spawn de `zero acp` (Agent Client Protocol sobre stdio) - um processo por sessão ativa, mantido vivo durante toda a conversa.
- O frontend envia mensagens do usuário (com anexos de arquivo opcionais) via commands do Tauri.
- O backend traduz as notificações `session/update` do ACP pro mesmo formato de evento que o frontend já renderiza, e transmite de volta via events do Tauri.
- Pedidos de permissão do agente (`session/request_permission`) são repassados pro frontend e respondidos de verdade pela mesma conexão JSON-RPC.
- O backend também faz proxy de várias consultas somente leitura do zero CLI (sessões, modelos, backends/ferramentas MCP) que são independentes de qualquer sessão ao vivo.

## Backend Rust

### Arquivos

- `src-tauri/src/locator.rs` — localiza o binário `zero` no PATH ou no cache isolado.
- `src-tauri/src/acp.rs` — peer JSON-RPC 2.0 minimalista feito à mão pro Agent Client Protocol (manda requisições, recebe requisições, recebe notificações - não é uma implementação só de cliente ou só de servidor).
- `src-tauri/src/bridge.rs` — `ZeroBridge`: mantém o processo `zero acp` por sessão, traduz eventos do ACP pro formato interno do app, grava o log de histórico local da sessão, e gerencia overlays de título/modelo por sessão.
- `src-tauri/src/mcp_cache.rs` — cache persistente em disco dos status de health-check dos backends MCP, para que o drawer renderize imediatamente com os últimos dados conhecidos antes das verificações ao vivo completarem.
- `src-tauri/src/lib.rs` — registra os commands e o estado do Tauri, além de todos os tipos de estado (`SessionInfo`, `SessionEvent`, `FileAttachment`, `McpBackendInfo`, `McpCheckResult`, `McpToolInfo`).

### Commands

#### Commands de sessão (via `zero acp`)

Todos os commands de sessão aceitam um `key: String` — a chave de roteamento do
frontend (UUID para novas sessões, `session_id` para retomadas). Eventos carregam
a mesma chave para o frontend rotear ao painel correto.

| Command                 | Descrição                                                                                                                                      |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `locate_zero_cli`       | Retorna o caminho e a versão do zero CLI.                                                                                                      |
| `start_zero_session`    | Sobe o `zero acp` para a chave + workspace informado e abre (ou carrega) uma sessão. Retorna `StartedSession { key, sessionId, reattached }`.  |
| `send_zero_message`     | Manda um `session/prompt` para a sessão identificada por `key`, opcionalmente com anexo, transmitindo progresso via eventos.                   |
| `respond_to_permission` | Responde um `session/request_permission` pendente. Roteado internamente pelo `session_key` armazenado no pedido pendente.                      |
| `cancel_zero_run`       | Mata o processo da sessão para `key` (não existe `session/cancel`). Session id e histórico preservados; próximo `send()` respawna e reconecta. |
| `stop_zero_session`     | Para a sessão para `key` e remove seu registro do bridge.                                                                                      |
| `list_live_sessions`    | Retorna `Vec<LiveSessionInfo { key, sessionId, cwd, live }>` de todas as sessões rastreadas — usado pelo frontend para reconciliar estado.     |

#### Commands de gerenciamento de sessão (via zero CLI)

| Command                | Descrição                                                                                                                                                                                                                                                                       |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `list_zero_sessions`   | Lista sessões de um workspace (`zero sessions list --json`, filtrado por `cwd`). Faz overlay dos títulos e modelos do zero-desktop.                                                                                                                                             |
| `load_session_history` | Carrega o histórico rico de uma sessão — prefere o log local do zero-desktop (`session-history/<id>.jsonl`), cai pro `events.jsonl` do zero. Retorna eventos tipados: `message`, `reasoning`, `tool_call`, `tool_result`, `permission_request`, `permission_decision`, `error`. |
| `delete_session`       | Apaga os dados de uma sessão: arquivo de histórico local do zero-desktop, overlays de título/modelo, e diretório de sessão do próprio zero.                                                                                                                                     |
| `rename_session`       | Define (ou sobrescreve) o título de uma sessão no mapa de títulos local do zero-desktop. Usado tanto para títulos auto-derivados na primeira mensagem quanto para renomeações explícitas do usuário.                                                                            |

#### Commands de arquivo

| Command                | Descrição                                                                                                                                                                                                                 |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `read_file_attachment` | Lê um arquivo do disco (até 10 MB), valida a extensão, detecta imagem vs. texto, rejeita binário em arquivos de texto, e retorna codificado em base64 com seu MIME type. Usado antes de anexar um arquivo a uma mensagem. |

#### Commands de modelo

| Command             | Descrição                                                                                                                                                                                             |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `list_zero_models`  | Consulta o endpoint de listagem de modelos do provedor ativo via `zero providers models --json` e retorna a lista completa mais qual modelo está ativo. Não é instantâneo — uma chamada de rede real. |
| `switch_zero_model` | Atualiza o modelo do provedor ativo globalmente via `zero providers add --model <x> --set-active`, depois mata apenas a sessão identificada por `key`. Outras sessões vivas não são afetadas.         |

#### Commands MCP

| Command                    | Descrição                                                                                                                                                  |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `list_mcp_backends`        | Lê servidores MCP configurados na config do zero (`zero backends --json`) e faz overlay de status de saúde cached.                                         |
| `check_mcp_backend`        | Verifica ao vivo um servidor MCP (`zero mcp check --json`): conecta, lista ferramentas, reporta status. Persiste o resultado no cache local.               |
| `check_mcp_backend_cached` | Retorna o status cached de um servidor se presente; caso contrário, faz verificação ao vivo.                                                               |
| `load_mcp_status_cache`    | Lê o cache bruto de status MCP do disco para renderização inicial rápida.                                                                                  |
| `list_mcp_tools`           | Lista todas as ferramentas expostas pelos servidores MCP habilitados (`zero mcp tools list --json`). Retorna `{ name, description }` para cada ferramenta. |

### Events

Todos os eventos carregam `sessionKey` no payload para o frontend rotear ao
painel/store correto. Listeners filtram por `payload.sessionKey`.

| Evento                    | Payload                                                | Descrição                                                                                                    |
| ------------------------- | ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------ |
| `zero:event`              | `{ schemaVersion, type, ...payload, sessionKey }`      | Um evento ACP traduzido: `text`, `reasoning`, `tool_call`, `tool_result`, `plan_update`, `run_end`, `error`. |
| `zero:permission-request` | `{ requestId, toolName, reason, options, sessionKey }` | Um pedido de permissão real do agente, aguardando resposta via `respond_to_permission`.                      |
| `zero:stderr`             | `{ sessionKey, line }`                                 | Uma linha do stderr do processo zero (ou linha de stdout não interpretável, logada pra visibilidade).        |
| `zero:process-exited`     | `{ sessionKey }`                                       | O stream de stdout do processo da sessão fechou.                                                             |

#### Tipos de evento dentro de `zero:event`

| Tipo          | Descrição                                                                                 |
| ------------- | ----------------------------------------------------------------------------------------- |
| `text`        | Delta de resposta do assistente em streaming (`{ delta: string }`).                       |
| `reasoning`   | Pedaço de pensamento do agente em streaming (`{ delta: string }`).                        |
| `tool_call`   | Agente iniciou uma chamada de ferramenta (`{ id, name, args }`).                          |
| `tool_result` | Chamada de ferramenta concluída ou falhou (`{ id, status: "ok"\|"error", output }`).      |
| `plan_update` | Checklist do plano do agente atualizada (`{ entries: [{ content, status, priority }] }`). |
| `run_end`     | Turno finalizado (`{ status, stopReason }`).                                              |
| `error`       | Erro fatal do bridge (`{ message }`).                                                     |

### Dependências

- `tokio` — runtime async e I/O de processos.
- `which` — localiza binários no PATH.
- `dirs` — resolve diretórios de dados específicos da plataforma (também usado para todos os caches e histórico locais).
- `thiserror` — tipos de erro.
- `base64` — codifica/decodifica anexos de arquivo.

Nenhuma crate de JSON-RPC foi adicionada - `acp.rs` implementa o framing delimitado por linha diretamente sobre `tokio` + `serde_json`, já que o ACP exige atuar tanto como remetente quanto receptor de requisições na mesma conexão, algo que a maioria das crates de JSON-RPC não suporta de forma limpa.

## Frontend

### Arquivos

- `src/services/zero.js` — envolve todos os commands e listeners de eventos do Tauri.
- `src/stores/zero-store.js` — store Pinia global: lista de modelos, backends MCP, modo de permissão. O estado de chat por sessão (mensagens, plano, sincronização de sessão) vive na `zero-session-store.js` — ver "Arquitetura da store" abaixo.
- `src/components/ChatView.vue` — contêiner principal do chat com renderização condicional.
- `src/components/chat/ChatInput.vue` — input de mensagem com botão anexar, toggle de modo de permissão, dropdown seletor de modelo, checklist de plano inline, indicador de status, e botão cancelar.
- `src/components/chat/TextMessage.vue` — mensagens de texto (usuário/assistente), renderizadas em markdown.
- `src/components/chat/ThinkingBlock.vue` — pensamento do modelo colapsável.
- `src/components/chat/ToolCallMessage.vue` — card de chamada de ferramenta com estados em execução/concluído/erro, visão de diff real pra `edit_file`, e visão de checklist pra `update_plan`.
- `src/components/chat/PendingPermissionPanel.vue` — fixado acima do input enquanto um pedido de permissão está pendente; renderiza as opções que o ACP realmente ofereceu (não um par fixo aprovar/negar).
- `src/components/chat/PermissionDecisionBadge.vue` — badge inline pra decisões automáticas informativas e pedidos de permissão já resolvidos no histórico.
- `src/components/chat/ErrorMessage.vue` — bolha de erro inline (ex: conexão perdida).
- `src/components/chat/PlanPanel.vue` — painel independente renderizando o plano atual; também embutido inline no `ChatInput.vue`.
- `src/components/McpDrawer.vue` — painel lateral direito: cards de backend MCP com status de saúde ao vivo, strip de arquivos editados com prévias de diff inline.
- `src/pages/IndexPage.vue` — ponto de entrada que renderiza o `ChatView`.

### Dependências

- `@tauri-apps/api` — API frontend do Tauri para commands e events.
- `pinia` — gerenciamento de estado.
- `vue-i18n` — internacionalização.

### Eventos suportados

A store lida com, via `zero:event`:

- `text` (acumulado na resposta em streaming)
- `reasoning` (renderizado em blocos de pensamento colapsáveis)
- `tool_call` / `tool_result` (renderizado como cards estruturados com spinner/status; chamadas de `update_plan` são rastreadas separadamente e fixadas acima do input em vez de aparecer como card)
- `plan_update` (substitui `currentPlan` na store; renderizado inline no `ChatInput.vue`)
- `run_end`
- `error`

E, via o evento dedicado `zero:permission-request`, um pedido de permissão de verdade que `respondToPermission` responde.

### Arquitetura da store

As stores Pinia estão divididas em três camadas (ver [ADR 004](../architecture/decisions/004-multi-session-parallel.md)):

- **`zero-store.js`** (global, singleton) — `zeroPath`, `availableModels`,
  `activeModel`, `mcpBackends`, `mcpTools`. Apenas estado app-wide.
- **`zero-session-store.js`** (fábrica, `useZeroSessionStore(key)`) — estado por
  sessão: `messages[]`, `currentResponse`, `currentThinking`, `currentPlan`,
  `runInProgress`, listeners (filtrados por `sessionKey`). Getters:
  `workingStatus`, `activePlan`, `editedFiles`.
- **`session-runtime-store.js`** (orquestrador) — `openKeys` (ordem de
  exibição), `focusedKeyByPath` (foco rastreado por workspace, não uma única
  chave global), `keyMeta` (metadata por chave para badges). O limite de 4
  painéis (`MAX_OPEN_PANELS`) é aplicado **por workspace**, não globalmente.
  Actions: `openPanel`, `closePanel` (esconde enquanto um turno está em
  execução; para e descarta quando ocioso — não existe mais uma ação manual
  separada de "Parar"), `stopAndDispose` (parada incondicional, usada ao
  excluir uma sessão), `openOrFocusSession` (entry point usado pela UI).

`ChatView.vue` cria uma session store para sua prop `sessionKey` e faz
`provide("zeroStore", store)` para os componentes filhos. `ChatInput.vue` usa a
session store injetada (para `switchModel` — por sessão) e a global store (para
lista de modelos, permission mode). `McpDrawer.vue` lê `editedFiles` da session
store focada.

## Limitações conhecidas (alpha)

- Não existe `session/cancel` no protocolo: cancelar um turno mata o processo daquela sessão; a próxima mensagem sobe o processo de novo e reconecta via `session/load`.
- Acesso à rede (ex: `web_fetch`) é negado pelo sandbox do próprio zero independente da permissão respondida - um limite rígido da política de sandbox atual, não algo que esse bridge controla.
- Sessões concorrentes editando os mesmos arquivos (mesmo workspace) podem causar corridas de escrita — um aviso não-bloqueante é mostrado, mas nenhum lock a nível de arquivo é enforceado.

## Referências

- [Arquitetura: Conexão](../architecture/connection.md)
- [ADR 003: Migrar para ACP](../architecture/decisions/003-migrate-to-acp.md)
- [ADR 004: Sessões Paralelas](../architecture/decisions/004-multi-session-parallel.md)
- [Agent Client Protocol](https://agentclientprotocol.com)
