# Arquitetura de Conexão

Este documento descreve como o **zero-desktop** se conecta ao agente de código [zero](https://github.com/Gitlawb/zero) sem conflitar com seu ciclo de vida ou exigir modificações no código do zero.

## 1. Visão Geral

O zero-desktop atua como um **cliente gráfico** para o zero. Ele não implementa a lógica do agente; apenas orquestra o binário `zero` já instalado na máquina do usuário.

A comunicação usa **`zero acp`** - o zero servindo o [Agent Client Protocol](https://agentclientprotocol.com) (JSON-RPC 2.0, JSON delimitado por linha sobre stdio), a mesma interface que o zero expõe para integração com editores (Zed, Neovim, ...). Isso substituiu um design anterior baseado em `zero exec --input-format stream-json` (ver [ADR 003](./decisions/003-migrate-to-acp.md)): `zero exec` é um comando batch de execução única que lê o stdin até EOF antes de agir sobre qualquer coisa, então não havia canal pra entregar nada de volta no meio do turno - pedidos de permissão nunca conseguiam chegar ao usuário de verdade. O ACP mantém o processo vivo durante toda a conversa e deixa o agente nos mandar uma requisição (`session/request_permission`) que respondemos pela mesma conexão.

```text
┌─────────────────────────────────────┐
│         Frontend Quasar (Vue)        │
│  - Interface de chat                 │
│  - Histórico de execução             │
│  - Pedidos de permissão (de verdade) │
└─────────────┬───────────────────────┘
              │ Comandos/eventos Tauri
┌─────────────▼───────────────────────┐
│           Núcleo Tauri (Rust)        │
│  - locator (encontra o binário zero) │
│  - acp (peer JSON-RPC 2.0)           │
│  - bridge (ZeroBridge: ciclo de vida │
│    da sessão + tradução de eventos)  │
└─────────────┬───────────────────────┘
              │ stdin / stdout / stderr (JSON-RPC, delimitado por linha)
┌─────────────▼───────────────────────┐
│        zero acp (processo filho)     │
│  - um processo por sessão ativa      │
│  - binário zero do PATH ou cache     │
└─────────────────────────────────────┘
```

## 2. Componentes do Backend Rust

### 2.1 `locator`

Localiza o binário `zero` no sistema (PATH, depois o diretório de cache do zero-desktop) e lê sua versão via `zero --version`.

### 2.2 `acp` - o peer JSON-RPC

Um "peer" JSON-RPC 2.0 minimalista, feito à mão (não é um cliente ou servidor puro, já que o ACP exige os dois papéis na mesma conexão): manda requisições e espera suas respostas (`initialize`, `session/new`, `session/load`, `session/prompt`), e também sabe receber uma requisição _do_ agente (`session/request_permission`) e responder quando o usuário decidir. Notificações (`session/update`) são interpretadas do mesmo jeito e repassadas ao chamador sem esperar resposta.

### 2.3 `bridge` - `ZeroBridge`

Mantém **um processo `zero acp` por sessão ativa** (não compartilhado entre sessões/workspaces - o `zero` não tem método `session/cancel`, então interromper um turno significa matar o processo, e um processo compartilhado derrubaria toda outra conversa aberta junto). Responsabilidades:

- Sobe o `zero acp`, completa o handshake `initialize` e abre uma sessão (`session/new`, ou `session/load` ao retomar).
- Roda a única tarefa que lê o stdout do processo, traduzindo notificações `session/update` pro mesmo formato `{schemaVersion, type, ...payload}` que o app já renderiza (`text`, `reasoning`, `tool_call`, `tool_result`), e repassando `session/request_permission` como um evento distinto e respondível.
- Grava uma cópia de cada evento traduzido num arquivo de histórico local (ver 2.4) conforme acontece, já que o próprio log em disco do `zero` registra bem menos em modo ACP do que registrava em modo exec (confirmado diretamente: só entradas `message`, nada de chamadas de ferramenta/pensamento/permissão).
- Sobe o processo de novo e reconecta via `session/load` se ele foi morto (cancelamento, ou uma queda) e chega uma nova mensagem.

### 2.4 Histórico local de sessão

O zero já indexa sessões (`zero sessions list --json`) e grava seu próprio `~/.local/share/zero/sessions/<id>/events.jsonl`, mas em modo ACP esse arquivo só contém entradas `message`. Por isso o zero-desktop mantém seu **próprio** log mais rico por sessão em `<diretório de dados do app>/zero-desktop/session-history/<sessionId>.jsonl`, gravado pelo bridge junto com o repasse de eventos pro frontend. `load_session_history` prefere esse arquivo quando existe, caindo pro `events.jsonl` do próprio zero em sessões criadas antes dessa migração (ou criadas fora do zero-desktop).

## 3. Fluxo de Conversa

1. Usuário digita uma mensagem no frontend.
2. Frontend chama o comando Tauri `send_zero_message`.
3. `ZeroBridge` manda uma requisição `session/prompt` pelo peer da sessão atual e retorna imediatamente - a requisição só resolve quando o turno inteiro termina, então é aguardada numa tarefa de fundo em vez de bloquear o comando.
4. A tarefa leitora de stdout traduz cada notificação `session/update` num `zero:event`, transmitindo texto/pensamento/chamada de ferramenta/resultado pro frontend conforme acontece.
5. Se o agente precisar de uma permissão que não consegue decidir sozinho, ele manda uma requisição `session/request_permission` de verdade. O bridge repassa isso como `zero:permission-request` e mantém a requisição em aberto.
6. O frontend mostra as opções que o `zero` realmente ofereceu (não um par fixo aprovar/negar - o ACP pode oferecer coisas como "permitir", "permitir pra sessão", "recusar"). A escolha do usuário volta via `respond_to_permission`, que o bridge entrega como a resposta JSON-RPC da requisição ainda aberta - o agente realmente recebe e continua (ou para) de acordo.
7. Quando `session/prompt` resolve, o bridge emite um evento no formato `run_end` e atualiza a lista de sessões.

## 4. Recuperação de Sessão

- O zero indexa toda sessão (`zero sessions list --json`), independente do transporte.
- `session/load` reconecta a uma sessão pelo id (o equivalente ACP do `--resume`), usado tanto ao reabrir uma sessão antiga explicitamente quanto quando o bridge sobe o processo de novo silenciosamente após um cancelamento.
- O próprio log de histórico local do zero-desktop (seção 2.4) é o que faz reabrir uma sessão mostrar cards ricos de chamada de ferramenta/pensamento/permissão, já que o log do próprio zero em modo ACP não guarda esse detalhe.

## 5. Instalação do zero

Quando o locator não encontra o binário:

1. A UI mostra um assistente de instalação.
2. O usuário escolhe:
   - **Instalação global**: roda o script oficial de instalação do zero, colocando `zero` em `~/.local/bin` e atualizando o PATH.
   - **Instalação isolada**: baixa o binário pro cache do zero-desktop sem mexer no PATH ou em diretórios do sistema.
3. O zero-desktop nunca sobrescreve uma instalação existente do zero.

## 6. Decisões e Restrições

- **Usamos `zero acp`, não `zero exec`**, especificamente pra que pedidos de permissão possam ser respondidos de verdade - ver [ADR 003](./decisions/003-migrate-to-acp.md) pra comparação completa e o que foi verificado ao vivo contra a CLI.
- **Um processo `zero acp` por sessão**, não um único processo compartilhado pelo app - o zero não tem como cancelar um turno específico em andamento, então cancelamento é "matar o processo", e isso não deveria derrubar outras conversas abertas.
- **Não embutimos o binário do zero** no pacote do zero-desktop, pra preservar o ciclo de vida independente do zero.
- **Não modificamos o zero**; só usamos suas interfaces públicas.
- **Workspace único no alpha**: o alpha começa com um workspace. Suporte a múltiplos workspaces vem depois.

## 7. Referências

- [Agent Client Protocol](https://agentclientprotocol.com)
- [Fluxo de Atualização do Zero](https://github.com/Gitlawb/zero/blob/main/docs/UPDATE.md)
- [`update-model.md`](./update-model.md)
- [`decisions/001-connection-via-stream-json.md`](./decisions/001-connection-via-stream-json.md) (substituído)
- [`decisions/003-migrate-to-acp.md`](./decisions/003-migrate-to-acp.md)
