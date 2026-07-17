# File Attachments

This document describes how zero-desktop handles file attachments — images and text/code files that users can attach to chat messages and send to the zero agent.

## Overview

Users can attach **any file** to a message via a native file dialog — there is no extension allowlist on either side (frontend dialog or backend). Every file is classified into one of three kinds:

- **Images**: png, jpg/jpeg, gif, webp — sent to the agent as ACP image content blocks so the model can see them.
- **Text/code files**: a curated extension list (txt, md, csv, json, yaml/yml, xml, html/htm, css, js, ts, jsx, tsx, py, go, rs, java, kt, swift, c, cpp/cc/cxx/h/hpp, rb, php, sh, sql, dockerfile) plus, for anything else, a content sniff (valid UTF-8, no null byte) — this is what makes extensionless project files (`Dockerfile`, `Makefile`, `.gitignore`, `.env`, lockfiles) and unrecognized-but-plain-text extensions attach correctly. Sent as text content blocks wrapped in an `<attached file>` tag so the agent can read their contents.
- **Binary**: anything that's neither a recognized image nor text-like content (PDFs, archives, office documents, executables, ...). Still attached and shown in the composer like any other file, but since no ACP content block can carry raw binary bytes to the model, it's sent as a named reference (`<attached file name="..." type="...">[binary file - content not included]</attached file>`) rather than inlined content — the agent knows a file was attached, just not what's inside it.

Files are limited to **10 MB**; that's the only remaining rejection (a size cap, unrelated to type).

## Data Flow

```
┌──────────────────────────────┐
│  Frontend                     │
│  ChatInput.vue                │
│  Attach button                │
│    → native file dialog       │
│    → readFileAttachment(path) │
└──────────────┬───────────────┘
               │ Tauri invoke
┌──────────────▼───────────────┐
│  Rust: read_file_attachment   │
│    → check file size ≤ 10MB   │
│    → detect kind: extension,  │
│      then content sniff,      │
│      never rejects            │
│    → read bytes               │
│    → base64-encode            │
│    → return FileAttachment    │
└──────────────┬───────────────┘
               │ mimeType, data (base64), name
┌──────────────▼───────────────┐
│  Frontend                     │
│  ChatInput.vue                │
│  Preview: image thumbnail or  │
│           file chip           │
│  User sends message           │
│    → sendZeroMessage(content, │
│        file)                  │
└──────────────┬───────────────┘
               │ Tauri invoke
┌──────────────▼───────────────┐
│  Rust: ZeroBridge::send       │
│    → build_prompt_blocks()    │
│      image  → {type:"image",  │
│        mimeType, data}        │
│      text   → {type:"text",   │
│        text:"<attached file>"}│
│      binary → {type:"text",   │
│        text:"<attached file   │
│        ...content not         │
│        included>"}            │
│    → session/prompt via ACP   │
│    → append user message to   │
│      local history (with file)│
└──────────────────────────────┘
```

## Rust Backend

### `read_file_attachment` (`lib.rs`)

```
Tauri command: read_file_attachment(path: String) → FileAttachment
```

**Steps:**

1. **Check size**: Reads file metadata (`tokio::fs::metadata`). Files over `MAX_FILE_BYTES` (10 MB) are rejected with a human-readable error showing the actual size. This is the only remaining rejection — nothing is rejected by type.
2. **Read bytes**: Reads the entire file into memory via `tokio::fs::read`.
3. **Detect kind**: `attachment_kind_for_file(path, bytes)` (never fails) decides `AttachmentKind::Image` / `Text` / `Binary` plus a MIME type:
   - A recognized image/text extension (`attachment_kind_from_extension`) wins outright — unless it's a "text" extension whose content actually contains a null byte, in which case it's downgraded to `Binary` (a `.txt` that's secretly binary shouldn't be force-decoded as UTF-8).
   - Otherwise a recognized binary/document extension (`binary_mime_for`: pdf, zip, gz/tgz, doc/docx, xls/xlsx, ppt/pptx) wins by extension alone — deliberately checked _before_ sniffing bytes, since e.g. a PDF's leading bytes (`%PDF-1.4...`) are valid ASCII and would otherwise be misclassified as text.
   - Otherwise, content is sniffed: valid UTF-8 with no null byte → `Text` (`text/plain`) — this is what makes extensionless files (`Dockerfile`, `.gitignore`, `.env`, lockfiles) and unrecognized-but-plain-text extensions attach as readable text.
   - Anything left → `Binary` (`application/octet-stream`).
4. **Encode**: Base64-encodes the bytes using the `base64` crate (standard engine, no padding variations).
5. **Extract name**: Takes the file name from the path (e.g. `screenshot.png`).

**FileAttachment struct:**

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileAttachment {
    pub mime_type: String,  // e.g. "image/png", "text/x-python"
    pub data: String,       // base64-encoded, no data: prefix
    pub name: String,       // original file name
}
```

### Curated extensions (in `attachment_kind_from_extension` / `binary_mime_for`)

These extensions get an exact, specific MIME type. Everything else falls through to content sniffing (see above) rather than being rejected.

| Kind   | Extensions                                                                                                                                               |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Image  | png, jpg, jpeg, gif, webp                                                                                                                                |
| Text   | txt, md, csv, json, yaml, yml, xml, html, htm, css, js, ts, jsx, tsx, py, go, rs, java, kt, swift, c, cpp, cc, cxx, h, hpp, rb, php, sh, sql, dockerfile |
| Binary | pdf, zip, gz, tgz, doc, docx, xls, xlsx, ppt, pptx                                                                                                       |

The MIME type for text/code files uses `text/x-<language>` for most languages (e.g. `text/x-python`, `text/x-rust`), with standard types for well-known formats (`text/markdown`, `text/csv`, `application/json`, etc.). Binary extensions get their real document MIME type (e.g. `application/pdf`); anything unrecognized falls back to `application/octet-stream`.

### Prompt block building (`bridge.rs`)

`build_prompt_blocks(content, file?)` constructs the ACP `prompt` array for `session/prompt`:

```rust
fn build_prompt_blocks(content: &str, file: Option<&FileAttachment>) -> Vec<serde_json::Value>
```

**Rules:**

1. If `content` is non-empty, a `{"type": "text", "text": content}` block is added first.
2. If a file is attached, `attachment_kind_from_mime()` (never `None` - defaults to `Binary`) decides how:
   - **Image** (`image/*`): `{"type": "image", "mimeType": "...", "data": "..."}`
   - **Text** (`text/*`, `application/json`, `application/yaml`, `application/xml`): the base64 data is decoded to UTF-8 and wrapped in an `<attached file>` XML tag:
     ```json
     {
       "type": "text",
       "text": "<attached file name=\"notes.md\" type=\"text/markdown\">\n...content...\n</attached file>"
     }
     ```
   - **Binary** (anything else - PDFs, archives, office documents, ...): there is no ACP content block that can carry raw binary bytes, so instead of silently dropping the attachment, a text block names it without inlining content:
     ```json
     {
       "type": "text",
       "text": "<attached file name=\"report.pdf\" type=\"application/pdf\">\n[binary file - content not included]\n</attached file>"
     }
     ```

`attachment_kind_from_mime()` on the Rust side (in `bridge.rs`) mirrors the Rust-side `AttachmentKind` classification but works from the already-resolved MIME type at send time, so the same file is correctly routed to the right ACP block type.

### History persistence

When a message with an attachment is sent, the user message in the local history (`session-history/<id>.jsonl`) includes a `file` key:

```json
{
  "type": "message",
  "payload": {
    "role": "user",
    "content": "What does this screenshot show?",
    "file": {
      "mimeType": "image/png",
      "data": "iVBORw0KGgo...",
      "name": "screenshot.png"
    }
  },
  "createdAt": "1720000000000"
}
```

Text-only messages omit the `file` key, keeping history entries identical to before file attachments existed.

## Frontend

### `zero.js` service

```js
export async function readFileAttachment(path) {
  return invoke("read_file_attachment", { path });
}
```

Returns a `FileAttachment` object (`{ mimeType, data, name }`).

### `ChatInput.vue` — Attach flow

1. User clicks the attach button (paperclip icon).
2. Native file dialog opens (`@tauri-apps/plugin-dialog`) with no `filters` — every file is selectable, not just a curated extension list.
3. On file selection, `readFileAttachment(path)` is called.
4. The frontend renders a preview based on MIME type:
   - **Image**: thumbnail using `base64ToObjectUrl()` from `src/utils/image.js` — converts base64 to a `blob:` URL to avoid bloating Vue's reactive state and DOM attributes with multi-MB base64 strings.
   - **Text/code/binary**: file chip showing the icon (from `getFileIcon()` in `src/utils/file.js`, defaulting to a generic file icon for anything unrecognized), file name, and MIME type. The chip doesn't need to know whether the content is readable text or opaque binary - `read_file_attachment` never fails by type, so this rendering path is unchanged either way.
5. A remove button (✕) lets the user detach the file before sending.
6. On send, the attachment is passed to `sendZeroMessage(key, content, file)` — `key` routes it to the correct session's process.

### File preview utilities

**`src/utils/file.js`:**

| Function               | Description                                                                                                                                  |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `isImageMimeType(m)`   | `true` if MIME type starts with `image/`.                                                                                                    |
| `isTextMimeType(m)`    | `true` for `text/*`, `application/json`, `application/yaml`, `application/xml`.                                                              |
| `getFileIcon(m, name)` | Returns a Material icon name based on MIME type or file extension (e.g. `"code"` for `.py`, `"javascript"` for `.js`, `"image"` for images). |

**`src/utils/image.js`:**

| Function                    | Description                                                                                               |
| --------------------------- | --------------------------------------------------------------------------------------------------------- |
| `base64ToObjectUrl(b64, m)` | Decodes base64 to a `blob:` URL via `Blob` + `URL.createObjectURL`. Caller must revoke the URL when done. |
| `base64ToUint8Array(b64)`   | Decodes standard base64 to `Uint8Array` via `atob`.                                                       |
| `base64ToDataUri(b64, m)`   | Builds a `data:` URI from base64 data. Fallback when blob URLs fail.                                      |

### History replay

When replaying a session from history, `buildMessagesFromHistory` — a per-session method on `zero-session-store.js` (each open panel replays its own history independently) — recognizes the `file` key on user `message` events and passes it to `addUserMessage(content, file)`, which stores it on the message object. `TextMessage.vue` renders the file preview (image or file chip) above the message text, same as live messages.

## Limitations

- **No drag-and-drop**: Files must be selected via the native dialog. Browser drag-and-drop APIs don't apply in a Tauri webview context.
- **Single file per message**: The ACP `session/prompt` interface accepts multiple content blocks, but the current UI only supports one attachment per message.
- **No streaming attachment read**: The entire file is read into memory before encoding. For the 10 MB limit this is acceptable; larger files would need a different approach.
- **Binary content isn't actually readable by the agent**: attaching a PDF, zip, or other binary file always succeeds, but the agent only ever sees its name and MIME type, not its content - there is no PDF/archive extraction on this path (unlike, say, the agent's own file-reading tools, which are a separate mechanism). This is a deliberate "attach anything, degrade honestly" tradeoff rather than pretending to support content it can't extract.

## References

- [zero-bridge: Connection to the zero CLI](./zero-bridge.md)
- [Chat Interface](./chat-interface.md)
- [Connection Architecture](../architecture/connection.md)
