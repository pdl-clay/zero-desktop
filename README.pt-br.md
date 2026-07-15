# Zero Desktop

<p align="center">
  <img src="src-tauri/icons/icon.png" alt="Zero Desktop" width="120" />
</p>

<p align="center">
  <b>Uma interface desktop nativa para o agente de código <a href="https://github.com/Gitlawb/zero">zero</a>.</b><br/>
  Construída com <a href="https://tauri.app/">Tauri</a> + <a href="https://quasar.dev/">Quasar</a>.
</p>

<p align="center">
  <a href="#funcionalidades">Funcionalidades</a> •
  <a href="#instalação">Instalação</a> •
  <a href="#início-rápido">Início Rápido</a> •
  <a href="#documentação">Documentação</a>
</p>

> **Status:** Alfa — em evolução ativa. Chat principal, arquitetura multi-sessão, painel MCP e gerenciamento de workspaces já estão implementados.

---

## O que é o Zero Desktop?

O Zero Desktop é uma interface nativa que envolve o agente de código `zero` e o transforma em uma ferramenta visual de produtividade multi-projeto. Em vez de alternar entre janelas de terminal, você obtém um ambiente de chat organizado por projeto, com vários painéis por workspace rodando sessões independentes lado a lado.

Ele **não** embute nem modifica o binário do `zero` — usa o protocolo público [Agent Client Protocol (ACP)](https://agentclientprotocol.com) do zero via stdio. Assim, as atualizações do `zero-desktop` permanecem separadas das atualizações do `zero` CLI.

---

## Funcionalidades

### 🗂️ Design Centrado em Workspaces

- Adicione pastas de projeto pelo seletor nativo do sistema operacional.
- Workspaces são persistidos entre reinicializações.
- Cada workspace tem seu próprio avatar, lista de sessões e processos ativos.
- Trocar de workspace conecta/desconecta o agente automaticamente.

### 💬 Chat Multi-Sessão Paralelo

- Abra até **4 painéis por workspace** simultaneamente.
- Cada painel é um processo `zero acp` vivo que pode pensar, escrever e executar ferramentas independentemente.
- Redimensione painéis livremente com divisores arrastáveis.
- Fechar o último painel abre automaticamente um novo, para que o workspace nunca fique vazio.

### 🧠 Interface de Chat Rica

- Respostas do assistente em streaming com blocos de raciocínio em tempo real.
- Cards de chamadas de ferramenta com estados ao vivo: em execução, concluído ou erro.
- Visualizador de diff inline para alterações de `edit_file` / `write_file`.
- Bolhas de erro e decisões de permissão renderizadas no contexto.
- Painéis de pensamento do modelo recolhíveis.

### ✅ Checklist de Plano em Tempo Real

- Os passos do `update_plan` do agente aparecem como uma checklist inline acima do campo de mensagem.
- Estados pendentes, em execução, concluídos e falhos são coloridos e animados.
- A checklist se oculta automaticamente quando todas as tarefas terminam.

### 📎 Anexos de Arquivos

- Anexe imagens ou arquivos de texto/código a qualquer mensagem.
- Imagens são enviadas como blocos de visão; arquivos de texto são envolvidos em blocos `<attached file>`.
- Pré-visualizações dos arquivos aparecem antes do envio.
- Suporta: imagens, markdown, JSON, YAML, Python, Go, Rust, JavaScript, TypeScript e muitos outros.

### 🛠️ Painel MCP

- Drawer lateral direito mostra todos os backends MCP configurados no `zero`.
- Health checks ao vivo com cache em disco para renderização instantânea.
- Contagem de ferramentas por backend e lista agregada de ferramentas.
- Faixa de arquivos editados com visualização inline de diffs expansível.

### ⚡ Controles de Permissão e Segurança

- Requisições de permissão reais repassadas do agente, com as opções exatas que ele oferece.
- Alterne entre os modos **Perguntar** e **Permitir automaticamente**.
- Decisões de permissão são persistidas e reproduzidas a partir do histórico.

### 🎨 Troca de Modelo

- Troque o provedor/modelo ativo diretamente pela barra de entrada do chat.
- Lista modelos reais da API do provedor.
- Snapshots do modelo são gravados por sessão, então o histórico sempre mostra quem respondeu.

### 🌙 Experiência Desktop Nativa

- Suporte a modo claro e escuro.
- Avatares compactos e animados para cada workspace.
- Indicadores reativos de sessão (parado, pensando, escrevendo, usando ferramenta).
- Layout responsivo para telas estreitas/mobile com drawers colapsáveis.

---

## Destaques da Arquitetura

- **Backend em Rust** inicia e gerencia um processo `zero acp` por painel ativo.
- **Peer JSON-RPC feito à mão** sobre `tokio` + `serde_json` para comunicação ACP full-duplex.
- **Histórico rico local** armazenado em `~/.local/share/zero-desktop/session-history/` para que sessões sejam reproduzidas fielmente.
- **Stores Pinia por sessão** mantêm várias conversas ativas reativas e independentes.

---

## Instalação

No Linux, execute:

```bash
curl -fsSL https://raw.githubusercontent.com/pdl-clay/zero-desktop/main/scripts/install.sh | bash
```

Para mais detalhes, veja o [Guia de Instalação no Linux](./docs/pt-br/distribution/linux-installation.md).

---

## Início Rápido (desenvolvimento)

```bash
pnpm install
pnpm dev
```

## Build

```bash
pnpm build
```

---

## Documentação

- [Architecture (EN)](./docs/en/architecture/index.md)
- [Arquitetura (PT-BR)](./docs/pt-br/architecture/index.md)
- [Linux Installation (EN)](./docs/en/distribution/linux-installation.md)
- [Instalação no Linux (PT-BR)](./docs/pt-br/distribution/linux-installation.md)

---

## Regras do Projeto

Toda nova funcionalidade, mudança significativa ou decisão arquitetural deve ser documentada em arquivos `.md` dentro de `docs/` antes ou junto com a implementação. Veja [`AGENTS.md`](./AGENTS.md).
