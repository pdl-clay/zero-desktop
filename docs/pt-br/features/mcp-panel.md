# Painel MCP

Este documento descreve o painel de backends MCP (Model Context Protocol) — o drawer lateral direito que mostra os servidores MCP configurados, seu status de saúde, e os arquivos que o agente editou durante a sessão atual.

## Visão Geral

O painel MCP oferece visibilidade sobre:

- **Backends MCP**: todos os servidores configurados no zero (`zero backends --json`), com status de health-check ao vivo (OK/erro/desconhecido), tipo de transporte (stdio/http), e contagem de ferramentas.
- **Arquivos editados**: arquivos que o agente modificou via `edit_file`/`write_file` durante a sessão atual, com prévias de diff inline.
- **Cache de status**: persistido em disco para que o drawer renderize imediatamente com os últimos dados conhecidos, antes de qualquer verificação ao vivo completar.

O painel fica em um `q-drawer` no lado direito da visualização de chat, alternado por um botão flutuante na borda direita da tela.

## Fluxo de Dados

```
┌─────────────────────────────────┐
│  zero CLI                        │
│  zero backends --json            │
│  zero mcp check --json           │
│  zero mcp tools list --json      │
└────────────┬────────────────────┘
             │ lido pelo Rust (ao vivo)
┌────────────▼────────────────────┐
│  Dados locais do zero-desktop    │
│  ~/.local/share/zero-desktop/    │
│    mcp-status-cache.json         │
└────────────┬────────────────────┘
             │ lido pelo Rust (cache)
┌────────────▼────────────────────┐
│  Backend Rust (Tauri)            │
│  list_mcp_backends()             │
│    → zero backends --json        │
│    → overlay de status cached    │
│  check_mcp_backend(name)         │
│    → zero mcp check --json       │
│    → persiste no cache           │
│  load_mcp_status_cache()         │
│    → leitura bruta do cache      │
│  list_mcp_tools()                │
│    → zero mcp tools list --json  │
└────────────┬────────────────────┘
             │ IPC Tauri invoke
┌────────────▼────────────────────┐
│  Frontend (Pinia Store)          │
│  loadMcpBackends()               │
│  checkMcpBackend(name)           │
│  mcpBackends[], mcpTools[]       │
│  getter editedFiles              │
└────────────┬────────────────────┘
             │ bindings reativos
┌────────────▼────────────────────┐
│  McpDrawer.vue                   │
│  Cards de backend + arqs. edit.  │
└─────────────────────────────────┘
```

## Backend Rust

### `mcp_cache.rs`

Cache persistente em disco dos status de health-check dos backends MCP em `<app_data_dir>/zero-desktop/mcp-status-cache.json`.

**Estruturas de dados:**

```rust
pub struct CachedStatus {
    pub status: String,          // "ok" | "error"
    pub tool_count: i64,
    pub error: Option<String>,
    pub checked_at: Option<u64>, // timestamp unix (segundos)
}

pub struct McpStatusCache {
    pub servers: HashMap<String, CachedStatus>,
    pub generated_at: Option<u64>,
}
```

**Operações:**

| Função            | Descrição                                                        |
| ----------------- | ---------------------------------------------------------------- |
| `load()`          | Lê cache do disco, retorna cache vazio se ausente ou corrompido. |
| `save(cache)`     | Grava cache no disco, criando diretórios pai se necessário.      |
| `set_status(...)` | Atualiza entrada de um servidor e persiste.                      |
| `remove(name)`    | Remove um servidor do cache.                                     |
| `clear()`         | Esvazia o cache completamente.                                   |
| `get(name)`       | Retorna clone do status cached de um servidor, se existir.       |
| `all()`           | Retorna clone de todos os status cached.                         |

O cache usa `thread_local!` com caminho substituível para testes, permitindo que o código de teste aponte o cache para um diretório temporário sem interferir no cache real.

### Commands em `lib.rs`

| Command                    | Descrição                                                                                                            |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `list_mcp_backends`        | Lê `zero backends --json` e faz overlay de status/tool_count/error cached. Retorna `Vec<McpBackendInfo>`.            |
| `check_mcp_backend`        | Verifica ao vivo um servidor (`zero mcp check --json`), persiste resultado no cache, retorna `McpCheckResult`.       |
| `check_mcp_backend_cached` | Retorna status cached se presente; caso contrário, faz verificação ao vivo.                                          |
| `load_mcp_status_cache`    | Lê o cache bruto do disco para renderização inicial rápida. Retorna `McpStatusCache`.                                |
| `list_mcp_tools`           | Lista ferramentas de todos os servidores MCP habilitados (`zero mcp tools list --json`). Retorna `Vec<McpToolInfo>`. |

## Frontend

### `zero-store.js` — Estado MCP

| Estado         | Tipo    | Descrição                                            |
| -------------- | ------- | ---------------------------------------------------- |
| `mcpBackends`  | `Array` | Servidores MCP configurados com status de saúde.     |
| `mcpTools`     | `Array` | Todas as ferramentas dos servidores MCP habilitados. |
| `isLoadingMcp` | `bool`  | True enquanto backends estão sendo buscados.         |
| `_mcpLoaded`   | `bool`  | Guarda para evitar buscas repetidas.                 |

### Ações

| Ação                         | Descrição                                                                                                                   |
| ---------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `loadMcpBackends({ force })` | Carrega cache primeiro, depois busca backends + ferramentas em paralelo. Se `force` ou sem cache, faz verificações ao vivo. |
| `checkMcpBackend(name)`      | Verifica ao vivo um backend. Atualiza `mcpBackends[name]` com status/ferramentas/erro inline.                               |
| `checkAllMcpBackends()`      | Executa `checkMcpBackend` para todos os backends em paralelo.                                                               |
| `loadMcpTools({ force })`    | Atualiza apenas a lista global de ferramentas.                                                                              |

### Getter `editedFiles`

A store Pinia expõe um getter `editedFiles` que agrupa chamadas `edit_file`/`write_file` por caminho de arquivo, preservando a ordem de encontro e a ordem de edição por arquivo. Derivado puramente de `state.messages`.

O utilitário `isEditTool()` (de `src/utils/edit-tools.js`) reconhece:

- `edit_file` / `write_file` (nativos do zero)
- `*_edit_file` / `*_write_file` (variantes MCP, ex: `mcp_filesystem_edit_file`)

## Componente UI

### `McpDrawer.vue`

```
┌─────────────────────────────────┐
│  dns  Painel MCP          🔄  ✕ │  ← cabeçalho
│ ─────────────────────────────── │
│  ARQUIVOS EDITADOS               │
│  [📄 app.rs] [📄 lib.rs] [📄 C…]│  ← chips com contagem de edições
│  ┌─────────────────────────┐    │
│  │ lib.rs                  ✕ │    │  ← painel de diff expandido
│  │ - linha antiga           │    │
│  │ + linha nova             │    │
│  └─────────────────────────┘    │
│ ─────────────────────────────── │
│  ┌─ terminal  filesystem ──┐    │
│  │ stdio            ✓ ok   │    │  ← card de backend
│  └─────────────────────────┘    │
│  ┌─ language  brave-search ─┐   │
│  │ http     search.brave.com│    │  ← backend http
│  │               ⬤ ocioso  │    │
│  └─────────────────────────┘    │
│ ─────────────────────────────── │
│  Backends configurados em       │  ← dica do rodapé
│  ~/.config/zero/config.json     │
└─────────────────────────────────┘
```

**Comportamento:**

- **Colapsar/expandir**: No desktop (≥1024px), o botão alterna a largura do drawer entre 0 e 320px. No mobile, alterna a visibilidade do overlay via `modelValue`.
- **Strip de arquivos editados**: Linha horizontal scrollável de chips. Cada chip mostra o nome base do arquivo e um badge com a contagem de edições (se >1). Clique expande um painel de diff inline abaixo.
- **Painel de diff**: Renderiza cada edição como um bloco monoespaçado com linhas removidas em vermelho e adicionadas em verde, usando `getEditStrings` de `edit-tools.js`.
- **Cards de backend**: Cada card mostra nome do servidor, badge de tipo (stdio=laranja, http=azul), URL (truncada para hostname), e ícone de status.
- **Atualizar tudo**: O botão de refresh no cabeçalho chama `loadMcpBackends({ force: true })` depois `checkAllMcpBackends()`.
- **Arquivo expandido** reseta ao trocar de sessão.

## Estratégia de Cache

1. Na primeira abertura: `loadMcpStatusCache()` carrega o cache persistido imediatamente — o drawer mostra os últimos status conhecidos sem espera de rede.
2. Simultaneamente: `listMcpBackends()` busca o snapshot da config e faz overlay dos dados cached.
3. Se não existir cache algum: todos os backends são verificados ao vivo.
4. No refresh explícito (force=true): todos os backends são verificados ao vivo e o cache é atualizado.

## Referências

- [zero-bridge: Conexão com o zero CLI](./zero-bridge.md)
- [Arquitetura de Conexão](../architecture/connection.md)
- [Sistema de Sessões](./session-system.md)
