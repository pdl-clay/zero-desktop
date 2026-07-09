# Instalação no Linux

Este documento descreve como os usuários instalam o **zero-desktop** no Linux durante a fase alpha.

## Método recomendado: script de instalação

A forma mais rápida de instalar o zero-desktop em qualquer distribuição Linux é pelo script oficial:

```bash
curl -fsSL https://raw.githubusercontent.com/Gitlawb/zero-desktop/main/scripts/install.sh | bash
```

### O que o script faz

1. Detecta a arquitetura do sistema (`x86_64` ou `aarch64`).
2. Busca o último release no GitHub.
3. Baixa o AppImage correspondente.
4. Instala o AppImage em `~/.local/apps/zero-desktop/zero-desktop.AppImage`.
5. Torna o arquivo executável.
6. Cria um symlink em `~/.local/bin/zero-desktop`.
7. Cria uma entrada `.desktop` em `~/.local/share/applications/zero-desktop.desktop`.
8. Atualiza o banco de dados de aplicativos para que o app apareça no menu do sistema.

### Requisitos

- `curl` ou `wget`
- `~/.local/bin` no seu `PATH`
- Um ambiente desktop que leia `~/.local/share/applications` (GNOME, KDE, XFCE, etc.)

## Instalação manual

Se preferir não executar o script, você pode baixar o AppImage manualmente na página de [GitHub Releases](https://github.com/Gitlawb/zero-desktop/releases):

```bash
chmod +x zero-desktop-vX.Y.Z-linux-x86_64.AppImage
./zero-desktop-vX.Y.Z-linux-x86_64.AppImage
```

Para integrá-lo ao menu do sistema, copie o AppImage para `~/.local/apps/zero-desktop/` e crie um arquivo `.desktop` manualmente.

## Primeira execução

Na primeira inicialização, o zero-desktop verifica se o CLI do [zero](https://github.com/Gitlawb/zero) está instalado no `PATH`. Se não for encontrado, um assistente de instalação oferece duas opções:

1. **Instalação global** — executa o script oficial do zero, colocando `zero` em `~/.local/bin`.
2. **Instalação isolada** — baixa o `zero` para o cache do zero-desktop (`~/.local/share/zero-desktop/bin/zero`) sem alterar diretórios do sistema.

## Atualização

Execute novamente o script de instalação para atualizar para a versão mais recente:

```bash
curl -fsSL https://raw.githubusercontent.com/Gitlawb/zero-desktop/main/scripts/install.sh | bash
```

O script substitui o AppImage existente preservando seus dados locais.

## Desinstalação

Execute:

```bash
zero-desktop --uninstall
```

Ou remova manualmente:

```bash
rm -rf ~/.local/apps/zero-desktop
rm ~/.local/bin/zero-desktop
rm ~/.local/share/applications/zero-desktop.desktop
rm -rf ~/.local/share/zero-desktop
```

## Arquiteturas suportadas

| Arquitetura | Nome do pacote |
|---|---|
| x86_64 | `zero-desktop-vX.Y.Z-linux-x86_64.AppImage` |
| aarch64 | `zero-desktop-vX.Y.Z-linux-aarch64.AppImage` |

## Observações

- AppImage é o único formato de distribuição na alpha.
- `.deb`, `.rpm`, Flatpak, Snap e outros formatos estão planejados para releases futuras.
- O script de instalação não requer privilégios de root.
