# Anexos de Arquivo

Este documento descreve como o zero-desktop lida com anexos de arquivo — imagens e arquivos de texto/código que os usuários podem anexar a mensagens de chat e enviar ao agente zero.

## Visão Geral

Usuários podem anexar arquivos a mensagens via um diálogo nativo de arquivos. Os tipos suportados incluem:

- **Imagens**: png, jpg/jpeg, gif, webp — enviadas ao agente como blocos de conteúdo ACP de imagem para que o modelo possa vê-las.
- **Arquivos de texto/código**: txt, md, csv, json, yaml/yml, xml, html/htm, css, js, ts, jsx, tsx, py, go, rs, java, kt, swift, c, cpp/cc/cxx/h/hpp, rb, php, sh, sql, dockerfile — enviados como blocos de texto envoltos em tag `<attached file>` para que o agente possa ler seu conteúdo.

Arquivos são limitados a **10 MB**. Arquivos binários (bytes nulos no conteúdo) que tenham extensão de texto são rejeitados.

## Fluxo de Dados

```
┌──────────────────────────────┐
│  Frontend                     │
│  ChatInput.vue                │
│  Botão anexar                 │
│    → diálogo nativo           │
│    → readFileAttachment(path) │
└──────────────┬───────────────┘
               │ Tauri invoke
┌──────────────▼───────────────┐
│  Rust: read_file_attachment   │
│    → verifica tamanho ≤ 10MB  │
│    → detecta tipo por extensão│
│    → lê bytes                 │
│    → valida (sem binário em   │
│      arquivos texto)          │
│    → codifica em base64       │
│    → retorna FileAttachment   │
└──────────────┬───────────────┘
               │ mimeType, data (base64), name
┌──────────────▼───────────────┐
│  Frontend                     │
│  ChatInput.vue                │
│  Prévia: miniatura ou chip    │
│  Usuário envia mensagem       │
│    → sendZeroMessage(content, │
│        file)                  │
└──────────────┬───────────────┘
               │ Tauri invoke
┌──────────────▼───────────────┐
│  Rust: ZeroBridge::send       │
│    → build_prompt_blocks()    │
│      imagem → {type:"image",  │
│        mimeType, data}        │
│      texto  → {type:"text",   │
│        text:"<attached file>"}│
│    → session/prompt via ACP   │
│    → anexa mensagem ao        │
│      histórico local          │
└──────────────────────────────┘
```

## Backend Rust

### `read_file_attachment` (`lib.rs`)

```
Comando Tauri: read_file_attachment(path: String) → FileAttachment
```

**Etapas:**

1. **Verifica tamanho**: Lê metadados do arquivo (`tokio::fs::metadata`). Arquivos acima de `MAX_FILE_BYTES` (10 MB) são rejeitados.
2. **Detecta tipo**: `attachment_kind_from_extension()` mapeia a extensão para `AttachmentKind::Image` ou `AttachmentKind::Text` com o MIME type correspondente. Extensões desconhecidas são rejeitadas.
3. **Lê bytes**: Lê o arquivo inteiro em memória via `tokio::fs::read`.
4. **Valida texto**: Para arquivos de texto, verifica se o conteúdo contém bytes nulos. Se sim, rejeita como dado binário.
5. **Codifica**: Base64-encoda os bytes usando crate `base64`.
6. **Extrai nome**: Obtém o nome do arquivo do caminho (ex: `screenshot.png`).

**Estrutura FileAttachment:**

```rust
pub struct FileAttachment {
    pub mime_type: String,  // ex: "image/png", "text/x-python"
    pub data: String,       // base64, sem prefixo data:
    pub name: String,       // nome original do arquivo
}
```

### Extensões suportadas

| Tipo   | Extensões                                                                                                                                                |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Imagem | png, jpg, jpeg, gif, webp                                                                                                                                |
| Texto  | txt, md, csv, json, yaml, yml, xml, html, htm, css, js, ts, jsx, tsx, py, go, rs, java, kt, swift, c, cpp, cc, cxx, h, hpp, rb, php, sh, sql, dockerfile |

### Construção de blocos de prompt (`bridge.rs`)

`build_prompt_blocks(content, file?)` monta o array `prompt` do ACP para `session/prompt`:

**Regras:**

1. Se `content` não estiver vazio, um bloco `{"type": "text", "text": content}` é adicionado primeiro.
2. Se um arquivo for anexado e seu MIME type reconhecido:
   - **Imagem** (`image/*`): `{"type": "image", "mimeType": "...", "data": "..."}`
   - **Texto** (`text/*`, `application/json`, `application/yaml`, `application/xml`): os dados base64 são decodificados para UTF-8 e envoltos em tag XML `<attached file>`.

### Persistência no histórico

Quando uma mensagem com anexo é enviada, a mensagem do usuário no histórico local inclui uma chave `file`:

```json
{
  "type": "message",
  "payload": {
    "role": "user",
    "content": "O que mostra essa screenshot?",
    "file": {
      "mimeType": "image/png",
      "data": "iVBORw0KGgo...",
      "name": "screenshot.png"
    }
  }
}
```

Mensagens só de texto omitem a chave `file`, mantendo as entradas de histórico idênticas a antes da existência de anexos.

## Frontend

### Serviço `zero.js`

```js
export async function readFileAttachment(path) {
  return invoke("read_file_attachment", { path });
}
```

### `ChatInput.vue` — Fluxo de anexo

1. Usuário clica no botão anexar (ícone de clipe).
2. Diálogo nativo de arquivos abre (`@tauri-apps/plugin-dialog`), filtrado para extensões suportadas.
3. Ao selecionar o arquivo, `readFileAttachment(path)` é chamado.
4. O frontend renderiza uma prévia baseada no MIME type:
   - **Imagem**: miniatura usando `base64ToObjectUrl()` de `src/utils/image.js` — converte base64 para URL `blob:` para evitar inchar o estado reativo do Vue.
   - **Texto/código**: chip de arquivo mostrando ícone (de `getFileIcon()` em `src/utils/file.js`), nome do arquivo e MIME type.
5. Um botão remover (✕) permite desanexar o arquivo antes de enviar.
6. Ao enviar, o anexo é passado para `sendZeroMessage(key, content, file)` — `key` roteia para o processo da sessão correta.

### Utilitários de prévia

**`src/utils/file.js`:**

| Função                 | Descrição                                                                        |
| ---------------------- | -------------------------------------------------------------------------------- |
| `isImageMimeType(m)`   | `true` se MIME type começa com `image/`.                                         |
| `isTextMimeType(m)`    | `true` para `text/*`, `application/json`, `application/yaml`, `application/xml`. |
| `getFileIcon(m, name)` | Retorna ícone Material baseado no MIME type ou extensão do arquivo.              |

**`src/utils/image.js`:**

| Função                      | Descrição                                                              |
| --------------------------- | ---------------------------------------------------------------------- |
| `base64ToObjectUrl(b64, m)` | Decodifica base64 para URL `blob:` via `Blob` + `URL.createObjectURL`. |
| `base64ToUint8Array(b64)`   | Decodifica base64 padrão para `Uint8Array` via `atob`.                 |
| `base64ToDataUri(b64, m)`   | Constrói URI `data:` a partir de dados base64. Fallback.               |

### Reprodução de histórico

Ao reproduzir uma sessão do histórico, `buildMessagesFromHistory` — um método por sessão em `zero-session-store.js` (cada painel aberto reproduz seu próprio histórico de forma independente) — reconhece a chave `file` em eventos `message` de usuário e a passa para `addUserMessage(content, file)`. `TextMessage.vue` renderiza a prévia do arquivo acima do texto da mensagem, igual às mensagens ao vivo.

## Limitações

- **Sem drag-and-drop**: Arquivos devem ser selecionados via diálogo nativo.
- **Um arquivo por mensagem**: A interface ACP aceita múltiplos blocos, mas a UI atual suporta apenas um anexo por mensagem.
- **Leitura completa em memória**: O arquivo inteiro é lido antes da codificação. Para o limite de 10 MB isso é aceitável.

## Referências

- [zero-bridge: Conexão com o zero CLI](./zero-bridge.md)
- [Interface de Chat](./chat-interface.md)
- [Arquitetura de Conexão](../architecture/connection.md)
