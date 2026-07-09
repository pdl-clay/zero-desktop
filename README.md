# Zero Desktop

A desktop GUI for the [zero](https://github.com/Gitlawb/zero) coding agent, built with [Tauri](https://tauri.app/) and [Quasar](https://quasar.dev/).

> **Status:** Alpha — basic chat and connection architecture are being implemented.

## Features

- Connects to the `zero` CLI via its public stream-json protocol.
- Does not bundle or modify the `zero` binary.
- Keeps zero-desktop updates separate from zero CLI updates.

## Install

On Linux, run:

```bash
curl -fsSL https://raw.githubusercontent.com/Gitlawb/zero-desktop/main/scripts/install.sh | bash
```

For details, see the [Linux Installation Guide](./docs/en/distribution/linux-installation.md).

## Documentation

- [Architecture (EN)](./docs/en/architecture/index.md)
- [Arquitetura (PT-BR)](./docs/pt-br/architecture/index.md)
- [Linux Installation (EN)](./docs/en/distribution/linux-installation.md)
- [Instalação no Linux (PT-BR)](./docs/pt-br/distribution/linux-installation.md)

## Quick Start (development)

```bash
pnpm install
pnpm dev
```

## Build

```bash
pnpm build
```

## Project Rules

Every new feature, significant change, or architectural decision must be documented in `.md` files under `docs/` before or alongside implementation. See [`AGENTS.md`](./AGENTS.md).
