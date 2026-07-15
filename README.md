# Zero Desktop

<p align="center">
  <img src="src-tauri/icons/icon.png" alt="Zero Desktop" width="120" />
</p>

<p align="center">
  <b>A native desktop GUI for the <a href="https://github.com/Gitlawb/zero">zero</a> coding agent.</b><br/>
  Built with <a href="https://tauri.app/">Tauri</a> + <a href="https://quasar.dev/">Quasar</a>.
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#install">Install</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#documentation">Docs</a>
</p>

> **Status:** Alpha — actively evolving. Core chat, multi-session architecture, MCP panel, and workspace management are already implemented.

---

## What is Zero Desktop?

Zero Desktop is a native desktop interface that wraps the `zero` coding agent and turns it into a visual, multi-project productivity tool. Instead of juggling terminal windows, you get a project-aware, multi-pane chat environment where every workspace can run independent sessions side by side.

It does **not** bundle or modify the `zero` binary — it uses zero's public [Agent Client Protocol (ACP)](https://agentclientprotocol.com) over stdio. This keeps `zero-desktop` updates separate from your `zero` CLI updates.

---

## Features

### 🗂️ Workspace-Centric Design

- Add project folders through the native OS folder picker.
- Workspaces are persisted across restarts.
- Each workspace has its own avatar, session list, and active processes.
- Switching workspaces connects/disconnects the agent automatically.

### 💬 Multi-Session Parallel Chat

- Open up to **4 panels per workspace** simultaneously.
- Each panel is a live `zero acp` process that can think, write, and run tools independently.
- Resize panels freely with draggable splitters.
- Closing the last panel automatically opens a fresh one so the workspace never feels empty.

### 🧠 Rich Chat Interface

- Streaming assistant replies with real-time reasoning blocks.
- Tool-call cards with live states: running, completed, or error.
- Inline diff viewer for `edit_file` / `write_file` changes.
- Error bubbles and permission decisions rendered in context.
- Collapsible model thinking panels.

### ✅ Live Plan Checklist

- The agent's `update_plan` steps are shown as an inline checklist above the input.
- Pending, in-progress, completed, and failed states are color-coded and animated.
- The checklist auto-hides once every task is done.

### 📎 File Attachments

- Attach images or text/code files to any message.
- Images are sent as vision blocks; text files are wrapped in `<attached file>` blocks.
- File previews appear before sending.
- Supported: images, markdown, JSON, YAML, Python, Go, Rust, JavaScript, TypeScript, and many more.

### 🛠️ MCP Panel

- Right-side drawer shows every MCP backend configured in `zero`.
- Live health checks with on-disk status cache for instant rendering.
- Tool counts per backend and aggregated tool list.
- Edited-files strip with expandable inline diff previews.

### ⚡ Permission & Safety Controls

- Real permission requests forwarded from the agent with the exact options it offers.
- Switch between **Ask** and **Auto-allow** modes.
- Permission decisions are persisted and replayed from history.

### 🎨 Model Switching

- Switch the active provider/model directly from the chat input bar.
- Lists real models from the provider's API.
- Model snapshots are recorded per session so history always shows what answered.

### 🌙 Native Desktop Experience

- Dark and light mode support.
- Compact, animated workspace avatars.
- Reactive session indicators (idle, thinking, writing, using tool).
- Responsive narrow/mobile layout with collapsible drawers.

---

## Architecture Highlights

- **Rust backend** spawns and manages one `zero acp` process per active panel.
- **Hand-rolled JSON-RPC peer** on top of `tokio` + `serde_json` for full ACP duplex communication.
- **Local rich history** stored at `~/.local/share/zero-desktop/session-history/` so sessions replay faithfully.
- **Per-session Pinia stores** keep multiple live conversations reactive and independent.

---

## Install

On Linux, run:

```bash
curl -fsSL https://raw.githubusercontent.com/pdl-clay/zero-desktop/main/scripts/install.sh | bash
```

For details, see the [Linux Installation Guide](./docs/en/distribution/linux-installation.md).

---

## Quick Start (development)

```bash
pnpm install
pnpm dev
```

## Build

```bash
pnpm build
```

---

## Documentation

- [Architecture (EN)](./docs/en/architecture/index.md)
- [Arquitetura (PT-BR)](./docs/pt-br/architecture/index.md)
- [Linux Installation (EN)](./docs/en/distribution/linux-installation.md)
- [Instalação no Linux (PT-BR)](./docs/pt-br/distribution/linux-installation.md)

---

## Project Rules

Every new feature, significant change, or architectural decision must be documented in `.md` files under `docs/` before or alongside implementation. See [`AGENTS.md`](./AGENTS.md).
