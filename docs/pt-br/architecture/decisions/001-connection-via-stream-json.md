# 001 — Conexão com o Zero via Stream-JSON

## Status

Aceito

## Contexto

O zero-desktop precisa se comunicar com o agente de código [zero](https://github.com/Gitlawb/zero). O zero oferece três interfaces programáticas nativas:

1. **`zero exec --input-format stream-json --output-format stream-json`** — protocolo JSONL bidirecional.
2. **`zero serve --mcp`** — servidor MCP via stdio.
3. **TUI interativa (`zero`)** — não é programática.

Precisávamos escolher a interface principal para o GUI sem modificar o zero ou conflitar com suas atualizações.

## Opções Consideradas

### Opção A: `zero exec` com stream-json

Usar o protocolo oficial de JSONL via stdin/stdout.

**Prós:**

- Interface pública, documentada e estável.
- Suporta streaming de texto, tool calls, permissões, reasoning e uso de tokens.
- Suporta sessões (`--resume`, `--fork`).
- Não requer permissões especiais nem `--allow-unsafe-tools` para chat básico.
- Não exige modificar o zero.

**Contras:**

- Cada execução é um processo novo; conversas contínuas exigem stdin aberto ou `--resume`.
- Exige parsing de JSONL e gerenciamento de subprocesso no Rust.

### Opção B: `zero serve --mcp`

Usar o zero como servidor MCP stdio e o GUI como host MCP.

**Prós:**

- Padrão emergente para ferramentas de IA.
- Expõe ferramentas do zero de forma estruturada.

**Contras:**

- MCP stdio é orientado a chamadas de ferramenta, não a chat contínuo.
- Não transmite streaming da resposta do LLM.
- Não é a interface natural para uma experiência de conversação.

### Opção C: adicionar servidor HTTP/WebSocket no zero

Modificar o zero para expor uma API HTTP.

**Prós:**

- Seria a interface mais amigável para GUIs.

**Contras:**

- Exige fork e manutenção do zero.
- Conflita com as atualizações oficiais.
- Aumenta a superfície de ataque.

## Decisão

Usar **Opção A**: `zero exec` com stream-json como backbone da comunicação.

O MCP (`zero serve --mcp`) pode ser reconsiderado no futuro como forma de expor ferramentas do zero para outros usos, mas não como backbone do chat.

## Consequências

- O backend Rust precisa de um `ProcessManager` para spawnar e gerenciar o subprocesso.
- O frontend recebe eventos via Tauri events em vez de WebSocket.
- A recuperação de sessões é confiável porque o zero persiste sessões em disco.
- Não há dependência de modificações no zero.
