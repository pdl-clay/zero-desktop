# 005 — Tauri Updater para Auto-Atualização do AppImage

## Status

Aceito

## Contexto

Antes desta decisão, atualizar o zero-desktop significava rodar manualmente o script de instalação de novo (`curl ... | bash`). O ADR 002 já previa adotar o updater oficial do Tauri "no futuro"; este ADR torna isso concreto, e também cobre o pipeline de release necessário para alimentá-lo, já que nenhum existia (`.github/workflows/` estava vazio).

Duas restrições já existentes moldaram cada escolha abaixo:

- O AppImage nunca pode ser buildado direto no host — veja o cabeçalho de `scripts/build-appimage-in-container.sh` e a regra 7 do AGENTS.md para o bug de runtime documentado (um AppImage buildado num toolchain/glibc de host bleeding-edge entrava em loop infinito de re-exec em runtime, sem nenhum erro no momento do build). Ele deve sempre ser buildado dentro de um ambiente Fedora 43.
- O zero-desktop e o `zero` CLI têm ciclos de vida de atualização independentes (seção 3.3 de `update-model.md`). Este updater nunca deve tocar no binário sidecar `zero` nem rodar `zero update` automaticamente.

## Decisão

- **Plugins**: adotar `tauri-plugin-updater` + `tauri-plugin-process` (plugins oficiais do Tauri), não um mecanismo de atualização caseiro.
- **Manifesto**: um `latest.json` estático publicado como asset de GitHub Release, referenciado via alias "latest" do GitHub (`.../releases/latest/download/latest.json`) — nenhum endpoint com variáveis de template é necessário, já que o próprio `latest.json` carrega um mapa por plataforma.
- **Modo de artefato**: `bundle.createUpdaterArtifacts: true` (o modo v2 atual, não-legado). Para o target `appimage`, isso reaproveita o próprio AppImage e adiciona um `.AppImage.sig` ao lado — **não** gera `.tar.gz`, então a convenção de nome de asset que `install.sh` já usa (`zero-desktop_<versão>_<amd64|arm64>.AppImage`) permanece intacta.
- **Assinatura**: um par de chaves Ed25519/minisign gerado via `tauri signer generate`. A chave pública é versionada em `src-tauri/tauri.conf.json`. A chave privada e sua senha existem apenas como secrets do GitHub Actions (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) e no gerenciador de senhas do mantenedor — nunca no repositório.
- **Ativação restrita ao AppImage**: a auto-atualização só faz sentido numa execução real como AppImage (a etapa de instalação sobrescreve o arquivo indicado por `$APPIMAGE`). Controlado tanto no lado Rust (registro condicional do plugin no `setup()` de `lib.rs`) quanto no frontend (um comando `is_appimage` esconde a UI de atualização em `tauri dev`).
- **UX**: checagem silenciosa em background na inicialização + checagem manual em Configurações → Geral → Sobre. Download+instalação silenciosos em background assim que uma versão mais nova é encontrada. Uma notificação dispensável oferece "Reiniciar agora" — o app nunca reinicia sem confirmação explícita do usuário. Isso corresponde à UX já esboçada em `update-model.md` antes deste ADR.
- **Pipeline de release**: um novo workflow do GitHub Actions (`.github/workflows/release.yml`), disparado em tags `v*` (+ disparo manual), cujo job roda com `container: quay.io/fedora/fedora:43` — reproduzindo diretamente o único ambiente de build validado como seguro, sem precisar de distrobox (o próprio container do job da GH Actions já É esse ambiente). Os passos reais de build foram extraídos de `scripts/build-appimage.sh` para um novo `scripts/build-appimage-in-container.sh`, compartilhado tanto pelo wrapper distrobox local quanto pelo CI, para que a receita de build tenha uma única fonte de verdade em vez de duas cópias divergindo com o tempo. Um novo `scripts/generate-update-manifest.sh` monta o `latest.json` a partir do `.sig` gerado pelo build — escolhido em vez de encapsular com `tauri-apps/tauri-action`, já que o build deste repositório tem particularidades demais (container Fedora fixo, busca do binário sidecar, workarounds de ambiente `NO_STRIP`/`APPIMAGELAUNCHER_DISABLE`) para valer a pena forçá-las pelos hooks dessa action. A publicação usa `softprops/action-gh-release@v2` (não exige `dnf install` extra, ao contrário da CLI `gh` numa imagem Fedora crua).
- **Escopo**: apenas amd64/x86_64 neste primeiro pipeline. Nunca houve evidência de um build arm64 de fato publicado, apesar de `install.sh` já suportar essa convenção de nome. Cross-buildar/emular um AppImage sob QEMU dentro do mesmo container Fedora que existe especificamente para contornar um bug de glibc/toolchain já misterioso é um péssimo lugar para introduzir um segundo bug, ainda mais difícil de diagnosticar. arm64 fica como um follow-up explícito (build em matriz + uma entrada `linux-aarch64` no `latest.json`), não uma omissão silenciosa.

## Consequências

- Todo `npm run build:appimage` (não só releases de CI) agora exige `TAURI_SIGNING_PRIVATE_KEY` (e `_PASSWORD`, se definida) exportadas antes — `tauri build` falha sem elas assim que `createUpdaterArtifacts` está habilitado. `scripts/build-appimage.sh` repassa essas variáveis para o container distrobox se estiverem presentes no ambiente de quem chamou.
- Perder a chave privada significa que nenhum release futuro poderá ser verificado como autêntico por nenhuma cópia já instalada do app. A recuperação exige gerar um novo par de chaves, embutir a nova chave pública, e pedir que todo usuário existente reinstale manualmente uma vez via `install.sh` — o updater dentro do app não consegue rotacionar sua própria chave pública sozinho, já que só confia na chave já embutida no app em execução.
- O CI passa a ser o único caminho que produz um release _completo_ (assinado + publicado); builds locais assinados continuam possíveis para teste, mas não são publicados automaticamente.
- A sobrescrita do arquivo em auto-atualização precisa ser verificada como compatível com o workaround `APPIMAGELAUNCHER_DISABLE=1` do wrapper de lançamento (`~/.local/bin/zero-desktop` gerado por `install.sh`, corrigido no commit `056760c`) — o processo relançado deve herdar essa variável de ambiente, já que processos filhos herdam o ambiente do pai, mas isso é um item para "verificar, não assumir" no primeiro teste ponta a ponta real (veja o smoke test manual no plano de implementação / doc de modelo de atualização).
