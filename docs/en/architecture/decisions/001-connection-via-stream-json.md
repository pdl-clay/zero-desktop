# 001 — Connecting to Zero via Stream-JSON

## Status

Accepted

## Context

zero-desktop needs to communicate with the [zero](https://github.com/Gitlawb/zero) coding agent. zero provides three programmatic interfaces:

1. **`zero exec --input-format stream-json --output-format stream-json`** — bidirectional JSONL protocol.
2. **`zero serve --mcp`** — MCP server over stdio.
3. **Interactive TUI (`zero`)** — not programmable.

We needed to choose the primary interface for the GUI without modifying zero or conflicting with its updates.

## Options Considered

### Option A: `zero exec` with stream-json

Use the official JSONL protocol over stdin/stdout.

**Pros:**

- Public, documented, and stable interface.
- Supports text streaming, tool calls, permission requests, reasoning, and token usage.
- Supports sessions (`--resume`, `--fork`).
- Does not require special permissions or `--allow-unsafe-tools` for basic chat.
- Requires no changes to zero.

**Cons:**

- Each run is a new process; continuous conversations require open stdin or `--resume`.
- Requires JSONL parsing and subprocess management in Rust.

### Option B: `zero serve --mcp`

Use zero as an MCP stdio server and the GUI as the MCP host.

**Pros:**

- Emerging standard for AI tools.
- Exposes zero's tools in a structured way.

**Cons:**

- MCP stdio is tool-call oriented, not continuous chat.
- Does not stream the LLM response.
- Not the natural interface for a conversational experience.

### Option C: Add HTTP/WebSocket Server to zero

Modify zero to expose an HTTP API.

**Pros:**

- Would be the most GUI-friendly interface.

**Cons:**

- Requires forking and maintaining zero.
- Conflicts with official updates.
- Increases attack surface.

## Decision

Use **Option A**: `zero exec` with stream-json as the communication backbone.

MCP (`zero serve --mcp`) may be reconsidered in the future as a way to expose zero's tools for other uses, but not as the chat backbone.

## Consequences

- The Rust backend needs a `ProcessManager` to spawn and manage the child process.
- The frontend receives events via Tauri events instead of WebSocket.
- Session recovery is reliable because zero persists sessions to disk.
- There is no dependency on modifying zero.
