# Arquitetura de Conexão

Este documento descreve como o **zero-desktop** se conecta ao agente de código [zero](https://github.com/Gitlawb/zero) sem conflitar com seu ciclo de vida nem exigir modificações no código do zero.

## 1. Visão Geral

O zero-desktop atua como um **cliente gráfico** do zero. Ele não implementa a lógica do agente; apenas orquestra o binário `zero` já instalado na máquina do usuário.

A comunicação usa o protocolo **stream-json** do zero (`zero exec --input-format stream-json --output-format stream-json`), que é:

- **Público e documentado** em [`STREAM_JSON_PROTOCOL.md`](https://github.com/Gitlawb/zero/blob/main/docs/STREAM_JSON_PROTOCOL.md).
- **Bidirecional** via stdin/stdout.
- **Baseado em JSONL**: um evento por linha.
- **Adequado para chat interativo**, pois transmite streaming de texto, tool calls, permissões, reasoning e uso de tokens.

```text
┌─────────────────────────────────────┐
│         Frontend Quasar (Vue)        │
│  - Chat UI                           │
│  - Histórico de execução             │
│  - Prompts de permissão              │
└─────────────┬───────────────────────┘
              │ Tauri commands / events
┌─────────────▼───────────────────────┐
│           Tauri Core (Rust)          │
│  - ZeroLocator                       │
│  - ProcessManager                    │
│  - ZeroBridge                        │
│  - SessionStore (cache local)        │
└─────────────┬───────────────────────┘
              │ stdin / stdout / stderr
┌─────────────▼───────────────────────┐
│      zero exec (processo filho)      │
│  - Binário do zero no PATH/cache     │
│  - Atualizado independentemente      │
└─────────────────────────────────────┘
```

## 2. Componentes do Backend Rust

### 2.1 `ZeroLocator`

Responsável por localizar o binário `zero` no sistema.

Ordem de resolução:

1. `zero` no `PATH` do usuário.
2. Cache isolado do zero-desktop (`%APP_DATA%/zero-desktop/bin/zero` no Windows, `~/.local/share/zero-desktop/bin/zero` no Linux, `~/Library/Application Support/zero-desktop/bin/zero` no macOS).
3. Se não encontrar, aciona o assistente de instalação.

Também coleta a versão via `zero --version` para verificação de compatibilidade.

### 2.2 `ProcessManager`

Gerencia o subprocesso `zero exec`:

- Faz spawn com os argumentos corretos (`--input-format stream-json`, `--output-format stream-json`, `--cwd`, `--resume`, etc.).
- Mantém o stdin aberto para turnos contínuos.
- Lê stdout/stderr linha a linha.
- Envia eventos de entrada (`message`, `prompt`) como JSONL.
- Mata o processo de forma limpa em caso de cancelamento.

### 2.3 `ZeroBridge`

Faz o parsing dos eventos stream-json e os converte em eventos Tauri tipados.

Eventos emitidos para o frontend:

| Evento Tauri | Tipo do zero | Descrição |
|---|---|---|
| `zero:run-start` | `run_start` | Início da execução |
| `zero:text` | `text` | Streaming do texto de resposta |
| `zero:reasoning` | `reasoning` | Raciocínio do modelo |
| `zero:tool-call` | `tool_call` | Ferramenta sendo invocada |
| `zero:permission-request` | `permission_request` | Solicitação de permissão |
| `zero:permission-decision` | `permission_decision` | Decisão de permissão |
| `zero:tool-result` | `tool_result` | Resultado da ferramenta |
| `zero:usage` | `usage` | Uso de tokens |
| `zero:final` | `final` | Resposta final completa |
| `zero:run-end` | `run_end` | Fim da execução |
| `zero:error` | `error` | Erro da execução |

### 2.4 `SessionStore` (cache local)

O zero já persiste sessões em disco. O `SessionStore` do zero-desktop apenas:

- Indexa sessões para a UI (`zero sessions list --json`).
- Mantém metadados leves (título, workspace, modelo, data).
- Não substitui o formato de sessão do zero.

Formato de armazenamento: arquivos JSON em `%APP_DATA%/zero-desktop/sessions/`.

## 3. Fluxo de uma Conversa

1. O usuário digita uma mensagem no frontend.
2. O frontend chama o comando Tauri `send_message`.
3. O `ProcessManager` escreve no stdin do `zero exec`:
   ```json
   { "schemaVersion": 2, "type": "message", "role": "user", "content": "..." }
   ```
4. O `ZeroBridge` lê o stdout e emite eventos Tauri.
5. O frontend renderiza streaming de texto, tool calls e permissões.
6. Ao receber `run_end`, a conversa é finalizada e os metadados são salvos.

## 4. Recuperação de Sessões

A preocupação com a estabilidade de recuperar sessões via stream-json é compreensível, mas o protocolo é **confiável** porque:

- O próprio zero persiste cada turno em disco (`zero sessions list` pode listá-las).
- O comando `zero exec --resume <session-id>` continua uma sessão existente.
- O comando `zero exec --fork <session-id>` cria uma ramificação.
- Os eventos `run_start` trazem o `sessionId`, permitindo ao GUI correlacionar a execução com a sessão correta.

Portanto, mesmo que o processo filho morra ou a UI seja fechada, a sessão pode ser retomada a partir do último estado persistido pelo zero.

## 5. Instalação do zero

Quando o `ZeroLocator` não encontra o binário:

1. A UI exibe um assistente de instalação.
2. O usuário escolhe:
   - **Instalação global**: executa o script oficial do zero (ex.: `curl -fsSL .../install.sh | bash`), que coloca `zero` em `~/.local/bin` e atualiza o PATH.
   - **Instalação isolada**: baixa o binário para o cache do zero-desktop, sem alterar PATH ou diretórios do sistema.
3. O zero-desktop nunca sobrescreve uma instalação existente do zero.

## 6. Decisões e Restrições

- **Não usamos `zero serve --mcp` como backbone** porque MCP stdio é orientado a ferramentas, não a chat contínuo com streaming.
- **Não embutimos o binário do zero** no pacote do zero-desktop para preservar o ciclo de vida independente do zero.
- **Não modificamos o zero**; usamos apenas suas interfaces públicas.
- **Workspace único na alpha**: a alpha inicia com um único workspace. Suporte a múltiplos workspaces será adicionado posteriormente.

## 7. Referências

- [Zero Stream-JSON Protocol](https://github.com/Gitlawb/zero/blob/main/docs/STREAM_JSON_PROTOCOL.md)
- [Zero Update Flow](https://github.com/Gitlawb/zero/blob/main/docs/UPDATE.md)
- [`update-model.md`](./update-model.md)
- [`decisions/001-connection-via-stream-json.md`](./decisions/001-connection-via-stream-json.md)
