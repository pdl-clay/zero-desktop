# Anexos de Arquivo

Este documento descreve como o zero-desktop lida com anexos de arquivo — imagens e arquivos de texto/código que os usuários podem anexar a mensagens de chat e enviar ao agente zero.

## Visão Geral

Usuários podem anexar **qualquer arquivo** a uma mensagem via diálogo nativo — não existe lista de extensões permitidas em nenhum dos dois lados (diálogo do frontend ou backend). Todo arquivo é classificado em um de três tipos:

- **Imagens**: png, jpg/jpeg, gif, webp — enviadas ao agente como blocos de conteúdo ACP de imagem para que o modelo possa vê-las.
- **Arquivos de texto/código**: uma lista curada de extensões (txt, md, csv, json, yaml/yml, xml, html/htm, css, js, ts, jsx, tsx, py, go, rs, java, kt, swift, c, cpp/cc/cxx/h/hpp, rb, php, sh, sql, dockerfile) mais, pra qualquer outra coisa, uma checagem de conteúdo (UTF-8 válido, sem byte nulo) — é isso que faz arquivos sem extensão do projeto (`Dockerfile`, `Makefile`, `.gitignore`, `.env`, lockfiles) e extensões desconhecidas mas de texto puro anexarem corretamente. Enviados como blocos de texto envoltos em tag `<attached file>` para que o agente possa ler seu conteúdo.
- **Binário**: qualquer coisa que não seja nem imagem reconhecida nem conteúdo parecido com texto (PDFs, arquivos compactados, documentos do Office, executáveis, ...). Ainda assim é anexado e aparece no composer como qualquer outro arquivo, mas como nenhum bloco de conteúdo ACP consegue carregar bytes binários crus até o modelo, ele é enviado como uma referência nomeada (`<attached file name="..." type="...">[binary file - content not included]</attached file>`) em vez de conteúdo inline — o agente sabe que um arquivo foi anexado, só não sabe o que tem dentro.

Arquivos são limitados a **10 MB**; essa é a única rejeição que resta (um limite de tamanho, sem relação com tipo).

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
│    → detecta tipo: extensão,  │
│      depois checa conteúdo,   │
│      nunca rejeita            │
│    → lê bytes                 │
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
│      imagem  → {type:"image", │
│        mimeType, data}        │
│      texto   → {type:"text",  │
│        text:"<attached file>"}│
│      binário → {type:"text",  │
│        text:"<attached file   │
│        ...content not         │
│        included>"}            │
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

1. **Verifica tamanho**: Lê metadados do arquivo (`tokio::fs::metadata`). Arquivos acima de `MAX_FILE_BYTES` (10 MB) são rejeitados. Essa é a única rejeição que resta — nada é rejeitado por tipo.
2. **Lê bytes**: Lê o arquivo inteiro em memória via `tokio::fs::read`.
3. **Detecta tipo**: `attachment_kind_for_file(path, bytes)` (nunca falha) decide `AttachmentKind::Image` / `Text` / `Binary` mais um MIME type:
   - Uma extensão de imagem/texto reconhecida (`attachment_kind_from_extension`) vence direto — a menos que seja uma extensão de "texto" cujo conteúdo na verdade tenha um byte nulo, caso em que rebaixa pra `Binary` (um `.txt` que secretamente é binário não deve ser forçado a decodificar como UTF-8).
   - Senão, uma extensão binária/de documento reconhecida (`binary_mime_for`: pdf, zip, gz/tgz, doc/docx, xls/xlsx, ppt/pptx) vence só pela extensão — checado deliberadamente _antes_ de sondar os bytes, já que os bytes iniciais de um PDF (`%PDF-1.4...`) são ASCII válido e seriam classificados incorretamente como texto.
   - Senão, o conteúdo é sondado: UTF-8 válido sem byte nulo → `Text` (`text/plain`) — é isso que faz arquivos sem extensão (`Dockerfile`, `.gitignore`, `.env`, lockfiles) e extensões desconhecidas mas de texto puro anexarem como texto legível.
   - O que sobrar → `Binary` (`application/octet-stream`).
4. **Codifica**: Base64-encoda os bytes usando crate `base64`.
5. **Extrai nome**: Obtém o nome do arquivo do caminho (ex: `screenshot.png`).

**Estrutura FileAttachment:**

```rust
pub struct FileAttachment {
    pub mime_type: String,  // ex: "image/png", "text/x-python"
    pub data: String,       // base64, sem prefixo data:
    pub name: String,       // nome original do arquivo
}
```

### Extensões curadas (em `attachment_kind_from_extension` / `binary_mime_for`)

Essas extensões ganham um MIME type exato e específico. Tudo o mais cai na checagem de conteúdo (ver acima) em vez de ser rejeitado.

| Tipo    | Extensões                                                                                                                                                |
| ------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Imagem  | png, jpg, jpeg, gif, webp                                                                                                                                |
| Texto   | txt, md, csv, json, yaml, yml, xml, html, htm, css, js, ts, jsx, tsx, py, go, rs, java, kt, swift, c, cpp, cc, cxx, h, hpp, rb, php, sh, sql, dockerfile |
| Binário | pdf, zip, gz, tgz, doc, docx, xls, xlsx, ppt, pptx                                                                                                       |

### Construção de blocos de prompt (`bridge.rs`)

`build_prompt_blocks(content, file?)` monta o array `prompt` do ACP para `session/prompt`:

**Regras:**

1. Se `content` não estiver vazio, um bloco `{"type": "text", "text": content}` é adicionado primeiro.
2. Se um arquivo for anexado, `attachment_kind_from_mime()` (nunca retorna "desconhecido" - assume `Binary` por padrão) decide como:
   - **Imagem** (`image/*`): `{"type": "image", "mimeType": "...", "data": "..."}`
   - **Texto** (`text/*`, `application/json`, `application/yaml`, `application/xml`): os dados base64 são decodificados para UTF-8 e envoltos em tag XML `<attached file>`.
   - **Binário** (qualquer outra coisa — PDFs, arquivos compactados, documentos do Office, ...): como não existe bloco de conteúdo ACP capaz de carregar bytes binários crus, em vez de descartar o anexo silenciosamente, um bloco de texto nomeia o arquivo sem incluir o conteúdo:
     ```json
     {
       "type": "text",
       "text": "<attached file name=\"relatorio.pdf\" type=\"application/pdf\">\n[binary file - content not included]\n</attached file>"
     }
     ```

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
2. Diálogo nativo de arquivos abre (`@tauri-apps/plugin-dialog`) sem `filters` — qualquer arquivo é selecionável, não só uma lista curada de extensões.
3. Ao selecionar o arquivo, `readFileAttachment(path)` é chamado.
4. O frontend renderiza uma prévia baseada no MIME type:
   - **Imagem**: miniatura usando `base64ToObjectUrl()` de `src/utils/image.js` — converte base64 para URL `blob:` para evitar inchar o estado reativo do Vue.
   - **Texto/código/binário**: chip de arquivo mostrando ícone (de `getFileIcon()` em `src/utils/file.js`, que cai num ícone genérico pra qualquer coisa não reconhecida), nome do arquivo e MIME type. O chip não precisa saber se o conteúdo é texto legível ou binário opaco — `read_file_attachment` nunca falha por tipo, então essa renderização é a mesma nos dois casos.
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
- **Conteúdo binário não é realmente lido pelo agente**: anexar um PDF, zip ou outro arquivo binário sempre funciona, mas o agente só vê o nome e o MIME type, não o conteúdo — não existe extração de PDF/arquivo compactado nesse caminho (diferente das próprias tools de leitura de arquivo do agente, que são um mecanismo separado). É uma escolha deliberada de "anexa qualquer coisa, mas degrada honestamente" em vez de fingir suportar conteúdo que não consegue extrair.

## Referências

- [zero-bridge: Conexão com o zero CLI](./zero-bridge.md)
- [Interface de Chat](./chat-interface.md)
- [Arquitetura de Conexão](../architecture/connection.md)
