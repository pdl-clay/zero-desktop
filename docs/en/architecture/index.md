# Architecture Documentation

This section collects high-level design decisions and architecture for **zero-desktop**.

## Index

- [`connection.md`](./connection.md) — how the GUI connects to the zero agent via ACP.
- [`update-model.md`](./update-model.md) — update model separated from the zero CLI.
- [`decisions/`](./decisions/) — Architecture Decision Records (ADRs).

## Registered Decisions

1. [Connection via Stream-JSON instead of MCP or HTTP](./decisions/001-connection-via-stream-json.md) — **superseded by ADR 003**
2. [Linux Distribution via AppImage + Install Script](./decisions/002-linux-appimage-plus-install-script.md)
3. [Migrate the Connection Backbone from `zero exec` to `zero acp`](./decisions/003-migrate-to-acp.md)
4. [Multi-Session Parallel Chat (Tiling)](./decisions/004-multi-session-parallel.md)
5. [Tauri Updater for AppImage Self-Update](./decisions/005-tauri-updater-for-appimage-self-update.md)

## Distribution

- [Linux Installation Guide](../distribution/linux-installation.md)
- [Releasing zero-desktop](../distribution/releasing.md)

## Features

- [zero-bridge: Connection to the zero CLI](../features/zero-bridge.md)
- [Session System](../features/session-system.md)
- [Workspace System](../features/workspace-system.md)
- [Chat Interface](../features/chat-interface.md)
- [MCP Panel](../features/mcp-panel.md)
- [File Attachments](../features/file-attachments.md)
- [Model Switching](../features/model-switching.md)
- [Plan System](../features/plan-system.md)

## Convention

Every significant new feature or architectural decision must be documented here in `.md` files. This rule is reflected in [`AGENTS.md`](../../../AGENTS.md).
