# Publicando releases do zero-desktop

Este documento descreve como os mantenedores cortam uma nova release do zero-desktop, agora que a publicação é automatizada de ponta a ponta via GitHub Actions e consumida pelo auto-updater dentro do app. Veja o [ADR 005](../architecture/decisions/005-tauri-updater-for-appimage-self-update.md) e o [`update-model.md`](../architecture/update-model.md) para o racional completo do design — este documento é a versão prática, passo a passo.

## O que dispara uma release

Só o **push de uma tag** que bata com `v*` (ou uma execução manual via `workflow_dispatch` na aba Actions) dispara o `.github/workflows/release.yml`. Commits e pushes normais em `main` não buildam nem publicam nada sozinhos — uma release é um ato deliberado e separado.

## Passo a passo

1. **Bump a versão** em `package.json` e em `src-tauri/tauri.conf.json`. As duas precisam ter exatamente o mesmo valor — o workflow valida isso logo no início e falha cedo se estiverem diferentes.
2. Rode `npm install` (sem flags especiais) pra manter o campo de versão do próprio `package-lock.json` sincronizado também.
3. Commit e `git push origin main` normalmente. Isso sozinho não builda nem publica nada.
4. Crie uma tag anotada com o prefixo `v` batendo com a nova versão, e dê push dela:
   ```bash
   git tag -a v0.1.0-alpha.3 -m "..."
   git push origin v0.1.0-alpha.3
   ```
5. O push da tag é o que dispara o workflow. Acompanhe a execução em `https://github.com/pdl-clay/zero-desktop/actions`.

## O que o workflow faz automaticamente

O job roda dentro de um container `quay.io/fedora/fedora:43` — o mesmo ambiente validado como seguro para buildar o AppImage localmente (veja `scripts/build-appimage-in-container.sh` e a regra de build do AGENTS.md). Ele:

1. Confere que a tag enviada bate com a versão declarada em `tauri.conf.json`.
2. Instala o Rust e atualiza o `npm` (veja a nota abaixo sobre o motivo).
3. Roda `npm ci` e busca o binário sidecar do CLI `zero`.
4. Builda o AppImage assinado via `scripts/build-appimage-in-container.sh`, usando os secrets do repositório `TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
5. Gera o `latest.json` (o manifesto do updater do Tauri) via `scripts/generate-update-manifest.sh`.
6. Publica o AppImage, seu `.sig`, um checksum `.sha256` e o `latest.json` como assets de um novo GitHub Release.

## O que acontece com as instalações já existentes

Depois de publicado, qualquer instância do zero-desktop rodando como um AppImage de verdade (não `tauri dev`) detecta a versão nova sozinha — seja na próxima inicialização, seja via "Verificar atualizações" em Configurações → Geral —, baixa e instala silenciosamente em background, e só reinicia quando o usuário clicar em "Reiniciar agora". Nada é forçado sem confirmação do usuário.

## Chaves de assinatura

As atualizações são assinadas com um par de chaves Ed25519/minisign. A metade pública é versionada em `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`); a metade privada e sua senha existem apenas nos dois secrets do GitHub Actions acima e no gerenciador de senhas do mantenedor — nunca no repositório. Perder a chave privada significa que nenhuma release futura poderá ser verificada pelos apps já instalados; a recuperação exige gerar um novo par de chaves, embutir a nova chave pública, e pedir que todo usuário reinstale uma vez via `scripts/install.sh`.

## Peculiaridade conhecida de CI: `npm ci` e peer dependencies opcionais

O `nodejs` instalado via `dnf` no Fedora 43 pode vir com uma versão de `npm` antiga o suficiente pra ter um bug conhecido: o `npm ci` passa a exigir que peer dependencies **opcionais** (por exemplo, o `esbuild` opcional do `vite@8`) estejam presentes no lockfile, mesmo quando nunca deveriam ser instaladas de fato. O workflow roda `npm install -g npm@latest` logo depois de instalar o `nodejs` via `dnf` justamente para evitar isso.

Se o erro `npm error Missing: esbuild@... from lock file` aparecer de novo numa execução de release, quase certamente é essa a causa, não um lockfile genuinamente quebrado — confirme rodando `npm ci` localmente primeiro: se funcionar local e só falhar no CI, é diferença de versão do `npm`, não um problema de lockfile.

## Escopo: apenas amd64/x86_64 (por enquanto)

O pipeline de release hoje só publica a plataforma `linux-x86_64` no `latest.json`. Nunca houve um build arm64 de fato publicado, e fazer cross-build/emulação de um AppImage sob QEMU dentro do mesmo container Fedora que existe especificamente para contornar um bug de glibc/toolchain já misterioso (veja a regra de build do AGENTS.md) foi julgado arriscado demais para entrar na primeira versão deste pipeline. Suporte a arm64 é um follow-up nomeado — um build em matriz mais uma entrada `linux-aarch64` no `latest.json` — não uma omissão silenciosa.
