# Sistema de Plano

Este documento descreve como o zero-desktop renderiza o plano de tarefas do agente — a checklist estruturada que o agente emite via chamadas `update_plan` e eventos ACP `plan_update`.

## Visão Geral

O agente do zero pode manter um plano em memória dos passos que pretende executar. Ele comunica esse plano através de:

- **Chamadas `update_plan`**: O agente chama uma ferramenta chamada `update_plan` com um array `args.plan` de entradas. Este é o mecanismo primário — o plano substitui o anterior por completo a cada chamada.
- **Atualizações `plan` da sessão**: Notificações ACP `session/update` com `sessionUpdate: "plan"` carregam um array `entries` diretamente. São traduzidas para eventos `plan_update` pelo bridge.

O frontend renderiza o plano **inline na barra de input do chat** (fixado acima do textarea) para que o usuário veja no que o agente está trabalhando sem rolar o histórico. O plano se auto-oculta quando todos os itens estão concluídos.

## Fluxo de Dados

```
┌─────────────────────────────────┐
│  ACP session/update              │
│  sessionUpdate: "plan"           │
│  entries: [{content, status,     │
│             priority}]           │
└───────────────┬─────────────────┘
                │ bridge.rs translate_session_update
┌───────────────▼─────────────────┐
│  zero:event                      │
│  type: "plan_update"             │
│  entries: [...]                  │
└───────────────┬─────────────────┘
                │ listener frontend
┌───────────────▼─────────────────┐
│  zero-store.js                   │
│  currentPlan = event.entries     │
│  getter activePlan               │
│    → null se todos concluídos    │
└───────────────┬─────────────────┘
                │ binding reativo
┌───────────────▼─────────────────┐
│  ChatInput.vue                   │
│  Checklist do plano (inline)     │
│  ou PlanPanel.vue (independente) │
└─────────────────────────────────┘
```

Além disso, chamadas `update_plan` (que carregam `args.plan`) são interceptadas em `addToolCall` e nunca renderizadas como cards de ferramenta — o plano é rastreado via `currentPlan`:

```
tool_call (name: "update_plan")
  → addToolCall()
    → se name === "update_plan":
        currentPlan = args.plan
        return (nenhum card renderizado)
```

## Backend Rust

### Tradução de eventos ACP (`bridge.rs`)

A função `translate_session_update` trata `sessionUpdate: "plan"`:

```rust
"plan" => {
    let entries = update.get("entries").cloned().unwrap_or(serde_json::json!([]));
    Some(OutputEvent::new("plan_update", serde_json::json!({ "entries": entries })))
}
```

Cada entrada no array `entries` tem:

```json
{
  "content": "Corrigir o bug de login",
  "status": "in_progress",
  "priority": 0
}
```

**Valores de status** (como emitidos pelo agente do zero):

- `"pending"` — ainda não iniciado
- `"in_progress"` — trabalhando atualmente
- `"completed"` — concluído
- `"failed"` — não foi possível completar

O bridge não interpreta nem modifica as entradas do plano; apenas as repassa. O agente do zero emite atualizações `plan` que **substituem** o plano inteiro a cada vez (não patches incrementais), então o `currentPlan` do frontend é sempre um snapshot completo.

## Frontend

### Estado do Plano

`currentPlan` vive na store **por sessão** `zero-session-store.js` (a store
factory `useZeroSessionStore(key)`), não na `zero-store.js` global — cada
painel aberto acompanha o plano do seu próprio agente de forma independente.

| Estado        | Tipo    | Descrição                                                      |
| ------------- | ------- | -------------------------------------------------------------- |
| `currentPlan` | `Array` | Entradas atuais do plano do agente (substituído por completo). |

### Getter `activePlan`

```js
activePlan(state) {
  if (!state.currentPlan || state.currentPlan.length === 0) return null;
  const allDone = state.currentPlan.every((item) => item.status === "completed");
  return allDone ? null : state.currentPlan;
}
```

Retorna `null` quando vazio ou todos concluídos — a UI se auto-oculta.

### Tratamento de eventos (`handleZeroEvent`)

```js
case "plan_update":
  if (Array.isArray(event.entries)) {
    this.currentPlan = event.entries;
  }
  break;
```

### Interceptação de chamada de ferramenta (`addToolCall`)

```js
if (event.name === "update_plan") {
  if (Array.isArray(event.args?.plan)) {
    this.currentPlan = event.args.plan;
  }
  return; // nenhum card renderizado
}
```

### Reset do plano

`currentPlan` é limpo para `[]` quando:

- Uma nova sessão inicia (`startSession`).
- Uma sessão é aberta do histórico (`openSession`).
- O processo termina (`handleProcessExited`).

### `src/utils/plan.js` — Ícones e cores

```js
export function planIcon(status) {
  // pending → "radio_button_unchecked"
  // in_progress → "autorenew"
  // completed → "check_circle"
  // failed → "cancel"
}

export function planColor(status) {
  // pending → "grey-6"
  // in_progress → "info"
  // completed → "positive"
  // failed → "negative"
}
```

## Componentes UI

### Inline no `ChatInput.vue`

A checklist do plano é renderizada diretamente acima do textarea quando `activePlan` é não-nulo:

```
┌─────────────────────────────────┐
│  ◉ Analisar o código            │  ← in_progress (azul,
│  ○ Escrever testes              │     ícone girando)
│  ○ Atualizar documentação       │  ← pending (cinza,
│  ○ Verificar build              │     círculo vazio)
│ ─────────────────────────────── │
│  [textarea]                  ↑  │
└─────────────────────────────────┘
```

**Comportamento:**

- Cada item mostra ícone de status (de `planIcon`) com a cor correspondente (de `planColor`).
- Itens concluídos ganham texto tachado.
- Itens `in_progress` ganham animação giratória.
- A checklist é compacta — projetada para não tomar espaço vertical significativo.
- Quando todos concluem, `activePlan` retorna `null` e a checklist se auto-oculta.

### `PlanPanel.vue` independente

Renderização alternativa como painel dedicado, usado na coluna direita do `ChatView.vue` em telas mais largas (≥1024px). Mostra a mesma checklist em um painel de `260px` com borda. Oculto em telas estreitas.

## Reprodução de histórico

Ao reproduzir uma sessão do histórico, chamadas `update_plan` no stream de eventos atualizam `currentPlan` pelo mesmo caminho `addToolCall` — mas como `currentPlan` só importa para a UI ao vivo, e `buildMessagesFromHistory` reconstrói mensagens do zero, o estado do plano durante a reprodução é transitório. O painel de plano aparece em sessões reproduzidas se um plano estava ativo ao final.

## Referências

- [Interface de Chat](./chat-interface.md)
- [zero-bridge: Conexão com o zero CLI](./zero-bridge.md)
- [Utilitário Edit Tools](../../src/utils/edit-tools.js)
