# Documentação de Arquitetura

Esta seção reúne as decisões e o design de alto nível do **zero-desktop**.

## Índice

- [`connection.md`](./connection.md) — como o GUI se conecta ao agente zero via ACP.
- [`update-model.md`](./update-model.md) — modelo de atualização separado do zero CLI.
- [`decisions/`](./decisions/) — registros de decisões arquiteturais (ADRs).

## Decisões Registradas

1. [Conexão via Stream-JSON em vez de MCP ou HTTP](./decisions/001-connection-via-stream-json.md) — **substituído pelo ADR 003**
2. [Distribuição no Linux via AppImage + Script de Instalação](./decisions/002-linux-appimage-plus-install-script.md)
3. [Migrar o Backbone de Conexão de `zero exec` para `zero acp`](./decisions/003-migrate-to-acp.md)
4. [Sessões de Chat Paralelas (Tiling)](./decisions/004-multi-session-parallel.md)
5. [Tauri Updater para Auto-Atualização do AppImage](./decisions/005-tauri-updater-for-appimage-self-update.md)

## Distribuição

- [Guia de Instalação no Linux](../distribution/linux-installation.md)
- [Publicando releases do zero-desktop](../distribution/releasing.md)

## Funcionalidades

- [zero-bridge: Conexão com o zero CLI](../features/zero-bridge.md)
- [Sistema de Sessões](../features/session-system.md)
- [Sistema de Workspaces](../features/workspace-system.md)
- [Interface de Chat](../features/chat-interface.md)
- [Painel MCP](../features/mcp-panel.md)
- [Anexos de Arquivo](../features/file-attachments.md)
- [Troca de Modelo](../features/model-switching.md)
- [Sistema de Plano](../features/plan-system.md)

## Convenção

Toda nova funcionalidade ou decisão arquitetural significativa deve ser documentada aqui em arquivos `.md`. Esta regra está refletida no [`AGENTS.md`](../../../AGENTS.md).
