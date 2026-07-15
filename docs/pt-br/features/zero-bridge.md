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

| Command                 | Descrição                                                                                                |
| ----------------------- | -------------------------------------------------------------------------------------------------------- |
| `locate_zero_cli`       | Retorna o caminho e a versão do zero CLI.                                                                |
| `start_zero_session`    | Sobe o `zero acp` pro workspace informado e abre (ou carrega) uma sessão.                                |
| `send_zero_message`     | Manda um `session/prompt`, opcionalmente com anexo de arquivo, transmitindo progresso de volta via eventos. |
| `respond_to_permission` | Responde um `session/request_permission` pendente com a opção escolhida.                                 |
| `cancel_zero_run`       | Mata o processo da sessão atual (não existe método `session/cancel`). Session id e histórico são preservados; o próximo `send()` respawna e reconecta. |
| `stop_zero_session`     | Para a sessão ativa e limpa todo o estado.                                                               |

#### Commands de gerenciamento de sessão (via zero CLI)

| Command                 | Descrição                                                                                                |
| ----------------------- | -------------------------------------------------------------------------------------------------------- |
| `list_zero_sessions`    | Lista sessões de um workspace (`zero sessions list --json`, filtrado por `cwd`). Faz overlay dos títulos e modelos do zero-desktop. |
| `load_session_history`  | Carrega o histórico rico de uma sessão — prefere o log local do zero-desktop (`session-history/<id>.jsonl`), cai pro `events.jsonl` do zero. Retorna eventos tipados: `message`, `reasoning`, `tool_call`, `tool_result`, `permission_request`, `permission_decision`, `error`. |
| `delete_session`        | Apaga os dados de uma sessão: arquivo de histórico local do zero-desktop, overlays de título/modelo, e diretório de sessão do próprio zero. |
| `rename_session`        | Define (ou sobrescreve) o título de uma sessão no mapa de títulos local do zero-desktop. Usado tanto para títulos auto-derivados na primeira mensagem quanto para renomeações explícitas do usuário. |

#### Commands de arquivo

| Command                 | Descrição                                                                                                |
| ----------------------- | -------------------------------------------------------------------------------------------------------- |
| `read_file_attachment`  | Lê um arquivo do disco (até 10 MB), valida a extensão, detecta imagem vs. texto, rejeita binário em arquivos de texto, e retorna codificado em base64 com seu MIME type. Usado antes de anexar um arquivo a uma mensagem. |

#### Commands de modelo

| Command                 | Descrição                                                                                                |
| ----------------------- | -------------------------------------------------------------------------------------------------------- |
| `list_zero_models`      | Consulta o endpoint de listagem de modelos do provedor ativo via `zero providers models --json` e retorna a lista completa mais qual modelo está ativo. Não é instantâneo — uma chamada de rede real. |
| `switch_zero_model`     | Atualiza o modelo do provedor ativo globalmente via `zero providers add --model <x> --set-active`, depois mata o processo da sessão ao vivo para que a próxima mensagem capte a mudança. |

#### Commands MCP

| Command                    | Descrição                                                                                             |
| -------------------------- | ----------------------------------------------------------------------------------------------------- |
| `list_mcp_backends`        | Lê servidores MCP configurados na config do zero (`zero backends --json`) e faz overlay de status de saúde cached. |
| `check_mcp_backend`        | Verifica ao vivo um servidor MCP (`zero mcp check --json`): conecta, lista ferramentas, reporta status. Persiste o resultado no cache local. |
| `check_mcp_backend_cached` | Retorna o status cached de um servidor se presente; caso contrário, faz verificação ao vivo.          |
| `load_mcp_status_cache`    | Lê o cache bruto de status MCP do disco para renderização inicial rápida.                             |
| `list_mcp_tools`           | Lista todas as ferramentas expostas pelos servidores MCP habilitados (`zero mcp tools list --json`). Retorna `{ name, description }` para cada ferramenta. |

### Events

| Evento                    | Descrição                                                                                             |
| ------------------------- | ----------------------------------------------------------------------------------------------------- |
| `zero:event`              | Um evento ACP traduzido: `text`, `reasoning`, `tool_call`, `tool_result`, `plan_update`, `run_end`, `error`. |
| `zero:permission-request` | Um pedido de permissão real do agente, aguardando resposta via `respond_to_permission`.               |
| `zero:stderr`             | Uma linha do stderr do processo zero (ou linha de stdout não interpretável, logada pra visibilidade). |
| `zero:process-exited`     | O stream de stdout do processo da sessão fechou.                                                      |

#### Tipos de evento dentro de `zero:event`

| Tipo             | Descrição                                                                          |
| ---------------- | ------------------------------------------------------------------------------------ |
| `text`           | Delta de resposta do assistente em streaming (`{ delta: string }`).                  |
| `reasoning`      | Pedaço de pensamento do agente em streaming (`{ delta: string }`).                   |
| `tool_call`      | Agente iniciou uma chamada de ferramenta (`{ id, name, args }`).                     |
| `tool_result`    | Chamada de ferramenta concluída ou falhou (`{ id, status: "ok"\|"error", output }`). |
| `plan_update`    | Checklist do plano do agente atualizada (`{ entries: [{ content, status, priority }] }`). |
| `run_end`        | Turno finalizado (`{ status, stopReason }`).                                         |
| `error`          | Erro fatal do bridge (`{ message }`).                                                |

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
- `src/stores/zero-store.js` — store Pinia para estado do chat, gerenciamento de sessão, lista de modelos, backends MCP, modo de permissão, estado do plano, e sincronização de sessão.
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

A store Pinia (`zero-store.js`) gerencia:

- `messages[]` — lista tipada de mensagens (text, thinking, tool_call, permission_request, permission_decision, error).
- `currentResponse` / `currentThinking` — buffers de streaming finalizados em mensagens permanentes na próxima fronteira de evento.
- `currentPlan` — checklist atual do plano do agente (substituída por completo a cada `plan_update`).
- `activePlan` getter — retorna `null` quando todos os itens estão concluídos, auto-ocultando o painel.
- `editedFiles` getter — agrupa chamadas `edit_file`/`write_file` por caminho de arquivo, preservando ordem de encontro.
- `workingStatus` getter — retorna `thinking`, `{ type: "tool", toolName }`, `writing`, `sending`, ou `null`.
- `availableModels` / `activeModel` — populados por `list_zero_models` (chamada de rede ao provedor).
- `mcpBackends` / `mcpTools` — populados por `list_mcp_backends` + `list_mcp_tools`, com overlay de status cached.
- `permissionMode` — `"ask"` (padrão) ou `"auto_allow"` (auto-aprova pedidos de permissão).
- `_sessionSyncTimer` — releitura periódica (3s) do histórico enquanto uma sessão está aberta, capturando mudanças externas.

## Limitações conhecidas (alpha)

- Não existe `session/cancel` no protocolo: cancelar um turno mata o processo daquela sessão; a próxima mensagem sobe o processo de novo e reconecta via `session/load`.
- Acesso à rede (ex: `web_fetch`) é negado pelo sandbox do próprio zero independente da permissão respondida - um limite rígido da política de sandbox atual, não algo que esse bridge controla.
- Sem interface com abas para múltiplos workspaces (apenas um workspace ativo por vez).

## Referências

- [Arquitetura: Conexão](../architecture/connection.md)
- [ADR 003: Migração para ACP](../architecture/decisions/003-migrate-to-acp.md)
- [Agent Client Protocol](https://agentclientprotocol.com)
