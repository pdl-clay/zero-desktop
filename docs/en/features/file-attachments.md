# File Attachments

This document describes how zero-desktop handles file attachments — images and text/code files that users can attach to chat messages and send to the zero agent.

## Overview

Users can attach files to messages via a native file dialog. Supported types include:

- **Images**: png, jpg/jpeg, gif, webp — sent to the agent as ACP image content blocks so the model can see them.
- **Text/code files**: txt, md, csv, json, yaml/yml, xml, html/htm, css, js, ts, jsx, tsx, py, go, rs, java, kt, swift, c, cpp/cc/cxx/h/hpp, rb, php, sh, sql, dockerfile — sent as text content blocks wrapped in an `<attached file>` tag so the agent can read their contents.

Files are limited to **10 MB**. Binary files (null bytes in content) that happen to have a text extension are rejected.

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
│    → detect kind by extension │
│    → read bytes               │
│    → validate (no binary in   │
│      text files)              │
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
│      image → {type:"image",   │
│        mimeType, data}        │
│      text  → {type:"text",    │
│        text:"<attached file>"}│
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

1. **Check size**: Reads file metadata (`tokio::fs::metadata`). Files over `MAX_FILE_BYTES` (10 MB) are rejected with a human-readable error showing the actual size.
2. **Detect kind**: `attachment_kind_from_extension()` maps the file extension to `AttachmentKind::Image` or `AttachmentKind::Text` with the corresponding MIME type. Unknown extensions are rejected.
3. **Read bytes**: Reads the entire file into memory via `tokio::fs::read`.
4. **Validate text**: For text files, checks if the content contains null bytes (`bytes.contains(&0)`). If so, rejects as binary data.
5. **Encode**: Base64-encodes the bytes using the `base64` crate (standard engine, no padding variations).
6. **Extract name**: Takes the file name from the path (e.g. `screenshot.png`).

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

### Supported extensions (in `attachment_kind_from_extension`)

| Kind  | Extensions                                                                                                                                               |
| ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Image | png, jpg, jpeg, gif, webp                                                                                                                                |
| Text  | txt, md, csv, json, yaml, yml, xml, html, htm, css, js, ts, jsx, tsx, py, go, rs, java, kt, swift, c, cpp, cc, cxx, h, hpp, rb, php, sh, sql, dockerfile |

The MIME type for text/code files uses `text/x-<language>` for most languages (e.g. `text/x-python`, `text/x-rust`), with standard types for well-known formats (`text/markdown`, `text/csv`, `application/json`, etc.).

### Prompt block building (`bridge.rs`)

`build_prompt_blocks(content, file?)` constructs the ACP `prompt` array for `session/prompt`:

```rust
fn build_prompt_blocks(content: &str, file: Option<&FileAttachment>) -> Vec<serde_json::Value>
```

**Rules:**

1. If `content` is non-empty, a `{"type": "text", "text": content}` block is added first.
2. If a file is attached and its MIME type is recognized:
   - **Image** (`image/*`): `{"type": "image", "mimeType": "...", "data": "..."}`
   - **Text** (`text/*`, `application/json`, `application/yaml`, `application/xml`): the base64 data is decoded to UTF-8 and wrapped in an `<attached file>` XML tag:
     ```json
     {
       "type": "text",
       "text": "<attached file name=\"notes.md\" type=\"text/markdown\">\n...content...\n</attached file>"
     }
     ```

The `attachment_kind_from_mime()` helper on the Rust side (in `bridge.rs`) mirrors `attachment_kind_from_extension()` but works from the already-resolved MIME type at send time, so the same file is correctly routed to the right ACP block type.

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
2. Native file dialog opens (`@tauri-apps/plugin-dialog`), filtered to supported extensions.
3. On file selection, `readFileAttachment(path)` is called.
4. The frontend renders a preview based on MIME type:
   - **Image**: thumbnail using `base64ToObjectUrl()` from `src/utils/image.js` — converts base64 to a `blob:` URL to avoid bloating Vue's reactive state and DOM attributes with multi-MB base64 strings.
   - **Text/code**: file chip showing the icon (from `getFileIcon()` in `src/utils/file.js`), file name, and MIME type.
5. A remove button (✕) lets the user detach the file before sending.
6. On send, the attachment is passed to `sendZeroMessage(content, file)`.

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

When replaying a session from history, `buildMessagesFromHistory` in `zero-store.js` recognizes the `file` key on user `message` events and passes it to `addUserMessage(content, file)`, which stores it on the message object. `TextMessage.vue` renders the file preview (image or file chip) above the message text, same as live messages.

## Limitations

- **No drag-and-drop**: Files must be selected via the native dialog. Browser drag-and-drop APIs don't apply in a Tauri webview context.
- **Single file per message**: The ACP `session/prompt` interface accepts multiple content blocks, but the current UI only supports one attachment per message.
- **No streaming attachment read**: The entire file is read into memory before encoding. For the 10 MB limit this is acceptable; larger files would need a different approach.

## References

- [zero-bridge: Connection to the zero CLI](./zero-bridge.md)
- [Chat Interface](./chat-interface.md)
- [Connection Architecture](../architecture/connection.md)
