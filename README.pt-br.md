# Zero Desktop

Uma interface desktop para o agente de código [zero](https://github.com/Gitlawb/zero), construída com [Tauri](https://tauri.app/) e [Quasar](https://quasar.dev/).

> **Status:** Alfa — chat básico e arquitetura de conexão estão sendo implementados.

## Funcionalidades

- Conecta-se ao CLI `zero` via seu protocolo público stream-json.
- Não embute nem modifica o binário do `zero`.
- Mantém as atualizações do zero-desktop separadas das atualizações do zero CLI.

## Documentação

- [Architecture (EN)](./docs/en/architecture/index.md)
- [Arquitetura (PT-BR)](./docs/pt-br/architecture/index.md)

## Início Rápido

```bash
pnpm install
pnpm dev
```

## Build

```bash
pnpm build
```

## Regras do Projeto

Toda nova funcionalidade, mudança significativa ou decisão arquitetural deve ser documentada em arquivos `.md` dentro de `docs/` antes ou junto com a implementação. Veja [`AGENTS.md`](./AGENTS.md).
