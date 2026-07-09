# Connection Architecture

This document describes how **zero-desktop** connects to the [zero](https://github.com/Gitlawb/zero) coding agent without conflicting with its lifecycle or requiring modifications to the zero codebase.

## 1. Overview

zero-desktop acts as a **graphical client** for zero. It does not implement the agent logic itself; it only orchestrates the `zero` binary already installed on the user's machine.

Communication uses zero's **stream-json** protocol (`zero exec --input-format stream-json --output-format stream-json`), which is:

- **Public and documented** in [`STREAM_JSON_PROTOCOL.md`](https://github.com/Gitlawb/zero/blob/main/docs/STREAM_JSON_PROTOCOL.md).
- **Bidirectional** over stdin/stdout.
- **JSONL-based**: one event per line.
- **Suitable for interactive chat**, because it streams text, tool calls, permission requests, reasoning, and token usage.

```text
┌─────────────────────────────────────┐
│         Frontend Quasar (Vue)        │
│  - Chat UI                           │
│  - Execution history                 │
│  - Permission prompts                │
└─────────────┬───────────────────────┘
              │ Tauri commands / events
┌─────────────▼───────────────────────┐
│           Tauri Core (Rust)          │
│  - ZeroLocator                       │
│  - ProcessManager                    │
│  - ZeroBridge                        │
│  - SessionStore (local cache)        │
└─────────────┬───────────────────────┘
              │ stdin / stdout / stderr
┌─────────────▼───────────────────────┐
│      zero exec (child process)       │
│  - zero binary from PATH or cache    │
│  - Updated independently             │
└─────────────────────────────────────┘
```

## 2. Rust Backend Components

### 2.1 `ZeroLocator`

Responsible for locating the `zero` binary on the system.

Resolution order:

1. `zero` on the user's `PATH`.
2. Isolated zero-desktop cache (`%APP_DATA%/zero-desktop/bin/zero` on Windows, `~/.local/share/zero-desktop/bin/zero` on Linux, `~/Library/Application Support/zero-desktop/bin/zero` on macOS).
3. If not found, trigger the installation assistant.

It also collects the version via `zero --version` for compatibility checks.

### 2.2 `ProcessManager`

Manages the `zero exec` child process:

- Spawns the process with the correct arguments (`--input-format stream-json`, `--output-format stream-json`, `--cwd`, `--resume`, etc.).
- Keeps stdin open for continuous turns.
- Reads stdout/stderr line by line.
- Sends input events (`message`, `prompt`) as JSONL.
- Kills the process cleanly on cancellation.

### 2.3 `ZeroBridge`

Parses stream-json events and converts them into typed Tauri events.

Events emitted to the frontend:

| Tauri event | zero type | Description |
|---|---|---|
| `zero:run-start` | `run_start` | Run started |
| `zero:text` | `text` | Response text streaming |
| `zero:reasoning` | `reasoning` | Model reasoning |
| `zero:tool-call` | `tool_call` | Tool being invoked |
| `zero:permission-request` | `permission_request` | Permission request |
| `zero:permission-decision` | `permission_decision` | Permission decision |
| `zero:tool-result` | `tool_result` | Tool result |
| `zero:usage` | `usage` | Token usage |
| `zero:final` | `final` | Final complete response |
| `zero:run-end` | `run_end` | Run finished |
| `zero:error` | `error` | Run error |

### 2.4 `SessionStore` (local cache)

zero already persists sessions to disk. The zero-desktop `SessionStore` only:

- Indexes sessions for the UI (`zero sessions list --json`).
- Keeps lightweight metadata (title, workspace, model, date).
- Does not replace zero's session format.

Storage format: JSON files under `%APP_DATA%/zero-desktop/sessions/`.

## 3. Conversation Flow

1. User types a message in the frontend.
2. Frontend calls the Tauri command `send_message`.
3. `ProcessManager` writes to the `zero exec` stdin:
   ```json
   { "schemaVersion": 2, "type": "message", "role": "user", "content": "..." }
   ```
4. `ZeroBridge` reads stdout and emits Tauri events.
5. Frontend renders text streaming, tool calls, and permission requests.
6. On `run_end`, the conversation is finalized and metadata is saved.

## 4. Session Recovery

Concerns about recovering sessions via stream-json are understandable, but the protocol is **reliable** because:

- zero persists every turn to disk (`zero sessions list` can list them).
- `zero exec --resume <session-id>` continues an existing session.
- `zero exec --fork <session-id>` creates a branch.
- `run_start` events include `sessionId`, letting the GUI correlate the run with the correct session.

Therefore, even if the child process dies or the UI is closed, the session can be resumed from the latest state persisted by zero.

## 5. zero Installation

When `ZeroLocator` cannot find the binary:

1. The UI shows an installation assistant.
2. The user chooses:
   - **Global installation**: runs zero's official install script (e.g., `curl -fsSL .../install.sh | bash`), placing `zero` in `~/.local/bin` and updating PATH.
   - **Isolated installation**: downloads the binary to the zero-desktop cache without touching PATH or system directories.
3. zero-desktop never overwrites an existing zero installation.

## 6. Decisions and Constraints

- **We do not use `zero serve --mcp` as the backbone** because MCP stdio is tool-oriented, not a continuous chat with streaming.
- **We do not embed the zero binary** in the zero-desktop package to preserve zero's independent lifecycle.
- **We do not modify zero**; we only use its public interfaces.
- **Single workspace in alpha**: the alpha starts with one workspace. Multi-workspace support will be added later.

## 7. References

- [Zero Stream-JSON Protocol](https://github.com/Gitlawb/zero/blob/main/docs/STREAM_JSON_PROTOCOL.md)
- [Zero Update Flow](https://github.com/Gitlawb/zero/blob/main/docs/UPDATE.md)
- [`update-model.md`](./update-model.md)
- [`decisions/001-connection-via-stream-json.md`](./decisions/001-connection-via-stream-json.md)
