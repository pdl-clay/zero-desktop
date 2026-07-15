# Painel de Terminal

Este documento descreve o emulador de terminal embutido — um painel encaixável na parte inferior da tela onde o usuário pode rodar processos de shell reais (servidores de dev, build tools, git, qualquer coisa) sem sair do app, e citar a saída de um terminal diretamente em um painel de chat focado para mostrar um erro ao agente.

## Visão Geral

O painel de terminal oferece:

- **Shells reais, não um console falso**: cada aba inicia um processo de shell real com PTY (o `$SHELL` do próprio usuário, como shell de login), então prompts, cores, controle de jobs e programas interativos (`vim`, `htop`, REPLs) funcionam exatamente como em um terminal nativo.
- **Multitarefa estilo abas de navegador**: abra/feche quantas abas de terminal forem necessárias; cada uma é um shell independente.
- **Escopo por workspace**: as abas de terminal pertencem ao workspace em que foram abertas (mesmo modelo dos painéis de sessão de chat) — trocar de workspace mostra as abas daquele workspace, enquanto as abas de outros workspaces continuam rodando em segundo plano.
- **Citação para o chat**: um botão na barra de ferramentas insere a saída visível (ou seleção) do terminal ativo como um bloco de código no painel de chat que estiver em foco no momento, para o usuário mostrar um erro ao agente sem precisar redigitá-lo.

O painel é um elemento customizado de posição fixa ancorado na parte inferior da janela (o `q-drawer` do Quasar só suporta `left`/`right`, não `bottom`), alternado por um botão flutuante e redimensionável por uma alça de arraste.

## Fluxo de Dados

```
┌──────────────────────────────┐
│  Shell do usuário ($SHELL,    │
│  login) - PTY real            │
│  (portable-pty)               │
└────────────┬─────────────────┘
             │ leitura de bytes brutos (thread dedicada)
┌────────────▼─────────────────┐
│  Backend Rust (Tauri)         │
│  TerminalManager (terminal.rs)│
│  spawn_terminal(key,cwd,...)  │
│  write_terminal(key,data)     │
│  resize_terminal(key,cols,..) │
│  kill_terminal(key)           │
│  list_terminals()             │
│  eventos: terminal:data       │
│           terminal:exit       │
└────────────┬─────────────────┘
             │ IPC Tauri invoke/listen
┌────────────▼─────────────────┐
│  Frontend (Pinia stores)      │
│  terminal-runtime-store.js    │
│    openKeys, focusedKeyByPath │
│  terminal-session-store.js    │
│    instância xterm.js Terminal│
└────────────┬─────────────────┘
             │ bindings reativos
┌────────────▼─────────────────┐
│  TerminalPanel.vue            │
│  TerminalTabStrip.vue         │
│  TerminalHost.vue (xterm.js)  │
└───────────────────────────────┘
```

## Backend Rust

### `terminal.rs` — `TerminalManager`

Um shell com PTY por aba aberta, rastreado em `Arc<Mutex<HashMap<String, TerminalHandle>>>` indexado por um uuid gerado pelo frontend — o mesmo formato de mapa-por-chave que o `ZeroBridge` (veja [zero-bridge](./zero-bridge.md)) usa para processos `zero acp`, mas para shells reais. Diferente do `ZeroBridge`, não há reconexão sob demanda: fechar uma aba mata seu shell definitivamente.

Construído sobre o crate [`portable-pty`](https://crates.io/crates/portable-pty) (o mesmo em que o próprio wezterm é baseado) em vez de uma alternativa exclusiva para Unix, para que o app não fique travado sem um backend de PTY para Windows (ConPTY) mais adiante, mesmo hoje distribuindo só para Linux.

**Detalhes de implementação importantes:**

- **Resolução do shell padrão**: `CommandBuilder::new_default_prog()` resolve `$SHELL` (com fallback para a base de dados passwd e depois `/bin/sh`) e o inicia como **shell de login** (argv0 prefixado com `-`), então arquivos `.bashrc`/`.zshrc`/profile são carregados — é isso que deixa o `PATH`/aliases/ambiente de dev normal do usuário disponível dentro do painel. Todo o ambiente do processo pai é herdado automaticamente; `TERM=xterm-256color` é definido explicitamente (apps GUI normalmente são iniciados sem nenhum `TERM`).
- **Streaming de bytes brutos, não baseado em linhas**: as APIs `Read`/`Write`/`Child` do `portable-pty` são bloqueantes/síncronas (sem equivalente tokio), então cada terminal ganha uma thread dedicada (`std::thread::spawn`, não `tokio::spawn`) fazendo leituras `read()` brutas em um buffer de 8 KB — diferente do leitor de stdout do `ZeroBridge`, que assume linhas JSON-RPC em UTF-8. Um pequeno buffer de bytes restantes (`drain_utf8`, ≤3 bytes) é carregado entre leituras para que um caractere UTF-8 multi-byte dividido entre duas leituras não seja corrompido em caracteres de substituição.
- **Escritas/redimensionamentos/kills** passam por `tokio::task::spawn_blocking`, já que chamam a mesma API síncrona de PTY a partir de um comando Tauri assíncrono.
- **Limpeza ao sair**: quando o loop de leitura vê EOF (shell saiu, ou foi morto), ele coleta o processo filho (`child.wait()`), remove o terminal do mapa e emite `terminal:exit` — este é o **único** lugar onde entradas são removidas, seja porque o shell saiu sozinho ou foi morto via `kill_terminal`/`kill_all`. `TerminalManager::kill_all()` está conectado ao handler `RunEvent::Exit` do app (junto com `ZeroBridge::kill_all()`), garantindo que nenhum shell órfão sobre quando o app fecha.

**Estruturas de dados:**

```rust
pub struct TerminalSpawnInfo {
    pub key: String,
    pub pid: Option<u32>,
    pub shell: String,   // nome base do $SHELL, para o rótulo da aba
}

pub struct LiveTerminalInfo {
    pub key: String,
    pub cwd: String,
    pub live: bool,
}
```

### Commands em `lib.rs`

| Command           | Descrição                                                                                                      |
| ----------------- | -------------------------------------------------------------------------------------------------------------- |
| `spawn_terminal`  | Abre um PTY, inicia o shell padrão em `cwd` com o `cols`/`rows` informados. Retorna `TerminalSpawnInfo`.       |
| `write_terminal`  | Escreve entrada bruta (teclas, texto colado) no stdin do shell.                                                |
| `resize_terminal` | Chama o `resize()` do PTY (ioctl) para que o shell/apps dentro dele reajustem (`$COLUMNS`, interfaces curses). |
| `kill_terminal`   | Envia o sinal de kill; não bloqueia até o processo realmente sair.                                             |
| `list_terminals`  | Retorna `Vec<LiveTerminalInfo>` para reconciliação de estado do frontend.                                      |

### Eventos

| Evento          | Payload             | Descrição                                 |
| --------------- | ------------------- | ----------------------------------------- |
| `terminal:data` | `{ key, data }`     | Um trecho de saída do PTY (UTF-8 válido). |
| `terminal:exit` | `{ key, exitCode }` | O processo do shell saiu.                 |

## Frontend

### `terminal-runtime-store.js`

Espelha o formato do `session-runtime-store.js`, com escopo por workspace, mas mais simples: sem limite de painéis, sem modo "fechar mas manter rodando".

| Estado             | Tipo     | Descrição                                                                                            |
| ------------------ | -------- | ---------------------------------------------------------------------------------------------------- |
| `openKeys`         | `Array`  | Lista plana de abas de terminal abertas, em **todos** os workspaces.                                 |
| `focusedKeyByPath` | `Object` | Aba em foco por caminho de workspace.                                                                |
| `keyMeta`          | `Object` | Metadados por chave: `{ cwd, title, shell, pid, status }` (`status`: `spawning`/`running`/`exited`). |
| `panelOpen`        | `bool`   | Se o painel inferior está expandido.                                                                 |
| `panelHeightPx`    | `number` | Altura do painel em px, persistida no `localStorage`.                                                |

| Getter                | Descrição                                                              |
| --------------------- | ---------------------------------------------------------------------- |
| `visibleKeys(path)`   | Abas pertencentes a um workspace específico (alimenta a tira de abas). |
| `focusedKeyFor(path)` | A aba em foco para um workspace específico.                            |

| Ação                  | Descrição                                                                                                                                                           |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `openTab(key, path)`  | Adiciona uma aba, foca nela, abre o painel.                                                                                                                         |
| `focusTab(key, path)` | Foca uma aba já aberta.                                                                                                                                             |
| `closeTab(key)`       | Mata o shell (`kill()` do `terminal-session-store`), depois remove a aba. Sempre mata — não existe modo "fechar mas manter rodando", diferente dos painéis de chat. |

### `terminal-session-store.js`

Store dinâmica por aba (`useTerminalSessionStore(key)`, uma instância Pinia por aba aberta, mesma fábrica do `zero-session-store.js`), dona das instâncias vivas `Terminal`/`FitAddon` do `xterm.js` (criadas pelo `TerminalHost.vue`, que precisa do `$q` para temas) e do ciclo de vida do PTY no backend.

| Ação                     | Descrição                                                                                                                                                                                                                                                                                                               |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `attach(term, fitAddon)` | Guarda as instâncias do xterm.js (via `markRaw()` — elas rodam seu próprio loop de renderização e não devem virar proxy reativo do Vue).                                                                                                                                                                                |
| `spawn(cwd)`             | Chama `spawn_terminal`, conecta os listeners `onTerminalData`/`onTerminalExit` filtrados pela chave desta aba, conecta `term.onData()` a `write_terminal`.                                                                                                                                                              |
| `resize(cols, rows)`     | Chama `resize_terminal`.                                                                                                                                                                                                                                                                                                |
| `kill()`                 | Desconecta listeners, chama `kill_terminal` (best-effort — já ter saído é aceitável), descarta a instância do xterm.js.                                                                                                                                                                                                 |
| `extractCiteText()`      | Retorna a seleção ativa (`term.getSelection()`) ou, se não houver, o viewport atualmente visível (`term.buffer.active`, linha a linha) como texto puro — usado pela ação "citar para o chat". Deliberadamente texto puro, não preservando ANSI (`@xterm/addon-serialize` foi considerado e descartado por esse motivo). |

## Componentes de UI (`src/components/terminal/`)

### `TerminalPanel.vue`

O painel inferior. Não é um `q-drawer` (o Quasar só suporta `left`/`right`) — um elemento `position: fixed` customizado, seguindo o mesmo idioma do botão-toggle flutuante do `McpDrawer.vue`, rotacionado para a borda inferior.

- **Largura acompanha os drawers laterais dinamicamente**: em vez de re-derivar a largura atual da sidebar esquerda e do `McpDrawer` a partir do estado local de cada um, o painel lê de volta o `padding-left`/`padding-right` real que o Quasar já aplica ao `.q-page-container` (via `ResizeObserver`, que dispara mesmo em mudanças só de padding) — o mesmo padding que já mantém o `SessionTileGrid` corretamente limitado entre os dois drawers. É isso que mantém o painel de terminal encaixado entre os drawers (tanto no modo empurrar quanto no modo overlay/mobile), em vez de ocupar a largura cheia da viewport e ficar visualmente cortado/coberto por eles.
- **Altura redimensionável** por uma alça de arraste na borda superior do painel, persistida no `localStorage`.
- **Toda aba aberta em todos os workspaces permanece montada** (`v-show`, nunca `v-if`), então trocar de aba ou de workspace nunca destrói uma instância viva do `xterm.js` — só a visibilidade do DOM do `TerminalHost.vue` alterna. Apenas a aba realmente em foco do workspace ativo é mostrada por vez.
- Contém o `TerminalTabStrip.vue`, um botão "citar para o chat" na barra de ferramentas, e os hosts de terminal.

### `TerminalTabStrip.vue`

Tira de chips fecháveis feita à mão (seguindo o padrão `.mcp-file-chip` do `McpDrawer.vue` em vez do `q-tabs` do Quasar, que é feito para abas de navegação, não para a semântica de abas fecháveis de navegador). Mostra um indicador de status (spawning/running/exited), o título da aba, um botão de fechar, e um "+" no final para abrir uma nova aba no workspace ativo.

### `TerminalHost.vue`

Cria o `Terminal` + `FitAddon` do `xterm.js` na primeira montagem, entrega para a `terminal-session-store` da aba via `attach()`, depois chama `spawn(cwd)`. Um `ResizeObserver` no próprio elemento chama `fitAddon.fit()` e (com debounce) `resize_terminal` a cada mudança de tamanho.

### Citação ("citar para o chat")

Um único botão na barra de ferramentas do `TerminalPanel.vue` (que opera sobre a aba de terminal em foco, não um botão por aba):

1. Resolve a aba de terminal em foco (`terminalRuntime.focusedKeyFor(activePath)`) e o painel de **chat** em foco (`sessionRuntime.focusedKeyFor(activePath)` — a mesma resolução que o `McpDrawer.vue` já usa para sua lista de arquivos editados).
2. Lê a saída visível do terminal via `extractCiteText()`.
3. Anexa como um bloco de código (` ``` `) ao `draftText` do painel de chat.

Isso exigiu dois pequenos pré-requisitos:

- **`draftText` movido de um `ref` local no `ChatView.vue` para o `zero-session-store.js`** — o texto do campo de composição agora vive na store Pinia por sessão em vez de estado local do componente, então outro componente (o painel de terminal) consegue inserir texto no painel que estiver em foco sem precisar acessar internamente o `ChatView.vue`. É limpo automaticamente ao fechar o painel (`store.$reset()`, já chamado por `closePanel`/`stopAndDispose`).
- **Rastreio de foco corrigido no `SessionTileGrid.vue`**: o `focusedKeyByPath` do `session-runtime-store.js` só era atualizado por `openPanel` (o painel aberto mais recentemente), nunca por o usuário clicar de fato em um painel já aberto — o `ChatView.vue` já emitia `@focus-input` no evento de foco do seu textarea, mas nada escutava esse evento. O `SessionTileGrid.vue` agora conecta esse emit (e um `@mousedown.capture` no painel inteiro, não só no textarea) a `sessionRuntime.focusPanel(key, activePath)`, para que "o painel em foco" reflita onde o usuário está realmente trabalhando.

## Notas de Comportamento

- **Efêmero**: terminais não são persistidos entre reinicializações do app. Fechar uma aba mata seu shell; fechar o app mata todos os terminais vivos (`kill_all` no `RunEvent::Exit`).
- **Sem limite por workspace**: diferente dos painéis de chat (`MAX_OPEN_PANELS = 4`), não há limite de quantas abas de terminal um workspace pode ter abertas.
- **`gridHeight` do `SessionTileGrid.vue`** subtrai `terminalRuntime.panelHeightPx` quando o painel está aberto, para que a grade de chat não renderize por baixo do painel de terminal.

## Novas Dependências

- **Cargo**: `portable-pty = "0.9"`
- **npm**: `@xterm/xterm`, `@xterm/addon-fit`

## Referências

- [zero-bridge: Conexão com o zero CLI](./zero-bridge.md) — o padrão de processo-por-chave que o `TerminalManager` desta feature espelha.
- [Sistema de Sessões](./session-system.md) — o modelo de painel/foco por workspace que o escopo de abas desta feature espelha.
- [Painel MCP](./mcp-panel.md) — o idioma de botão-toggle/drawer flutuante em que a UI deste painel é baseada.
