# Documentação de Arquitetura

Esta seção reúne as decisões e o design de alto nível do **zero-desktop**.

## Índice

- [`connection.md`](./connection.md) — como o GUI se conecta ao agente zero.
- [`update-model.md`](./update-model.md) — modelo de atualização separado do zero CLI.
- [`decisions/`](./decisions/) — registros de decisões arquiteturais (ADRs).

## Decisões Registradas

1. [Conexão via Stream-JSON em vez de MCP ou HTTP](./decisions/001-connection-via-stream-json.md)
2. [Distribuição no Linux via AppImage + Script de Instalação](./decisions/002-linux-appimage-plus-install-script.md)

## Distribuição

- [Guia de Instalação no Linux](../distribution/linux-installation.md)

## Convenção

Toda nova funcionalidade ou decisão arquitetural significativa deve ser documentada aqui em arquivos `.md`. Esta regra está refletida no [`AGENTS.md`](../../../AGENTS.md).
