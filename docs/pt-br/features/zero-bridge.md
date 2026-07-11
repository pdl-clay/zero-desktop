# zero-bridge: Conexão com o zero CLI

Este documento descreve a camada de conexão entre a GUI do zero-desktop e o zero CLI.

## Visão Geral

A conexão segue a arquitetura definida em [`docs/pt-br/architecture/connection.md`](../architecture/connection.md) e no [ADR 003](../architecture/decisions/003-migrate-to-acp.md):

- O backend Rust faz spawn de `zero acp` (Agent Client Protocol sobre stdio) - um processo por sessão ativa, mantido vivo durante toda a conversa.
- O frontend envia mensagens do usuário via commands do Tauri.
- O backend traduz as notificações `session/update` do ACP pro mesmo formato de evento que o frontend já renderiza, e transmite de volta via events do Tauri.
- Pedidos de permissão do agente (`session/request_permission`) são repassados pro frontend e respondidos de verdade pela mesma conexão JSON-RPC.

## Backend Rust

### Arquivos

- `src-tauri/src/locator.rs` — localiza o binário `zero` no PATH ou no cache isolado.
- `src-tauri/src/acp.rs` — peer JSON-RPC 2.0 minimalista feito à mão pro Agent Client Protocol (manda requisições, recebe requisições, recebe notificações - não é uma implementação só de cliente ou só de servidor).
- `src-tauri/src/bridge.rs` — `ZeroBridge`: mantém o processo `zero acp` por sessão, traduz eventos do ACP pro formato interno do app, e grava o log de histórico local.
- `src-tauri/src/lib.rs` — registra os commands e o estado do Tauri.

### Commands

| Command                 | Descrição                                                                                                |
| ----------------------- | -------------------------------------------------------------------------------------------------------- |
| `locate_zero_cli`       | Retorna o caminho e a versão do zero CLI.                                                                |
| `start_zero_session`    | Sobe o `zero acp` pro workspace informado e abre (ou carrega) uma sessão.                                |
| `send_zero_message`     | Manda um `session/prompt`, transmitindo progresso de volta via eventos.                                  |
| `respond_to_permission` | Responde um `session/request_permission` pendente com a opção escolhida.                                 |
| `cancel_zero_run`       | Mata o processo da sessão atual (não existe método `session/cancel`).                                    |
| `stop_zero_session`     | Para a sessão ativa.                                                                                     |
| `list_zero_sessions`    | Lista sessões de um workspace (`zero sessions list --json`).                                             |
| `load_session_history`  | Carrega o histórico de uma sessão - prefere o log local do zero-desktop, cai pro `events.jsonl` do zero. |
| `delete_session`        | Apaga os dados de uma sessão, incluindo o arquivo de histórico local do zero-desktop.                    |

### Events

| Evento                    | Descrição                                                                                             |
| ------------------------- | ----------------------------------------------------------------------------------------------------- |
| `zero:event`              | Um evento traduzido do ACP (`text`, `reasoning`, `tool_call`, `tool_result`, `run_end`, `error`).     |
| `zero:permission-request` | Um pedido de permissão de verdade do agente, aguardando resposta via `respond_to_permission`.         |
| `zero:stderr`             | Linha do stderr do processo zero (ou uma linha de stdout não interpretável, logada pra visibilidade). |
| `zero:process-exited`     | O stream de stdout do processo da sessão fechou.                                                      |

### Dependências

- `tokio` — runtime async e I/O de processos.
- `which` — localiza binários no PATH.
- `dirs` — resolve diretórios de dados específicos da plataforma (também usado pro log de histórico local).
- `thiserror` — tipos de erro.

Nenhuma crate de JSON-RPC foi adicionada - `acp.rs` implementa o framing delimitado por linha diretamente sobre `tokio` + `serde_json`, já que o ACP exige atuar tanto como remetente quanto receptor de requisições na mesma conexão, algo que a maioria das crates de JSON-RPC não suporta de forma limpa.

## Frontend

### Arquivos

- `src/services/zero.js` — envolve commands e listeners de eventos do Tauri.
- `src/stores/zero-store.js` — store Pinia para o estado do chat.
- `src/components/ChatView.vue` — contêiner principal do chat com renderização condicional.
- `src/components/chat/ChatInput.vue` — input de mensagem, indicador de status de trabalho, e checklist de plano fixado.
- `src/components/chat/TextMessage.vue` — mensagens de texto (usuário/assistente), renderizadas em markdown.
- `src/components/chat/ThinkingBlock.vue` — pensamento do modelo colapsável.
- `src/components/chat/ToolCallMessage.vue` — card de chamada de ferramenta com estados em execução/concluído/erro, uma visão de diff real pra `edit_file`, e uma visão de checklist pra `update_plan`.
- `src/components/chat/PendingPermissionPanel.vue` — fixado acima do input enquanto um pedido de permissão está pendente; renderiza as opções que o ACP realmente ofereceu (não um par fixo aprovar/negar).
- `src/components/chat/PermissionDecisionBadge.vue` — badge inline pra decisões automáticas informativas e pedidos de permissão já resolvidos no histórico.
- `src/components/chat/ErrorMessage.vue` — bolha de erro inline (ex: conexão perdida).
- `src/pages/IndexPage.vue` — ponto de entrada que renderiza o `ChatView`.

### Dependências

- `@tauri-apps/api` — API frontend do Tauri para commands e events.

### Eventos suportados

A store atualmente lida com, via `zero:event`:

- `text` (acumulado na resposta em streaming)
- `reasoning` (renderizado em blocos de pensamento colapsáveis)
- `tool_call` / `tool_result` (renderizado como cards estruturados com spinner/status; chamadas de `update_plan` são rastreadas separadamente e fixadas acima do input em vez de aparecer como card)
- `run_end`
- `error`

E, via o evento dedicado `zero:permission-request`, um pedido de permissão de verdade que `respondToPermission` responde.

## Limitações conhecidas (alpha)

- Não existe `session/cancel` no protocolo: cancelar um turno mata o processo daquela sessão; a próxima mensagem sobe o processo de novo e reconecta via `session/load`.
- Acesso à rede (ex: `web_fetch`) é negado pelo sandbox do próprio zero independente da permissão respondida - um limite rígido da política de sandbox atual, não algo que esse bridge controla.
- Sem interface com abas para múltiplos workspaces (apenas um workspace ativo por vez).

## Referências

- [Arquitetura: Conexão](../architecture/connection.md)
- [ADR 003: Migração para ACP](../architecture/decisions/003-migrate-to-acp.md)
- [Agent Client Protocol](https://agentclientprotocol.com)
