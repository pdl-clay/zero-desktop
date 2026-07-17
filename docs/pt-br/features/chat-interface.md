# chat-interface: Componentes da Interface de Chat

Este documento descreve a arquitetura de componentes da interface de chat e o sistema de renderização de mensagens multi-tipo.

## Visão Geral

A UI de chat renderiza uma lista heterogênea de mensagens tipadas. Cada mensagem possui um campo `type` que determina qual componente Vue a renderiza. Isso substitui o modelo antigo de `{ role, content }` onde todos os eventos não-texto eram despejados como strings JSON cruas.

## Modelo de mensagens

Todas as mensagens compartilham campos comuns e adicionam campos específicos ao tipo:

```js
{
  id: string,          // identificador único
  type: 'text' | 'thinking' | 'tool_call' | 'permission_request' | 'permission_decision' | 'error',
  timestamp: number,
  // campos específicos do tipo abaixo
}
```

### Mensagens `text`

```js
{
  type: 'text',
  role: 'user' | 'assistant' | 'system',
  content: string,
  file?: { mimeType: string, data: string, name: string }  // apenas mensagens de usuário
}
```

Renderizadas por `TextMessage.vue` usando `<q-chat-message>` do Quasar com cores baseadas no papel. Mensagens de usuário com arquivo anexado mostram a prévia do arquivo (miniatura de imagem ou chip de arquivo) acima do texto.

### Mensagens `thinking`

```js
{ type: 'thinking', content: string }
```

Renderizadas por `ThinkingBlock.vue` em dois modos:

- **Streaming** (`streaming=true`): Barra fina com tom âmbar, spinner e rótulo "Pensando...". Não expansível — o conteúdo ainda está chegando. Aparece no fim da lista de mensagens junto com a bolha de texto em streaming.
- **Finalizado** (`streaming=false`): Um `q-expansion-item` colapsável com ícone de check e rótulo "Pensamento". Clique para revelar o texto completo do raciocínio em itálico.

### Mensagens `tool_call`

```js
{
  type: 'tool_call',
  toolName: string,
  toolUseId: string,
  input: object,
  status: 'running' | 'completed' | 'error',
  result: string | null,
}
```

Renderizadas por `ToolCallMessage.vue` como um card com estados:

- **running**: spinner + ícone da ferramenta + nome da ferramenta + rótulo "em execução...". Parâmetros mostrados em tooltip.
- **completed**: ícone de check + nome da ferramenta + rótulo "concluído". Área de resultado expansível com toggle "Mostrar mais/Mostrar menos" (truncado em 25 linhas) e botão de copiar.
- **error**: ícone de erro + nome da ferramenta + rótulo "erro". Resultado mostrado em vermelho.

Renderização especial para ferramentas conhecidas:

- **`edit_file` / `write_file`**: Mostra uma visão de diff unificado (oldStr em vermelho, newStr em verde) com fonte monoespaçada.
- **`update_plan`**: Não é renderizado como card — a store captura as entradas do plano separadamente e elas aparecem fixadas acima do input do chat via `activePlan`.

Chamadas de ferramenta são atualizadas inline: quando um evento `tool_result` chega, a store encontra o `tool_call` correspondente por `toolUseId` e define `status` e `result`.

### Mensagens `permission_request`

```js
{
  type: 'permission_request',
  requestId: string,
  toolName: string,
  reason: string,
  options: Array<{ optionId: string, name: string, kind: string }>,
  answerable: boolean,
  status: 'pending' | 'approved' | 'denied',
}
```

Dois caminhos de renderização dependendo de `answerable`:

- **Ao vivo (answerable=true)**: Renderizado por `PendingPermissionPanel.vue` fixado acima do input do chat. Mostra o nome da ferramenta, motivo, e as opções que o ACP realmente ofereceu (ex: "Permitir", "Permitir para sessão", "Recusar" — não um par fixo). O usuário clica em uma opção e `respondToPermission` entrega a resposta JSON-RPC.
- **Histórico (answerable=false)**: Renderizado inline na lista de mensagens como um card somente leitura por `PendingPermissionPanel.vue` ou como um badge por `PermissionDecisionBadge.vue`. Mostra o resultado se existir um `permission_decision` correspondente, senão mostra "expirado".

### Mensagens `permission_decision`

```js
{
  type: 'permission_decision',
  toolName: string,
  action: 'allow' | 'deny',
  reason: string,
}
```

Renderizadas por `PermissionDecisionBadge.vue` como um badge inline compacto. Vêm de decisões automáticas informativas que o modelo toma sem perguntar, ou da decisão do usuário sendo persistida e depois reproduzida do histórico.

### Mensagens `error`

```js
{ type: 'error', content: string }
```

Renderizadas por `ErrorMessage.vue` como uma bolha de erro inline com ícone de aviso. Tipicamente mostradas quando o processo do zero crasha inesperadamente.

## Árvore de componentes

```
ChatView.vue
├── WorkingIndicator.vue          (barra de status global)
├── TextMessage.vue               (type: text)
├── ThinkingBlock.vue             (type: thinking — barra compacta ou expansível)
├── ToolCallMessage.vue           (type: tool_call — running/completed/error com visão de diff)
├── PendingPermissionPanel.vue    (type: permission_request — respondível ou somente leitura)
├── PermissionDecisionBadge.vue   (type: permission_decision — badge compacto)
├── ErrorMessage.vue              (type: error)
└── q-chat-message                (streaming — currentResponse)
```

Mais, acima/abaixo da lista de mensagens:

```
ChatView.vue / IndexPage.vue
├── PendingPermissionPanel.vue    (fixado acima do input enquanto permissão ao vivo está pendente)
└── ChatInput.vue
    ├── Checklist do plano         (inline: fixado acima do input enquanto plano ativo)
    ├── Barra de status            (barra colorida com status thinking/tool/writing/sending)
    ├── Prévia de anexo            (miniatura de imagem ou chip de arquivo com botão remover)
    ├── Botão anexar               (diálogo nativo de arquivos → read_file_attachment)
    ├── Seletor de modo de execução (auto / perguntar / plano — ver doc do Modo Plano)
    ├── Seletor de modelo          (dropdown com busca, recentes, indicador de ativo)
    └── Botão Enviar / Cancelar    (arrow_upward quando ocioso, pause quando executando)
```

Todos os componentes vivem em `src/components/chat/`.

## Fluxo de permissão

1. O agente envia uma requisição JSON-RPC `session/request_permission` via ACP.
2. O bridge Rust traduz, atribui um `correlation_id`, persiste a requisição no histórico local, e emite `zero:permission-request` para o frontend.
3. A store cria uma mensagem `permission_request` com `status: 'pending'` e `answerable: true`.
4. `PendingPermissionPanel.vue` renderiza fixado acima do input do chat com as opções que o ACP ofereceu.
5. Usuário clica em uma opção → store chama `respondToPermission(requestId, optionId)`.
6. A store atualiza `status` e `chosenOptionId` da mensagem, e invoca o comando Tauri.
7. O bridge Rust busca a requisição pendente por `correlation_id`, persiste um `permission_decision` no histórico local, e envia a resposta JSON-RPC ao agente.
8. O agente recebe a decisão e continua ou aborta a chamada da ferramenta.

Não existe mais um atalho de auto-permitir client-side: o seletor de modo de execução com três opções (`auto` / `ask` / `plan`) é o `session/set_mode` real do ACP, reforçado pelo próprio motor — em modo `auto` o motor já executa ferramentas seguras sem perguntar e só pede permissão nas genuinamente arriscadas, então todo `session/request_permission` que o frontend recebe é um que o próprio motor decidiu valer a pena perguntar. Ver [Modo Plano](./plan-mode.md) para o mecanismo completo de troca de modo e sua persistência por sessão.

## Sistema de plano

A chamada de ferramenta `update_plan` é tratada especialmente: em vez de renderizar um card de ferramenta, a store atualiza `currentPlan` com as entradas do evento `plan_update`. O getter `activePlan` retorna `null` quando todos os itens estão concluídos, auto-ocultando a checklist.

A checklist do plano é renderizada:

- **Inline no `ChatInput.vue`**: Fixada acima do textarea enquanto ativa. Mostra cada item com ícone de status (pending → `radio_button_unchecked`, in_progress → `autorenew`, completed → `check_circle` com texto tachado).
- **No `PlanPanel.vue`**: Componente de painel independente, usado em layouts alternativos.

## Gerenciamento de estado

Desde o chat paralelo multi-sessão (ver [ADR 004](../architecture/decisions/004-multi-session-parallel.md)), esse estado por conversa vive na store **por sessão** `zero-session-store.js` (a store factory `useZeroSessionStore(key)`) — uma instância por painel aberto —, não na `zero-store.js` global:

- `messages[]` — lista de mensagens tipadas
- `currentResponse` — buffer de texto em streaming
- `currentThinking` — buffer de pensamento em streaming
- `currentPlan` — entradas do plano do agente (substituído por completo a cada `plan_update`)
- `workingStatus` getter — retorna `'thinking'`, `{ type: 'tool', toolName }`, `'writing'`, `'sending'`, ou `null`. Usado pela barra de status do `ChatInput.vue`.

A store global singleton `zero-store.js` mantém apenas estado de escopo do app: `activeModel` / `availableModels`, que alimentam o seletor de cada sessão. O modo de execução (`auto` / `ask` / `plan`) é estado por sessão em `zero-session-store.js` — ver [Modo Plano](./plan-mode.md).

Streaming é finalizado em mensagens permanentes quando:

- Pensamento: o próximo evento não-`reasoning` chega (`text`, `tool_call`, `permission_request`, `run_end`, `error`).
- Texto: evento `run_end` chega (ou o processo termina).

## Anexos de arquivo

O input do chat inclui um botão de anexar que abre o diálogo nativo de arquivos filtrado para extensões suportadas (imagens: png/jpg/gif/webp; texto/código: txt, md, csv, json, yaml, xml, html, css, js, ts, jsx, tsx, py, go, rs, java, kt, swift, c, cpp, rb, php, sh, sql, dockerfile).

Após a seleção:

1. `readFileAttachment(path)` é chamado no lado Rust, que lê o arquivo, valida o tamanho (máx 10 MB), detecta imagem vs. texto pela extensão, e retorna codificado em base64.
2. O frontend renderiza uma prévia: miniatura para imagens, chip de arquivo (ícone + nome + MIME type) para texto/código.
3. Ao enviar, o anexo é passado junto com o conteúdo da mensagem para `send_zero_message`.
4. O bridge constrói blocos ACP: imagens viram `{"type":"image","mimeType":...,"data":...}`, arquivos de texto viram `{"type":"text","text":"<attached file name=...>\n...\n</attached file>"}`.

## i18n

Chaves de tradução do chat em `src/i18n/`:

| Chave                     | pt-BR                    | en-US               |
| ------------------------- | ------------------------ | ------------------- |
| `chat.thinking`           | Pensamento               | Thinking            |
| `chat.thinkingRunning`    | Pensando...              | Thinking...         |
| `chat.toolRunning`        | em execução...           | running...          |
| `chat.toolCompleted`      | concluído                | completed           |
| `chat.writing`            | Escrevendo resposta...   | Writing response... |
| `chat.sending`            | Enviando...              | Sending...          |
| `chat.showMore`           | Mostrar mais             | Show more           |
| `chat.showLess`           | Mostrar menos            | Show less           |
| `chat.copy`               | Copiar                   | Copy                |
| `chat.permissionRequired` | Permissão necessária     | Permission required |
| `chat.approve`            | Aprovar                  | Approve             |
| `chat.deny`               | Recusar                  | Deny                |
| `chat.cancelRun`          | Cancelar execução        | Cancel run          |
| `chat.attachFile`         | Anexar arquivo           | Attach file         |
| `chat.removeAttachment`   | Remover anexo            | Remove attachment   |
| `chat.modelLabel`         | Modelo                   | Model               |
| `chat.switchModel`        | Trocar modelo            | Switch model        |
| `chat.searchModel`        | Buscar modelo...         | Search model...     |
| `chat.recentModels`       | Recentes                 | Recent              |
| `chat.loadingModels`      | Carregando modelos...    | Loading models...   |
| `chat.noModelsMatch`      | Nenhum modelo encontrado | No models match     |
| `chat.autoAllow`          | Auto                     | Auto                |
| `chat.ask`                | Perguntar                | Ask                 |

## Referências

- [zero-bridge: Conexão com o zero CLI](./zero-bridge.md)
- [Sistema de Sessões](./session-system.md)
- [Arquitetura de Conexão](../architecture/connection.md)
