# zero-bridge: Conexão com o zero CLI

Este documento descreve a implementação inicial da camada de conexão entre a GUI do zero-desktop e o zero CLI.

## Visão Geral

A conexão segue a arquitetura definida em [`docs/pt-br/architecture/connection.md`](../architecture/connection.md):

- O backend Rust faz spawn de `zero exec --input-format stream-json --output-format stream-json`.
- O frontend envia mensagens do usuário via commands do Tauri.
- O backend envia eventos JSONL de volta para o frontend via events do Tauri.

## Backend Rust

### Arquivos

- `src-tauri/src/locator.rs` — localiza o binário `zero` no PATH ou no cache isolado.
- `src-tauri/src/bridge.rs` — gerencia o processo filho e faz parse dos eventos stream-json.
- `src-tauri/src/lib.rs` — registra os commands e o estado do Tauri.

### Commands

| Command | Descrição |
|---|---|
| `locate_zero_cli` | Retorna o caminho e a versão do zero CLI. |
| `start_zero_session` | Inicia o `zero exec` no diretório de workspace informado. |
| `send_zero_message` | Envia uma mensagem do usuário para a sessão ativa. |
| `stop_zero_session` | Para a sessão ativa. |

### Events

| Evento | Descrição |
|---|---|
| `zero:event` | Evento de saída stream-json do zero. |
| `zero:stderr` | Linha do stderr do processo zero. |

### Dependências adicionadas

- `tokio` — runtime async e I/O de processos.
- `which` — localiza binários no PATH.
- `dirs` — resolve diretórios de dados específicos da plataforma.
- `thiserror` — tipos de erro.

## Frontend

### Arquivos

- `src/services/zero.js` — envolve commands e listeners de eventos do Tauri.
- `src/stores/zero-store.js` — store Pinia para o estado do chat.
- `src/components/ChatView.vue` — UI básica de chat.
- `src/pages/IndexPage.vue` — ponto de entrada que renderiza o `ChatView`.

### Dependências adicionadas

- `@tauri-apps/api` — API frontend do Tauri para commands e events.

### Eventos suportados

A store atualmente lida com:

- `run_start`
- `text` (acumulado na resposta em streaming)
- `final`
- `run_end`
- `error`
- `tool_call`, `tool_result`, `permission_request` (exibidos como mensagens de evento)

## Limitações conhecidas (alpha)

- Solicitações de permissão são exibidas como eventos brutos; aprovação interativa ainda não foi implementada.
- Sem interface com abas para múltiplos workspaces (apenas um workspace ativo por vez).

## Referências

- [Arquitetura: Conexão](../architecture/connection.md)
- [Zero Stream-JSON Protocol](https://github.com/Gitlawb/zero/blob/main/docs/STREAM_JSON_PROTOCOL.md)
