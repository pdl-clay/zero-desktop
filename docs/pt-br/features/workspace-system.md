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

Quando `activePath` muda (via `workspacesStore.select()`), um `watch` no `MainLayout.vue` dispara:

```
watch(activePath)
  → zeroStore.stopSession()      // desconecta do workspace anterior
  → zeroStore.startSession(cwd)  // conecta ao novo workspace
  → zeroStore.loadSessions(cwd)  // atualiza lista de sessões
```

A ação `startSession`:

1. Limpa as mensagens do workspace anterior.
2. Chama `setupListeners()` para anexar os listeners `zero:event` e `zero:stderr`.
3. Chama o comando Tauri `start_zero_session(cwd)` que informa a bridge Rust o diretório do workspace.
4. Define `isConnected = true`.

A primeira mensagem enviada após a seleção faz a bridge Rust spawnar `zero exec --cwd <path>`, e mensagens subsequentes usam `--resume <sessionId>` para a mesma sessão.

## Backend Rust

### Comandos Tauri

| Comando                                | Arquivo     | Descrição                                                               |
| -------------------------------------- | ----------- | ----------------------------------------------------------------------- |
| `locate_zero_cli`                      | `lib.rs:59` | Encontra o binário zero e retorna caminho + versão.                     |
| `start_zero_session(cwd, session_id?)` | `lib.rs:65` | Informa a bridge o workspace. `session_id` opcional para resume.        |
| `send_zero_message(content)`           | `lib.rs:73` | Envia uma mensagem do usuário. Bridge spawna `zero exec` se necessário. |
| `stop_zero_session`                    | `lib.rs:78` | Mata o processo zero atual e limpa o estado.                            |

### Estado da Bridge (`bridge.rs`)

O `ZeroBridge` mantém um `SessionState` por conexão ativa:

```rust
struct SessionState {
    cwd: PathBuf,                           // diretório do workspace
    session_id: Arc<Mutex<Option<String>>>, // capturado do run_start
    child: Option<Child>,                   // processo zero atual
}
```

- `start(cwd, resume_id)` — armazena o workspace e o ID de sessão para resume.
- `send(event)` — spawna um novo processo `zero exec` (com `--resume` se session_id estiver definido), escreve a mensagem e spawna leitores de stdout/stderr que emitem eventos Tauri.
- `stop()` — mata o processo filho e limpa o estado.

## Dependências

- **Seletor de pastas nativo:** `tauri-plugin-dialog` (Rust) + `@tauri-apps/plugin-dialog` (JS).
- **Persistência:** `localStorage` (sem dependências adicionais).
- **Cores dos avatares:** Função hash em JavaScript puro, sem biblioteca.

## Referências

- [Arquitetura: Conexão](../architecture/connection.md)
- [ADR 001: Conexão via stream-json](../architecture/decisions/001-connection-via-stream-json.md)
