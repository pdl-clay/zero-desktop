# Modelo de Atualização

Este documento define como o **zero-desktop** gerencia suas próprias atualizações sem interferir no mecanismo oficial do zero (`zero update`).

## 1. Princípio Fundamental

> O ciclo de vida do **zero-desktop** é separado do ciclo de vida do **zero CLI**.

| Componente     | Quem atualiza             | Mecanismo                                  |
| -------------- | ------------------------- | ------------------------------------------ |
| `zero-desktop` | zero-desktop              | Tauri updater                              |
| `zero` CLI     | usuário ou script oficial | `zero update --check`, npm, install script |

## 2. Atualização do zero-desktop

O zero-desktop é distribuído como **AppImage** e se atualiza sozinho, dentro do próprio app, usando o **updater oficial do Tauri** (`tauri-plugin-updater` + `tauri-plugin-process`). A primeira instalação continua passando pelo script de instalação:

```bash
curl -fsSL https://raw.githubusercontent.com/pdl-clay/zero-desktop/main/scripts/install.sh | bash
```

Veja [`docs/pt-br/distribution/linux-installation.md`](../distribution/linux-installation.md) para mais detalhes. Racional completo das escolhas abaixo: [ADR 005](./decisions/005-tauri-updater-for-appimage-self-update.md).

### 2.1 Fluxo

- **Endpoint**: `https://github.com/pdl-clay/zero-desktop/releases/latest/download/latest.json` — um arquivo estático publicado junto de cada GitHub Release (veja `.github/workflows/release.yml`). O alias "latest" do GitHub sempre resolve para o release mais recente, então nenhuma variável de template é necessária.
- **Verificação**: a cada inicialização, e sob demanda em Configurações → Geral → Sobre → "Verificar atualizações".
- **Download + instalação**: silenciosos, em background, assim que uma versão mais nova é encontrada — nenhuma confirmação do usuário é necessária para baixar a atualização.
- **Reinício**: nunca automático. Depois que a atualização é instalada, uma notificação dispensável oferece um botão "Reiniciar agora"; o app continua rodando a versão antiga até o usuário clicar explicitamente.

### 2.2 Ativação restrita ao AppImage

A auto-atualização só faz sentido quando o app está de fato rodando como o AppImage empacotado — a etapa de instalação sobrescreve o arquivo indicado pela variável de ambiente `$APPIMAGE`, que só existe nesse caso. Isso é controlado em dois lugares:

- **Rust**: o `tauri-plugin-updater` só é registrado no `setup()` de `src-tauri/src/lib.rs` quando `$APPIMAGE` está presente.
- **Frontend**: um comando `is_appimage` permite que a UI esconda completamente "Verificar atualizações" fora de uma execução real como AppImage (ex.: `tauri dev`).

### 2.3 Assinatura

As atualizações são assinadas com um par de chaves Ed25519/minisign gerado via `tauri signer generate`. A chave pública é versionada em `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`); a chave privada e sua senha vivem apenas nos secrets do GitHub Actions (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) e no gerenciador de senhas do mantenedor — nunca no repositório.

## 3. Detecção e Instalação do zero CLI

### 3.1 Detecção

Na inicialização, o `ZeroLocator` procura o binário `zero`:

1. Em cada diretório do `PATH`.
2. No cache isolado do zero-desktop.
3. Via `zero --version` para confirmar que é executável.

### 3.2 Quando Não Encontrado

A UI apresenta três opções:

1. **Instruções manuais**: mostra o comando oficial do zero para instalação global.
2. **Instalação global assistida**: executa o script oficial do zero (`scripts/install.sh` ou `scripts/install.ps1`), que coloca `zero` em `~/.local/bin` ou `%LOCALAPPDATA%\zero\bin`.
3. **Instalação isolada**: baixa o binário do release do zero diretamente para o cache do zero-desktop, sem alterar PATH.

### 3.3 Política de Não Conflito

- O zero-desktop **nunca** substitui um `zero` encontrado no PATH.
- O zero-desktop **nunca** executa `zero update` automaticamente.
- O zero-desktop pode, a pedido do usuário, executar `zero update --check` apenas para **informar** se há atualização disponível.

## 4. Cache Isolado

Local padrão do cache isolado:

| Sistema | Caminho                                               |
| ------- | ----------------------------------------------------- |
| Linux   | `~/.local/share/zero-desktop/bin/zero`                |
| macOS   | `~/Library/Application Support/zero-desktop/bin/zero` |
| Windows | `%LOCALAPPDATA%\zero-desktop\bin\zero.exe`            |

O cache isolado é usado apenas quando:

- Não existe `zero` no PATH.
- O usuário escolheu explicitamente a instalação isolada.

## 5. Verificação de Compatibilidade

Futuramente, o zero-desktop pode declarar uma versão mínima do zero CLI. Na inicialização:

- Se a versão detectada for inferior à mínima, alerta o usuário.
- Sugere atualizar via mecanismo oficial do zero.

## 6. Segurança

- Downloads sempre via HTTPS.
- Verificação de checksum SHA256 quando disponível nos releases do zero.
- Nunca executa scripts não solicitados.

## 7. Referências

- [Tauri Updater Plugin](https://tauri.app/plugin/updater/)
- [Zero Update Flow](https://github.com/Gitlawb/zero/blob/main/docs/UPDATE.md)
- [Zero Install Scripts](https://github.com/Gitlawb/zero/blob/main/docs/INSTALL.md)
