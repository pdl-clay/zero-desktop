# Sistema de Workspaces

Este documento descreve como o zero-desktop gerencia diretórios de projeto (workspaces).

## Visão Geral

Workspaces são diretórios no sistema de arquivos do usuário onde o agente de código zero opera. Cada workspace é uma pasta de projeto — o zero lê, escreve e executa comandos dentro desse diretório.

O sistema de workspaces oferece:

- Lista persistente de workspaces entre reinicializações via `localStorage`.
- Seletor de pastas nativo para adicionar novos workspaces.
- Avatares visuais com cores determinísticas baseadas no nome do diretório.
- Conexão automática ao zero quando um workspace é selecionado.
- Listagem de sessões com escopo do workspace (sessões filtradas por `cwd`).

## Modelo de Dados

### `workspaces-store.js` (Pinia)

**Arquivo:** `src/stores/workspaces-store.js`

| Estado       | Tipo                             | Descrição                                    |
| ------------ | -------------------------------- | -------------------------------------------- |
| `workspaces` | `Array<{ path, name, addedAt }>` | Todos os workspaces salvos.                  |
| `activePath` | `string \| null`                 | Caminho do workspace atualmente selecionado. |

| Ação           | Descrição                                                                                                                                                           |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `add(path)`    | Normaliza o caminho, extrai o nome do diretório, deduplica, insere no array, salva no `localStorage`. Se nenhum workspace estiver ativo, seleciona automaticamente. |
| `remove(path)` | Filtra o caminho, salva no `localStorage`. Se o workspace removido estava ativo, seleciona o primeiro restante (ou define `null`).                                  |
| `select(path)` | Define `activePath`.                                                                                                                                                |

| Getter      | Descrição                                                           |
| ----------- | ------------------------------------------------------------------- |
| `active`    | Retorna o objeto completo do workspace para `activePath` ou `null`. |
| `hasActive` | `true` se um workspace está selecionado.                            |

### Persistência

Workspaces são armazenados no `localStorage` sob a chave `zero-desktop-workspaces`. Formato JSON:

```json
[
  {
    "path": "/home/user/meu-projeto",
    "name": "meu-projeto",
    "addedAt": 1752019200000
  }
]
```

Chamadas de persistência:

- `loadWorkspaces()` — chamada na criação da store (síncrona, não bloqueia).
- `saveWorkspaces()` — chamada após cada `add()` e `remove()`.

## Componentes de UI

### MainLayout.vue — Drawer (coluna esquerda)

```
┌──────┐
│  🎯  │  logo do zero
│ ──── │
│ [M]╳ │  meu-projeto (ativo)
│ [T]╳ │  teste
│  [+] │  adicionar workspace (abre seletor de pastas nativo)
│ ──── │
│  ⚙   │  configurações
│  ☀   │  alternar modo claro/escuro
└──────┘
```

- **Avatar:** Um `<div>` circular estilizado com `border-radius: 50%`. A cor de fundo é derivada de um hash do nome do diretório (10 cores predefinidas). A letra é o primeiro caractere do nome do diretório, em maiúscula. O workspace ativo recebe tamanho maior (40px vs 34px) e um anel duplo via `box-shadow`.
- **Botão de remoção:** Um pequeno ícone `X` (`q-btn round size="xs"`) posicionado no canto inferior direito do avatar. Oculto por padrão (`opacity: 0; transform: scale(0.4)`), aparece no hover com animação de escala.
- **Botão de adicionar:** Um ícone `+` (`q-btn round icon="add"`) que abre o seletor de pastas nativo diretamente — sem diálogo, sem confirmação. Clique → explorador de arquivos nativo abre → seleciona pasta → workspace adicionado.
- **Tooltip:** Cada avatar tem um tooltip mostrando o nome do diretório (negrito) e o caminho completo.
- **Modo escuro:** O fundo da coluna esquerda adapta via `:class="$q.dark.isActive ? 'bg-dark' : 'bg-grey-3'"`.

### Adicionando um Workspace

Fluxo:

1. Usuário clica no botão `+`.
2. `onBrowseAndAdd()` chama `open({ directory: true, multiple: false })` do `@tauri-apps/plugin-dialog`.
3. O seletor de pastas nativo do sistema abre (GTK no Linux, Finder no macOS, Explorer no Windows).
4. Se uma pasta for selecionada, `workspacesStore.add(selectedPath)` é chamado imediatamente.
5. A store normaliza o caminho, deduplica, salva no `localStorage` e seleciona automaticamente se for o primeiro workspace.

### Seleção de Workspace

Desde o chat paralelo multi-sessão (ver [ADR 004](../architecture/decisions/004-multi-session-parallel.md)), trocar de workspace não mata nem inicia mais nenhuma sessão — é navegação pura. Um `watch` em `workspacesStore.activePath` no `MainLayout.vue` apenas atualiza a lista de sessões do workspace recém-ativo:

```
watch(activePath)
  → workspacesStore.loadSessions(newPath)  // atualiza a lista de sessões do novo workspace
```

Painéis pertencentes a outros workspaces continuam rodando em segundo plano (até o limite por workspace de `MAX_OPEN_PANELS`); trocar de workspace só muda quais painéis o `SessionTileGrid.vue` renderiza, via `sessionRuntime.visibleKeys(workspacePath)`.

Selecionar um workspace também abre ou foca uma sessão para ele, via `onSelectWorkspace()`:

1. `workspacesStore.select(ws.path)` define `activePath`.
2. Se ainda não houver um painel aberto para o `cwd` desse workspace, uma nova chave é gerada e `openOrFocusSession(key, ws.path, null)` abre um painel para ele.
3. Abrir um painel apenas o prepara (carrega o histórico se estiver retomando); o processo `zero acp` real só é spawnado de forma preguiçosa, na primeira vez que o usuário envia uma mensagem (ver [Sistema de Sessões](./session-system.md)).

## Backend Rust

### Comandos Tauri

| Comando                                     | Arquivo  | Descrição                                                                                                  |
| ------------------------------------------- | -------- | ---------------------------------------------------------------------------------------------------------- |
| `locate_zero_cli`                           | `lib.rs` | Encontra o binário zero e retorna caminho + versão.                                                        |
| `start_zero_session(key, cwd, session_id?)` | `lib.rs` | Spawna `zero acp` para a chave de roteamento + workspace dados. `session_id` opcional para `session/load`. |
| `send_zero_message(key, content, file?)`    | `lib.rs` | Envia uma mensagem do usuário (com anexo opcional) para a sessão identificada por `key`.                   |
| `stop_zero_session(key)`                    | `lib.rs` | Mata o processo de `key` e limpa seu estado de sessão.                                                     |
| `cancel_zero_run(key)`                      | `lib.rs` | Mata o processo de `key`, mas preserva o id de sessão/histórico para reconexão.                            |
| `switch_zero_model(key, model)`             | `lib.rs` | Atualiza o modelo ativo globalmente e mata apenas a sessão identificada por `key`.                         |
| `list_zero_sessions(cwd)`                   | `lib.rs` | Lista sessões filtradas pelo diretório do workspace.                                                       |

### Estado da Bridge (`bridge.rs`)

`ZeroBridge.sessions` é um `HashMap<String, AcpSession>` indexado por uma chave
de roteamento pertencente ao frontend (UUID para sessões novas, `session_id`
para as retomadas) — não por workspace. Um único workspace pode ter vários
`AcpSession` ativos ao mesmo tempo (um por painel aberto), cada um independente
dos demais:

```rust
struct AcpSession {
    cwd: PathBuf,             // diretório do workspace
    session_id: String,       // capturado da resposta de session/new ou session/load
    history_path: PathBuf,    // arquivo de histórico local do zero-desktop para esta sessão
    live: Option<LiveProcess>, // o processo filho zero acp em execução + AcpPeer
}
```

- `start(key, cwd, resume_id)` — spawna um novo `zero acp` para essa chave, completa o handshake `initialize` e abre a sessão (`session/new` ou `session/load`). Não afeta a sessão de nenhuma outra chave.
- `send(key, content, file?)` — persiste a mensagem do usuário no histórico local, depois dispara `session/prompt` em uma tarefa em segundo plano. Retorna imediatamente; o progresso é transmitido via `zero:event`.
- `cancel(key)` — mata o processo ativo dessa chave, mas mantém `session_id` e `history_path` para que o próximo `send()` reconecte via `session/load`.
- `stop(key)` — mata o processo dessa chave e limpa seu estado de sessão.

## Dependências

- **Seletor de pastas nativo:** `tauri-plugin-dialog` (Rust) + `@tauri-apps/plugin-dialog` (JS).
- **Persistência:** `localStorage` (sem dependências adicionais).
- **Cores dos avatares:** Função hash em JavaScript puro, sem biblioteca.

## Referências

- [Arquitetura: Conexão](../architecture/connection.md)
- [Sistema de Sessões](./session-system.md)
