# Modo Plano

O Modo Plano é o equivalente nativo do zero-desktop ao Plan Mode do Claude
Code: o agente fica restrito à exploração somente-leitura e, em vez de fazer
alterações, redige um plano de implementação e para para o usuário revisar em
um diálogo com ações de **Aprovar** / **Pedir melhorias**. Aprovar deixa o
agente seguir para implementar o plano (automaticamente ou com confirmação por
edição); pedir melhorias mantém o agente em modo leitura enquanto ele revisa o
plano com base no feedback.

## Visão Geral

O motor do zero (`zero acp`) já implementa o mecanismo subjacente nativamente
como o modo de permissão ACP `"spec-draft"` (anunciado aos clientes como
`"Plan"`). Enquanto uma sessão está nesse modo:

- Só ferramentas de leitura, `ask_user`, e uma tool especial `submit_spec` são
  anunciadas ao modelo — reforçado inteiramente **no lado do servidor** pelo
  motor. O zero-desktop não implementa nenhum bloqueio de ferramentas no
  cliente.
- Quando o modelo termina de explorar, ele chama `submit_spec(title, plan)`,
  que grava um markdown em `<cwd>/.zero/specs/<data>-<slug>.md` e encerra o
  turno.
- O motor emite uma notificação ACP `session/update` com
  `sessionUpdate: "_zero/spec_review_required"` (uma extensão vendor do ZERO)
  carregando o id do spec, título e caminho do arquivo.

Diferente do `zero exec --use-spec` (CLI) ou do TUI, o caminho ACP não
bifurca uma sessão separada de "spec-impl" na aprovação — `zero spec
approve`/`reject` são comandos exclusivos de CLI que não funcionam contra
sessões criadas via ACP (os metadados de sessão que eles exigem nunca são
gravados sobre ACP). Aprovação e rejeição são, portanto, implementadas como
uma **continuação da mesma sessão**: aprovar troca o modo ACP de volta para
`"auto"` ou `"ask"` e envia um prompt de continuação normal instruindo o
agente a implementar o spec já submetido; pedir melhorias simplesmente envia
o feedback do usuário como o próximo prompt enquanto o modo permanece
`"spec-draft"`. Isso também é mais fiel ao próprio Plan Mode do Claude Code,
que nunca bifurca a conversa.

## Fluxo de Dados

```
┌───────────────────────────────┐
│ Usuário escolhe "Plano" no       │
│ dropdown de modo de execução    │
│ ChatInput.vue → setMode()       │
└───────────────┬─────────────────┘
                │ session/set_mode "spec-draft"
┌───────────────▼─────────────────┐
│ zero acp (motor Go)              │
│ só ferramentas de leitura +      │
│ submit_spec são anunciadas       │
└───────────────┬─────────────────┘
                │ submit_spec(title, plan)
                │ → grava .zero/specs/<id>.md
                │ → session/update
                │   "_zero/spec_review_required"
┌───────────────▼─────────────────┐
│ bridge.rs                        │
│ translate_session_update         │
│  → zero:event "spec_review_required" │
│ spawn_stdout_reader              │
│  → persiste PendingSpec em       │
│    session-plan-state.json       │
└───────────────┬─────────────────┘
                │ listener frontend
┌───────────────▼─────────────────┐
│ zero-session-store.js            │
│ _loadPlanReview(event)           │
│  → readSpecFile() → pendingPlanReview │
└───────────────┬─────────────────┘
                │ binding reativo
┌───────────────▼─────────────────┐
│ PlanReviewDialog.vue             │
│ plano em markdown + Aprovar /    │
│ Pedir melhorias                  │
└─────────────────────────────────┘
```

## Backend Rust (`src-tauri/src/bridge.rs`, `lib.rs`)

### Estado de plano persistido por sessão

Todo processo `zero acp` novo — respawn no meio da execução ou reinício
completo do app — registra sua sessão ACP sempre em `PermissionModeAuto` no
lado Go (`registerSession` em `internal/acp/agent.go` do `my-zero`); o motor
não guarda memória de uma sessão ter estado em `spec-draft`. O zero-desktop
mantém, portanto, seu próprio registro, seguindo o mesmo padrão que
`session-models.json` já usa para a escolha de modelo por sessão:

`~/.local/share/zero-desktop/session-plan-state.json` — um
`HashMap<session_id, SessionPlanState>`:

```rust
struct SessionPlanState {
    mode: String, // "auto" | "ask" | "spec-draft" - ausente = "auto"
    pending_spec: Option<PendingSpec>, // spec_id, title, file_path, relative_path
}
```

- `spawn_and_handshake` reaplica `spec-draft` via `session/set_mode` logo
  após cada handshake se o modo persistido exigir — o mesmo padrão de
  reaplicação pós-respawn que o mapa de modelos já usa. `"auto"`/`"ask"` não
  precisam de reaplicação (`"auto"` já é o default do motor).
- `spawn_stdout_reader` persiste um `PendingSpec` no momento em que
  `_zero/spec_review_required` chega, resolvendo o `session_id` estável a
  partir do mapa compartilhado `sessions` (o loop do reader só tem o
  `session_key` do painel em escopo). Não é gravado no log de histórico do
  chat — o registro durável é o arquivo `.md` mais esta entrada JSON; o
  `tool_call`/`tool_call_update` do próprio `submit_spec` (sempre emitido à
  parte pelo motor) já deixa rastro normal na transcrição.
- `delete_session` remove a entrada de plan state da sessão junto com seus
  registros de título e modelo.

### Comandos Tauri

| Comando                                         | Finalidade                                                                                |
| ----------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `switch_zero_mode(key, mode)`                   | Push ao vivo via `session/set_mode` (sessão precisa estar conectada) + persiste em disco. |
| `set_zero_session_mode_by_id(session_id, mode)` | Persiste só em disco, para um painel que ainda não (re)conectou.                          |
| `get_zero_session_plan_state(session_id)`       | Leitura pura em disco — modo + plano pendente, sem exigir sessão viva.                    |
| `clear_zero_pending_spec(session_id)`           | Limpa o spec pendente persistido após aprovar/pedir melhorias.                            |
| `read_spec_file(path)`                          | Lê o conteúdo de um arquivo markdown de spec para o diálogo de revisão.                   |

## Frontend (`src/stores/zero-session-store.js`)

| Estado              | Tipo             | Descrição                                                                            |
| ------------------- | ---------------- | ------------------------------------------------------------------------------------ |
| `sessionMode`       | `string`         | O modo de permissão ACP ao vivo desta sessão: `"auto"` \| `"ask"` \| `"spec-draft"`. |
| `pendingPlanReview` | `Object \| null` | `{ specId, title, filePath, relativePath, content }` aguardando decisão, ou `null`.  |
| `_sessionModeDirty` | `bool`           | Interno: uma troca de modo feita antes deste painel ter um `sessionId`.              |

Ações principais:

- `setMode(mode)` — empurra ao vivo se conectado, persiste por id se existe
  `sessionId` mas não está conectado, ou marca a escolha como "dirty" para
  `_syncPlanStateFromDisk` aplicar quando `startSession` conectar. Move os
  três modos (`"auto"` / `"ask"` / `"spec-draft"`), não só o Modo Plano.
- `_syncPlanStateFromDisk()` — restaura `sessionMode`/`pendingPlanReview` a
  partir do estado persistido no backend (ou aplica uma troca de modo dirty).
  Chamada tanto de `openSession` (navegando histórico, sem exigir conexão
  viva — cobre recuperação de sessão) quanto do caminho de sucesso de
  `startSession` (cobre reconectar após reiniciar o app inteiro).
- `_loadPlanReview(event)` — busca o markdown do spec quando
  `spec_review_required` chega ao vivo durante a sessão atual do app.
- `approvePlanReview(mode, comment)` — fecha a revisão, troca o modo para
  `"auto"` ou `"ask"`, envia a instrução de implementação como mensagem de
  continuação normal.
- `requestPlanChanges(feedback)` — fecha a revisão (modo continua
  `"spec-draft"`), envia o feedback como próxima mensagem.

`sessionMode` é resetado para `"auto"` e `pendingPlanReview` para `null` numa
sessão nova, no reset pré-conexão de `startSession`/`openSession`, e em
`removeSession` (sessão realmente excluída). `handleProcessExited` (crash do
processo vivo, não troca de sessão) limpa só `pendingPlanReview` — o modo é
deixado intocado, já que o bridge Rust já reaplica `spec-draft` no próximo
respawn, e resetá-lo localmente mostraria "auto" no dropdown enquanto o motor
continua restrito a leitura por baixo.

## Componentes de UI

- **`ChatInput.vue`** — o Modo Plano é uma das três opções de um único
  dropdown de modo de execução (auto / perguntar / plano), substituindo o que
  antes era um toggle de dois estados (auto/perguntar) separado. As três
  opções chamam a mesma ação `sessionStore.setMode(mode)` e são movidas pelo
  `session/set_mode` real do ACP — não sobra nenhuma aproximação client-side;
  o antigo atalho global `auto_allow` (persistido em `localStorage`, que
  clicava "permitir" automaticamente em todo pedido de permissão) foi
  removido em favor disso. Ver [Componentes da Interface de Chat](./chat-interface.md)
  para o lugar do dropdown entre os outros controles da barra de input.
- **`PlanReviewDialog.vue`** — o primeiro `q-dialog` do app (persistente, sem
  fechar por Esc/clique fora). Renderiza o spec através do utilitário já
  existente `renderMarkdown()` (`src/utils/markdown.js`) e oferece três
  ações:
  - **Aprovar e implementar automaticamente** → `approvePlanReview("auto")`
  - **Aprovar e revisar cada edição** → `approvePlanReview("ask")`
  - **Pedir melhorias** → abre um textarea de feedback inline, depois
    `requestPlanChanges(feedback)`

  Isso espelha a própria distinção do Claude Code entre aceitar edições
  automaticamente e revisá-las uma a uma ao sair do plan mode — ambas mapeiam
  diretamente para os modos ACP `"auto"`/`"ask"` que o motor já suporta.

- **`ChatView.vue`** — monta `<PlanReviewDialog />` e adiciona
  `!store.pendingPlanReview` ao gate `canSend`, bloqueando o composer
  principal enquanto uma revisão está pendente (o feedback é digitado dentro
  do próprio diálogo).

## Persistência e Recuperação de Sessão

Tanto o modo de execução da sessão quanto um plano pendente de revisão sobrevivem a:

- **Crash/respawn do processo `zero acp`** — o bridge Rust reaplica
  `spec-draft` automaticamente antes do próximo turno.
- **Fechar e reabrir o app inteiro** — `session-plan-state.json` é relido na
  próxima vez que a sessão conecta ou é aberta pelo histórico.
- **Reabrir uma sessão do histórico sem reconectar ainda** — `openSession`
  restaura o modo selecionado e, se o arquivo do spec ainda for legível, o
  diálogo de revisão pendente, antes mesmo de existir um processo vivo.

Se o arquivo `.md` de um spec pendente tiver sido apagado do disco nesse
meio-tempo, o frontend se autocorrige limpando o registro órfão persistido em
vez de exibir um diálogo de revisão quebrado.

## Referências

- [Sistema de Plano](./plan-system.md) — a checklist inline de todo, sempre
  ativa e sem relação com isto (tool `update_plan` / atualizações ACP `plan`)
  — não confundir com o Modo Plano.
- [Modo Advisor](./advisor-mode.md) — o precedente mais próximo de uma
  feature togglável por sessão, com seus próprios comandos Tauri e estado de
  store.
- [zero-bridge](./zero-bridge.md)
- [Troca de Modelo](./model-switching.md) — o padrão de reaplicação pós-respawn
  via `session-models.json` que a persistência desta feature segue.
