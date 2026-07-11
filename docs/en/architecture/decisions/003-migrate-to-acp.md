# 003 — Migrate the Connection Backbone from `zero exec` to `zero acp`

## Status

Accepted. Supersedes [001 — Connecting to Zero via Stream-JSON](./001-connection-via-stream-json.md).

## Context

ADR 001 chose `zero exec --input-format stream-json --output-format stream-json` as the connection backbone. In practice, `zero exec` turned out to be a **one-shot batch command**: it reads stdin to EOF before acting on any of it (confirmed by holding stdin open and observing zero produce no stdout/network activity at all, no matter how long you wait), so zero-desktop had to write the message and close stdin immediately for a turn to run at all.

The direct consequence: there was no channel left to send anything back into a running turn. Permission prompts (`send_permission_decision`) were unimplementable - the button existed in the UI, but clicking it always failed, because the process that would need to receive the decision no longer had an open stdin. Tested exhaustively across autonomy levels (`low`/`medium`/`high`) and action types (file edits, shell commands, network access): `zero exec` never once asked interactively. It either auto-decided (emitting an informational `permission_decision` event) or auto-denied, failing the tool call with "Sandbox approval required."

## Options Considered

### Option A: Keep `zero exec`, work around the stdin limitation

Could not identify a workaround. The EOF-before-processing behavior is fundamental to how the command reads input, not a flag or timing issue - confirmed by testing with stdin deliberately held open for extended periods.

### Option B: `zero acp`

`zero acp` serves the [Agent Client Protocol](https://agentclientprotocol.com): JSON-RPC 2.0 over stdio, newline-delimited (not `Content-Length`-framed like LSP), built for editor integrations (Zed, Neovim, ...). Verified directly against the real CLI:

- The process stays alive for the whole conversation - no EOF requirement.
- `session/new` / `session/load` / `session/prompt` work as expected; `session/prompt` streams progress via `session/update` notifications and only resolves once the turn ends.
- **The agent can send requests to us mid-turn.** `session/request_permission` is a real JSON-RPC request with an `id`; replying with `{"outcome":{"outcome":"selected","optionId":...}}` genuinely unblocks the agent - proven end-to-end with both a throwaway Python harness and, more importantly, the actual Rust `AcpPeer` implementation against the real binary.
- `session/load` works for reattaching to a session by id (the ACP equivalent of `--resume`).
- No `session/cancel` method exists (`method not found`) - cancelling a turn means killing the process.
- zero's own on-disk session log (`events.jsonl`) is _sparser_ under ACP than it was under exec: only `message` entries are persisted, none of the tool-call/reasoning/permission activity exec-mode used to record.

### Option C: `zero daemon`

A background worker with `run`/`attach`/session routing, capable of serving multiple sessions and even remote bridging. Not pursued for this migration - ACP already solves the concrete problem (real permission delivery) with a simpler process model (one process per active session vs. a long-running daemon to manage), and is purpose-built for exactly this "external client drives an agent conversation" use case rather than being general session infrastructure.

## Decision

Use **Option B**: `zero acp` as the connection backbone, replacing `zero exec`.

Process model: **one `zero acp` process per active session**, not a single process shared across the app. Since there's no `session/cancel`, interrupting a turn means killing the process; a shared process would take every other open conversation down with it. A process per session keeps that blast radius contained, while still being a large improvement over exec's "one process per _message_."

To cover the session-history regression (Option B's sparser on-disk log), zero-desktop now writes its **own** rich per-session history log (`<app_data_dir>/zero-desktop/session-history/<sessionId>.jsonl`) alongside forwarding live events to the frontend, and reads from it in preference to zero's own `events.jsonl` when present. Sessions created before this migration (or outside zero-desktop) fall back to the old read path unchanged.

## Consequences

- `src-tauri/src/bridge.rs` was rewritten around a persistent per-session JSON-RPC connection instead of spawning `zero exec` per message. `src-tauri/src/acp.rs` is a new, minimal hand-rolled JSON-RPC 2.0 peer (not a dependency - the protocol is simple enough, and no available crate cleanly supports being both a request-sender and a request-receiver on the same connection, which ACP requires).
- Permission approval/denial now actually reaches the agent. The frontend renders whatever options ACP offers for a given request (e.g. "Allow", "Allow for session", "Reject") instead of a fixed Approve/Deny pair.
- The old `send_permission_decision` command, `PermissionRequest.vue` (already orphaned before this migration), and a client-side localStorage workaround for remembering permission decisions (needed only because decisions previously couldn't be delivered) were removed.
- Cancelling a turn kills that session's process; the next message respawns it and reattaches via `session/load`.
- Session history richness now depends on zero-desktop's own log for sessions created after this migration; very old sessions (or ones created directly via the `zero` CLI, outside zero-desktop) keep working through the previous, sparser read path.
