# 003 — Migrar o backbone de conexão de `zero exec` para `zero acp`

## Status

Aceito. Substitui [001 — Conectando ao Zero via Stream-JSON](./001-connection-via-stream-json.md).

## Contexto

O ADR 001 escolheu `zero exec --input-format stream-json --output-format stream-json` como backbone de conexão. Na prática, o `zero exec` se mostrou um **comando batch de execução única**: ele lê o stdin até EOF antes de agir sobre qualquer coisa (confirmado segurando o stdin aberto e observando o zero não produzir nenhuma atividade de stdout/rede, não importa quanto tempo se espere), então o zero-desktop precisava escrever a mensagem e fechar o stdin imediatamente pra um turno sequer rodar.

Consequência direta: não sobrava canal nenhum pra mandar qualquer coisa de volta pra dentro de um turno em andamento. Pedidos de permissão (`send_permission_decision`) eram impossíveis de implementar de verdade - o botão existia na UI, mas clicar sempre falhava, porque o processo que precisaria receber a decisão já não tinha mais stdin aberto. Testado exaustivamente em vários níveis de autonomia (`low`/`medium`/`high`) e tipos de ação (edição de arquivo, comando de shell, acesso à rede): o `zero exec` nunca, uma vez sequer, perguntou interativamente. Ele decidia sozinho (emitindo um evento informativo `permission_decision`) ou negava automaticamente, falhando a chamada de ferramenta com "Sandbox approval required".

## Opções Consideradas

### Opção A: Manter o `zero exec`, contornar a limitação do stdin

Não foi possível identificar um contorno. O comportamento de "EOF antes de processar" é fundamental a como o comando lê a entrada, não é uma flag ou questão de timing - confirmado testando com o stdin deliberadamente mantido aberto por períodos longos.

### Opção B: `zero acp`

O `zero acp` serve o [Agent Client Protocol](https://agentclientprotocol.com): JSON-RPC 2.0 sobre stdio, delimitado por linha (não usa framing por `Content-Length` como o LSP), feito pra integração com editores (Zed, Neovim, ...). Verificado diretamente contra a CLI real:

- O processo fica vivo durante toda a conversa - sem exigência de EOF.
- `session/new` / `session/load` / `session/prompt` funcionam como esperado; `session/prompt` transmite progresso via notificações `session/update` e só resolve quando o turno termina.
- **O agente pode nos mandar requisições no meio do turno.** `session/request_permission` é uma requisição JSON-RPC de verdade com um `id`; responder com `{"outcome":{"outcome":"selected","optionId":...}}` realmente desbloqueia o agente - provado de ponta a ponta tanto com um script Python descartável quanto, mais importante, com a implementação real em Rust do `AcpPeer` contra o binário de verdade.
- `session/load` funciona pra reconectar a uma sessão pelo id (o equivalente ACP do `--resume`).
- Não existe método `session/cancel` (`method not found`) - cancelar um turno significa matar o processo.
- O próprio log de sessão em disco do zero (`events.jsonl`) é _mais pobre_ em modo ACP do que era em modo exec: só entradas `message` são persistidas, nada da atividade de chamada de ferramenta/pensamento/permissão que o modo exec registrava.

### Opção C: `zero daemon`

Um worker em segundo plano com `run`/`attach`/roteamento de sessão, capaz de servir múltiplas sessões e até bridging remoto. Não foi seguido nessa migração - o ACP já resolve o problema concreto (entrega real de permissão) com um modelo de processo mais simples (um processo por sessão ativa vs. um daemon de longa duração pra gerenciar), e é feito sob medida pra exatamente esse caso de uso ("cliente externo conduz uma conversa com o agente") em vez de ser infraestrutura de sessão genérica.

## Decisão

Usar a **Opção B**: `zero acp` como backbone de conexão, substituindo o `zero exec`.

Modelo de processo: **um processo `zero acp` por sessão ativa**, não um único processo compartilhado pelo app. Como não existe `session/cancel`, interromper um turno significa matar o processo; um processo compartilhado derrubaria toda outra conversa aberta junto. Um processo por sessão mantém esse raio de impacto contido, e ainda assim é uma melhoria grande sobre o exec's "um processo por _mensagem_".

Pra cobrir a regressão de histórico de sessão (o log em disco mais pobre da Opção B), o zero-desktop agora grava seu **próprio** log rico por sessão (`<diretório de dados do app>/zero-desktop/session-history/<sessionId>.jsonl`) junto com o repasse de eventos ao vivo pro frontend, e lê dele com prioridade sobre o `events.jsonl` do próprio zero quando presente. Sessões criadas antes dessa migração (ou fora do zero-desktop) caem no caminho de leitura antigo sem mudança.

## Consequências

- `src-tauri/src/bridge.rs` foi reescrito em torno de uma conexão JSON-RPC persistente por sessão em vez de subir um `zero exec` por mensagem. `src-tauri/src/acp.rs` é um peer JSON-RPC 2.0 novo, minimalista e feito à mão (não é uma dependência - o protocolo é simples o suficiente, e nenhuma crate disponível suporta de forma limpa ser ao mesmo tempo remetente e receptor de requisições na mesma conexão, o que o ACP exige).
- Aprovar/negar permissão agora realmente chega ao agente. O frontend renderiza as opções que o ACP de fato ofereceu pra um pedido (ex: "Permitir", "Permitir pra sessão", "Recusar") em vez de um par fixo Aprovar/Negar.
- O comando antigo `send_permission_decision`, o `PermissionRequest.vue` (já órfão antes dessa migração), e um workaround client-side em localStorage pra lembrar decisões de permissão (necessário só porque decisões antes não conseguiam ser entregues) foram removidos.
- Cancelar um turno mata o processo daquela sessão; a próxima mensagem sobe o processo de novo e reconecta via `session/load`.
- A riqueza do histórico de sessão agora depende do próprio log do zero-desktop pra sessões criadas depois dessa migração; sessões bem antigas (ou criadas diretamente via CLI do `zero`, fora do zero-desktop) continuam funcionando pelo caminho de leitura anterior, mais pobre.
