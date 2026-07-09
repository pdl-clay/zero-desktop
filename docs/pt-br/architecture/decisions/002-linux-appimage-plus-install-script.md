# 002 — Distribuição no Linux via AppImage + Script de Instalação

## Status

Aceito

## Contexto

Durante a fase alpha, o zero-desktop tem o Linux como foco inicial. Precisamos de um método de distribuição que seja:

- Simples para o usuário instalar.
- Universal entre as distribuições Linux.
- Fácil para o time construir e manter.
- Capaz de fornecer integração com o menu do sistema sem depender de gerenciadores de pacotes.

## Decisão

Usar **AppImage** como o único formato de distribuição alpha para Linux, combinado com um **script de instalação** que cuida da integração com o sistema.

O script de instalação:

1. Detecta a arquitetura (`x86_64`, `aarch64`).
2. Baixa o AppImage mais recente do GitHub Releases.
3. Instala em `~/.local/apps/zero-desktop/`.
4. Cria um symlink em `~/.local/bin/`.
5. Cria uma entrada `.desktop` em `~/.local/share/applications/`.
6. Atualiza o banco de dados de aplicativos.

Outros formatos como `.deb`, `.rpm`, Flatpak e Snap ficam para releases futuras.

## Consequências

- O usuário pode instalar com um único comando.
- Não requer privilégios de root.
- AppImage funciona na maioria das distribuições Linux.
- O script de instalação compensa a falta de integração automática com o sistema do AppImage.
- O pipeline de release só precisa produzir artefatos AppImage, reduzindo a complexidade do CI.
- Os portes para Windows e macOS serão planejados depois que a alpha no Linux estabilizar.
