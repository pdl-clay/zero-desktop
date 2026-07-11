# Connection Architecture

This document describes how **zero-desktop** connects to the [zero](https://github.com/Gitlawb/zero) coding agent without conflicting with its lifecycle or requiring modifications to the zero codebase.

## 1. Overview

zero-desktop acts as a **graphical client** for zero. It does not implement the agent logic itself; it only orchestrates the `zero` binary already installed on the user's machine.

Communication uses **`zero acp`** - zero serving the [Agent Client Protocol](https://agentclientprotocol.com) (JSON-RPC 2.0, newline-delimited JSON over stdio), the same interface zero exposes for editor integrations (Zed, Neovim, ...). This replaced an earlier design based on `zero exec --input-format stream-json` (see [ADR 003](./decisions/003-migrate-to-acp.md)): `zero exec` is a one-shot batch command that reads stdin to EOF before acting on anything, so there was no channel to deliver anything back mid-turn - permission prompts could never actually reach the user. ACP keeps the process alive for the whole conversation and lets the agent send _us_ a request (`session/request_permission`) that we reply to over the same connection.

```text
┌─────────────────────────────────────┐
│         Frontend Quasar (Vue)        │
│  - Chat UI                           │
│  - Execution history                 │
│  - Permission prompts (real ones)    │
└─────────────┬───────────────────────┘
              │ Tauri commands / events
┌─────────────▼───────────────────────┐
│           Tauri Core (Rust)          │
│  - locator (finds the zero binary)   │
│  - acp (JSON-RPC 2.0 peer)           │
│  - bridge (ZeroBridge: session       │
│    lifecycle + event translation)    │
└─────────────┬───────────────────────┘
              │ stdin / stdout / stderr (JSON-RPC, newline-delimited)
┌─────────────▼───────────────────────┐
│         zero acp (child process)     │
│  - one process per active session    │
│  - zero binary from PATH or cache    │
└─────────────────────────────────────┘
```

## 2. Rust Backend Components

### 2.1 `locator`

Locates the `zero` binary on the system (PATH, then the zero-desktop cache directory) and reads its version via `zero --version`.

### 2.2 `acp` - the JSON-RPC peer

A minimal, hand-rolled JSON-RPC 2.0 "peer" (not a client-only or server-only implementation, since ACP requires both roles over the same connection): sends requests and awaits their responses (`initialize`, `session/new`, `session/load`, `session/prompt`), while also being able to receive a request _from_ the agent (`session/request_permission`) and reply to it once the user decides. Notifications (`session/update`) are parsed the same way and handed to the caller with no reply expected.

### 2.3 `bridge` - `ZeroBridge`

Owns **one `zero acp` process per active session** (not shared across sessions/workspaces - `zero` has no `session/cancel` method, so interrupting a turn means killing the process, and a shared process would take every other open conversation down with it). Responsibilities:

- Spawns `zero acp`, completes the `initialize` handshake, and opens a session (`session/new`, or `session/load` when resuming).
- Runs the one task that reads the process's stdout, translating `session/update` notifications into the same `{schemaVersion, type, ...payload}` shape the app already renders (`text`, `reasoning`, `tool_call`, `tool_result`), and forwarding `session/request_permission` as a distinct, answerable event.
- Writes a copy of every translated event to a local history file (see 2.4) as it happens, since `zero`'s own on-disk session log records far less in ACP mode than it used to in exec mode (verified directly: only `message` entries, no tool calls/reasoning/permission activity).
- Respawns the process and reattaches via `session/load` if it was killed (cancel, or a crash) and a new message comes in.

### 2.4 Local session history

zero already indexes sessions (`zero sessions list --json`) and writes its own `~/.local/share/zero/sessions/<id>/events.jsonl`, but in ACP mode that file only contains `message` entries. So zero-desktop keeps its **own** richer log per session at `<app_data_dir>/zero-desktop/session-history/<sessionId>.jsonl`, written by the bridge alongside forwarding events to the frontend. `load_session_history` prefers this file when present, falling back to zero's own `events.jsonl` for sessions created before this existed (or created outside zero-desktop entirely).

## 3. Conversation Flow

1. User types a message in the frontend.
2. Frontend calls the Tauri command `send_zero_message`.
3. `ZeroBridge` sends a `session/prompt` request over the peer for the current session and returns immediately - the request only resolves once the whole turn ends, so it's awaited in a background task rather than blocking the command.
4. The stdout reader task translates each `session/update` notification into a `zero:event`, streaming text/reasoning/tool-call/tool-result to the frontend as it happens.
5. If the agent needs a permission it can't auto-decide, it sends a real `session/request_permission` request. The bridge surfaces this as `zero:permission-request` and holds the request open.
6. The frontend shows the options `zero` actually offered (not a fixed approve/deny - ACP can offer things like "allow", "allow for session", "reject"). The user's choice goes back via `respond_to_permission`, which the bridge delivers as the JSON-RPC reply to the still-open request - the agent genuinely receives it and continues (or stops) accordingly.
7. Once `session/prompt` resolves, the bridge emits a `run_end`-shaped event and refreshes the session list.

## 4. Session Recovery

- zero indexes every session (`zero sessions list --json`), regardless of transport.
- `session/load` reattaches to a session by id (the ACP equivalent of `--resume`), used both when explicitly reopening an old session and when the bridge silently respawns after a cancel.
- zero-desktop's own local history file (2.4) is what makes reopening a session show rich tool-call/reasoning/permission cards, since zero's own ACP-mode log doesn't retain that detail.

## 5. zero Installation

When the locator cannot find the binary:

1. The UI shows an installation assistant.
2. The user chooses:
   - **Global installation**: runs zero's official install script (e.g., `curl -fsSL .../install.sh | bash`), placing `zero` in `~/.local/bin` and updating PATH.
   - **Isolated installation**: downloads the binary to the zero-desktop cache without touching PATH or system directories.
3. zero-desktop never overwrites an existing zero installation.

## 6. Decisions and Constraints

- **We use `zero acp`, not `zero exec`**, specifically so permission requests can be answered for real - see [ADR 003](./decisions/003-migrate-to-acp.md) for the full comparison and what was verified live against the CLI.
- **One `zero acp` process per session**, not a single process shared across the app - `zero` has no way to cancel a single in-flight turn, so cancellation is "kill the process," and that shouldn't take other open conversations down with it.
- **We do not embed the zero binary** in the zero-desktop package to preserve zero's independent lifecycle.
- **We do not modify zero**; we only use its public interfaces.
- **Single workspace in alpha**: the alpha starts with one workspace. Multi-workspace support will be added later.

## 7. References

- [Agent Client Protocol](https://agentclientprotocol.com)
- [Zero Update Flow](https://github.com/Gitlawb/zero/blob/main/docs/UPDATE.md)
- [`update-model.md`](./update-model.md)
- [`decisions/001-connection-via-stream-json.md`](./decisions/001-connection-via-stream-json.md) (superseded)
- [`decisions/003-migrate-to-acp.md`](./decisions/003-migrate-to-acp.md)
