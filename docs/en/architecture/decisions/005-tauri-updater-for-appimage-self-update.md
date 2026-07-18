# 005 — Tauri Updater for AppImage Self-Update

## Status

Accepted

## Context

Before this decision, updating zero-desktop meant manually re-running the install script (`curl ... | bash`). ADR 002 already anticipated adopting the official Tauri updater "in the future"; this ADR makes that concrete, and also covers the release pipeline needed to feed it, since none existed at all (`.github/workflows/` was empty).

Two existing constraints shaped every choice below:

- The AppImage must never be built directly on the host — see `scripts/build-appimage-in-container.sh`'s header and AGENTS.md rule 7 for the documented runtime bug (an AppImage built on a bleeding-edge host toolchain/glibc entered an infinite re-exec loop at runtime, with no build-time error). It must always be built inside a Fedora 43 environment.
- zero-desktop and the `zero` CLI have independent update lifecycles (section 3.3 of `update-model.md`). This updater must never touch the `zero` sidecar binary or run `zero update` automatically.

## Decision

- **Plugins**: adopt `tauri-plugin-updater` + `tauri-plugin-process` (official Tauri plugins), not a hand-rolled update mechanism.
- **Manifest**: a static `latest.json` published as a GitHub Release asset, referenced via GitHub's "latest" alias (`.../releases/latest/download/latest.json`) — no template-variable endpoint needed, since `latest.json` itself carries a per-platform map.
- **Artifact mode**: `bundle.createUpdaterArtifacts: true` (the current, non-legacy v2 mode). For the `appimage` target this reuses the AppImage itself and adds a sidecar `.AppImage.sig` — it does **not** produce a `.tar.gz`, so the asset-naming convention `install.sh` already relies on (`zero-desktop_<version>_<amd64|arm64>.AppImage`) is untouched.
- **Signing**: an Ed25519/minisign keypair generated via `tauri signer generate`. The public key is committed in `src-tauri/tauri.conf.json`. The private key and its password exist only as GitHub Actions secrets (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) and in the maintainer's password manager — never in the repository.
- **AppImage-only activation**: self-update only makes sense for a real AppImage run (the install step overwrites the file at `$APPIMAGE`). Gated both in Rust (conditional plugin registration in `lib.rs`'s `setup()`) and in the frontend (an `is_appimage` command hides the update UI in `tauri dev`).
- **UX**: silent background check on startup + manual check in Settings → General → About. Silent background download+install once a newer version is found. A dismissible notification offers "Restart now" — the app never restarts without explicit user confirmation. This matches the UX already sketched in `update-model.md` before this ADR.
- **Release pipeline**: a new GitHub Actions workflow (`.github/workflows/release.yml`), triggered on `v*` tags (+ manual dispatch), whose job runs with `container: quay.io/fedora/fedora:43` — reproducing the only validated-safe build environment directly, without needing distrobox (the GH Actions job container _is_ that environment). The actual build steps were extracted from `scripts/build-appimage.sh` into a new `scripts/build-appimage-in-container.sh`, shared by both the local distrobox wrapper and CI, so the build recipe has a single source of truth instead of two copies drifting apart. A new `scripts/generate-update-manifest.sh` assembles `latest.json` from the build's `.sig` file — chosen over wrapping `tauri-apps/tauri-action`, since this repo's build has enough non-standard requirements (fixed Fedora container, sidecar binary fetch, `NO_STRIP`/`APPIMAGELAUNCHER_DISABLE` env workarounds) that driving them through that action's hooks would fight the tool more than it would save. Publishing uses `softprops/action-gh-release@v2` (needs no extra `dnf install`, unlike the `gh` CLI on a bare Fedora image).
- **Scope**: amd64/x86_64 only for this first pipeline. No arm64 build has ever actually been published despite `install.sh` supporting the asset naming, and cross-building/emulating an AppImage under QEMU inside the same Fedora container that exists specifically to dodge one already-mysterious glibc/toolchain bug is a bad place to introduce a second, harder-to-diagnose one. arm64 is an explicit follow-up (a matrix build + a `linux-aarch64` entry in `latest.json`), not a silent omission.

## Consequences

- Every `npm run build:appimage` (not just CI releases) now requires `TAURI_SIGNING_PRIVATE_KEY` (and `_PASSWORD`, if set) exported first — `tauri build` hard-fails without them once `createUpdaterArtifacts` is enabled. `scripts/build-appimage.sh` forwards these into the distrobox container if present in the caller's environment.
- Losing the private key means no future release can be verified as authentic by any already-installed copy of the app. Recovery requires generating a new keypair, embedding the new public key, and asking every existing user to manually reinstall once via `install.sh` — the in-app updater cannot bootstrap a pubkey rotation on its own, since it only trusts the key already embedded in the running app.
- CI is now the only path that produces a _complete_ (signed + published) release; local signed builds remain possible for testing but are not automatically published.
- The self-update file-overwrite-in-place must be verified compatible with the `APPIMAGELAUNCHER_DISABLE=1` launcher-wrapper workaround (`install.sh`'s `~/.local/bin/zero-desktop` wrapper, fixed in commit `056760c`) — the relaunched process should inherit that env var since child processes inherit parent environment, but this is a "verify, don't assume" item for the first real end-to-end test (see the manual smoke test in the update-model doc / implementation plan).
