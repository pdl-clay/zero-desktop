# Troca de Modelo

Este documento descreve como o zero-desktop permite ao usuário trocar o modelo de IA ativo e como essa mudança se propaga pelo sistema.

## Visão Geral

O zero suporta múltiplos provedores e modelos de IA. O zero-desktop expõe a lista de modelos do provedor ativo e permite ao usuário trocar de modelo pela barra de input do chat. A troca de modelo é uma **mudança global e persistida na config do zero CLI** — o ACP não possui método de troca por sessão (`session/set_model` e `session/models` retornam "method not found"), então a troca afeta todo processo `zero` na máquina, não apenas a sessão atual.

## Fluxo de Dados

```
┌──────────────────────────────┐
│  Frontend                     │
│  ChatInput.vue                │
│  Dropdown seletor de modelo   │
│    → loadAvailableModels()    │
│    → switchModel(id)          │
└──────────────┬───────────────┘
               │ Tauri invoke
┌──────────────▼───────────────┐
│  Rust: list_zero_models       │
│    → active_provider_entry()  │
│      → zero config --json     │
│        → lê activeProvider    │
│        → lê provider.model    │
│    → zero providers models    │
│      <provider> --json        │
│        → chamada de rede ao   │
│          endpoint /v1/models  │
│    → retorna { models, active }│
└──────────────────────────────┘

┌──────────────────────────────┐
│  Rust: switch_zero_model      │
│    → active_provider_entry()  │
│    → zero providers add       │
│      <provider>               │
│      --name <provider>        │
│      --model <novo-modelo>    │
│      --set-active             │
│    → bridge.cancel()          │
│      (mata o processo vivo)   │
└──────────────┬───────────────┘
               │ próximo send() respawna
               │ com session/load,
               │ novo modelo entra em vigor
```

## Backend Rust

### `list_zero_models` (`bridge.rs` → `lib.rs`)

```
Comando Tauri: list_zero_models() → AvailableModels
```

**Etapas:**

1. `active_provider_entry()` executa `zero config --json` para encontrar o nome e modelo atual do provedor ativo.
2. Executa `zero providers models <provider> --json` — uma chamada de rede real ao endpoint `/v1/models` do provedor. Não é instantâneo, não é cached.
3. Faz parse dos campos `models[].id` para um array de strings.
4. Retorna `AvailableModels { models: Vec<String>, active: String }`.

**Detalhes de active_provider_entry():**

- Faz parse da saída de `zero config --json`.
- Lê `activeProvider` (ex: `"opencode-go"`) e o campo `model` do provedor correspondente.
- O `name` do provedor serve como `<catalog-id>` para `zero providers add` — verificado ao vivo que atualizar um perfil existente com o mesmo `--name` atualiza no lugar, sem criar duplicata.

### `switch_zero_model` (`bridge.rs` → `lib.rs`)

```
Comando Tauri: switch_zero_model(key: String, model: String) → ()
```

**Etapas:**

1. Resolve o nome do provedor ativo via `active_provider_entry()`.
2. Executa `zero providers add <provider> --name <provider> --model <model> --set-active` — atualiza o modelo na config do zero. Essa parte é global — muda o arquivo de config que todo processo `zero` lê.
3. Chama `bridge.cancel(key)` para matar **apenas** o processo `zero acp` vivo da sessão identificada por `key` (ver [ADR 004](../architecture/decisions/004-multi-session-parallel.md), decisão nº 6). O session id e histórico são preservados, e qualquer outra sessão aberta continua rodando com o modelo que já tinha snapshotado.
4. No próximo `send()` para essa mesma `key`, o bridge respawna o processo e re-snapshota o modelo no `session-models.json` — o novo modelo entra em vigor no próximo turno dessa sessão.

**Por que matar o processo?** O ACP não tem método para trocar de modelo no meio da sessão. A única forma de um processo `zero acp` em execução captar uma mudança de config é reiniciá-lo. Matar e respawnar via `session/load` é efetivamente uma reconexão de sessão com o novo modelo.

### Snapshot de modelo por sessão

Após todo handshake bem-sucedido (`session/new`, `session/load`, ou fallback), o bridge faz snapshot do modelo ativo:

```rust
if let Some(model_id) = active_model_id().await {
    let _ = set_session_model(&session_id, &model_id);
}
```

Isso é armazenado em `~/.local/share/zero-desktop/session-models.json` e sobreposto na saída de `list_zero_sessions`, já que o ACP reporta `modelId` vazio no `zero sessions list --json`. O snapshot acontece após **todo** handshake, não apenas `session/new`, então uma troca de modelo no meio da sessão captura o novo modelo corretamente.

## Frontend

### `zero-store.js` — Estado de Modelo

| Estado            | Tipo       | Descrição                                  |
| ----------------- | ---------- | ------------------------------------------ |
| `availableModels` | `string[]` | Lista de IDs de modelos do provedor ativo. |
| `activeModel`     | `string`   | ID do modelo atualmente ativo.             |
| `isLoadingModels` | `bool`     | True enquanto busca modelos.               |
| `_modelsLoaded`   | `bool`     | Guarda para evitar buscas repetidas.       |

### Ações

| Ação                             | Descrição                                                                              |
| -------------------------------- | -------------------------------------------------------------------------------------- |
| `loadAvailableModels({ force })` | Chama `listZeroModels()`. Cache no `_modelsLoaded`; passe `force: true` para rebuscar. |

O próprio `switchModel(model)` **não** é uma ação da `zero-store.js` — desde o chat paralelo multi-sessão (ADR 004), ele vive na store por sessão `zero-session-store.js` (`useZeroSessionStore(key)`). Ele guarda contra no-op (mesmo modelo) e run em andamento, chama `switchZeroModel(key, model)` para reiniciar apenas o processo daquela sessão, atualiza o `activeModel` próprio da sessão, e também atualiza o `activeModel` da store global para que qualquer painel ainda não conectado adote o novo padrão na primeira conexão.

### `ChatInput.vue` — Seletor de Modelo

O seletor de modelo é um dropdown na barra de input que mostra:

- **Modelo atual** como rótulo do botão (truncado com ellipsis para nomes longos).
- **Campo de busca/filtro** no topo do dropdown.
- **Indicador de ativo** (check ou ponto) ao lado do modelo atualmente ativo.
- **Lista de modelos** em área scrollável.

O seletor fica desabilitado enquanto `runInProgress` é true, já que o modelo só entra em vigor no próximo turno.

## Considerações de UX

- **Lista de modelos é chamada de rede**: `zero providers models` consulta a API ao vivo do provedor. Na primeira abertura pode demorar; aberturas subsequentes usam a lista cached.
- **Troca é global**: Mudar o modelo afeta toda invocação do `zero` na máquina, incluindo uso CLI fora do zero-desktop.
- **Troca durante execução é bloqueada**: O seletor fica desabilitado enquanto um turno está em andamento.
- **Histórico preserva o modelo**: `session-models.json` registra qual modelo respondeu cada sessão.

## Referências

- [zero-bridge: Conexão com o zero CLI](./zero-bridge.md)
- [Arquitetura de Conexão](../architecture/connection.md)
- [Sistema de Sessões](./session-system.md)
