# Architecture Documentation

This section collects high-level design decisions and architecture for **zero-desktop**.

## Index

- [`connection.md`](./connection.md) — how the GUI connects to the zero agent.
- [`update-model.md`](./update-model.md) — update model separated from the zero CLI.
- [`decisions/`](./decisions/) — Architecture Decision Records (ADRs).

## Registered Decisions

1. [Connection via Stream-JSON instead of MCP or HTTP](./decisions/001-connection-via-stream-json.md)
2. [Linux Distribution via AppImage + Install Script](./decisions/002-linux-appimage-plus-install-script.md)

## Distribution

- [Linux Installation Guide](../distribution/linux-installation.md)

## Convention

Every significant new feature or architectural decision must be documented here in `.md` files. This rule is reflected in [`AGENTS.md`](../../../AGENTS.md).
