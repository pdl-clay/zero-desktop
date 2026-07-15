# 004 — Sessões de Chat Paralelas (Tiling)

## Status

Aceito. Baseado em [ADR 003 — Migrar para ACP](./003-migrate-to-acp.md).

## Contexto

O ADR 003 estabeleceu um processo `zero acp` por sessão, mas a GUI ainda
mantinha **uma única sessão viva por vez**: `ZeroBridge` guardava um slot
`Option<AcpSession>`, e tanto o `start()` no Rust quanto o frontend matavam
incondicionalmente a sessão atual antes de iniciar uma nova. Trocar de workspace
ou sessão significava perder qualquer trabalho em andamento.

O objetivo desta mudança é paralelismo real: múltiplas sessões processando
simultaneamente, visíveis lado a lado em painéis redimensionáveis, onde abrir
uma nova sessão nunca mata uma existente.

## Decisões (confirmadas com o usuário)

1. **Paralelismo dentro do mesmo workspace** é permitido. O usuário aceita o
   risco de edições concorrentes nos mesmos arquivos sem bloqueio — apenas um
   aviso não-bloqueante.
2. **Visualização em tiling**: cada sessão aberta ganha seu próprio painel
   redimensionável.
3. **Fechar painel NÃO para a sessão**: apenas uma ação explícita "Parar" mata o
   processo. Reabrir reconecta ao mesmo processo/estado.
4. **Indicadores**: badges/spinners na lista de sessões e avatares de workspace
   mostram quem está processando em segundo plano, com um estado distinto
   "precisa de atenção" para permissões pendentes.
5. **Limite = 4, unificado**: máximo de 4 processos vivos E 4 painéis abertos.
6. **Troca de modelo afeta apenas a sessão em foco**: demais sessões continuam
   com o modelo anterior.
7. **Painéis responsivos**: cada painel adapta seu conteúdo à largura
   disponível conforme mais painéis abrem (1 → 2 → 3 → 4).

## Arquitetura

### Rust: mapa de sessões por chave

`ZeroBridge.sessions` mudou de `Option<AcpSession>` para
`HashMap<String, AcpSession>`, indexado por uma **chave de roteamento do
frontend** (UUID para novas sessões, `session_id` para retomadas). Todos os
comandos aceitam este parâmetro `key: String`.

Todo evento emitido carrega `sessionKey` no payload, para os listeners do
frontend filtrarem pela sua própria chave.

`start()` retorna `StartedSession { key, session_id, reattached }` — corrigindo
também um bug pré-existente onde o `currentSessionId` do frontend nunca era
atualizado com o id real atribuído pela CLI.

O limite é enforceado com checagem dupla (antes e depois do handshake ACP) sobre
`live_count`, retornando erro com prefixo `SESSION_CAP_REACHED` ao exceder.

### Frontend: divisão de stores

A store monolítica `zero-store.js` foi dividida em:

- **`zero-store.js`** (global): `zeroPath`, `availableModels`, `activeModel`,
  `mcpBackends`, `mcpTools`, `permissionMode` — apenas estado app-wide.
- **`zero-session-store.js`** (fábrica): `useZeroSessionStore(key)` cria uma
  store Pinia por sessão com `messages`, `currentResponse`, `runInProgress`,
  etc. Todos os listeners filtram por `sessionKey`.
- **`session-runtime-store.js`** (orquestrador): `openKeys`, `focusedKey`,
  `keyMeta`. Fornece `openOrFocusSession(key, cwd, sessionId)`.
- **`workspaces-store.js`**: ganhou `sessionsByPath` para listar sessões por
  workspace.

### Frontend: UI de tiling

`SessionTileGrid.vue` substitui o `<ChatView>` único em `MainLayout.vue`.
Renderiza 1 (tela cheia), 2 (divisão horizontal), 3 (aninhado), ou 4 (grade 2×2)
painéis usando `QSplitter` do Quasar. Cada painel tem um `SessionPaneHeader.vue`
com botões distintos "Fechar" (esconde) e "Parar" (mata processo).

`ChatView.vue` agora aceita uma prop `sessionKey`, cria sua própria instância de
session store, e a `provide` para componentes filhos via `inject("zeroStore")`.

### Frontend: responsividade

Cada `ChatView` tem um `ResizeObserver` que rastreia a largura real do painel
(não a janela). Abaixo de 500px, o painel recebe a classe `pane--narrow` que
esconde o PlanPanel, colapsa botões do ChatInput e reduz padding.

### Limpeza

`.kill_on_drop(true)` no `Command` do `zero acp` mais um handler `RunEvent::Exit`
chamando `ZeroBridge::kill_all()` garantem que nenhum processo órfão permaneça
ao fechar o app. Guards de `std::sync::Mutex` por arquivo em
`session-titles.json` e `session-models.json` previnem corridas de
leitura-modificação-escrita concorrentes.

## Consequências

- `MainLayout.vue` não mata mais sessões ao trocar de workspace — trocar é
  navegação pura (carrega a lista de sessões daquele workspace).
- `McpDrawer.vue` lê `editedFiles` da session store focada (não da global).
- `ChatInput.vue` chama `sessionStore.switchModel()` (restart por sessão).
- A lista de sessões mostra badges ao vivo via `sessionRuntime.keyMeta`.
- Comando `list_live_sessions` adicionado para reconciliação de estado.
