# Releasing zero-desktop

This document describes how maintainers cut a new zero-desktop release, now that publishing is automated end-to-end via GitHub Actions and consumed by the in-app auto-updater. See [ADR 005](../architecture/decisions/005-tauri-updater-for-appimage-self-update.md) and [`update-model.md`](../architecture/update-model.md) for the full design rationale — this document is the practical, step-by-step version.

## What triggers a release

Only a **tag push** matching `v*` (or a manual `workflow_dispatch` run from the Actions tab) triggers `.github/workflows/release.yml`. Regular commits and pushes to `main` build and publish nothing by themselves — a release is a deliberate, separate act.

## Step by step

1. **Bump the version** in both `package.json` and `src-tauri/tauri.conf.json`. They must hold the exact same value — the workflow validates this early and fails fast if they diverge.
2. Run `npm install` (no special flags) so `package-lock.json`'s own version field stays in sync too.
3. Commit and `git push origin main` as usual. This alone does not build or publish anything.
4. Create an annotated tag with a `v` prefix matching the new version, and push it:
   ```bash
   git tag -a v0.1.0-alpha.3 -m "..."
   git push origin v0.1.0-alpha.3
   ```
5. The tag push is what triggers the workflow. Watch it run at `https://github.com/pdl-clay/zero-desktop/actions`.

## What the workflow does automatically

The job runs inside a `quay.io/fedora/fedora:43` container — the same environment validated safe for building the AppImage locally (see `scripts/build-appimage-in-container.sh` and AGENTS.md's build rule). It:

1. Verifies the pushed tag matches the version declared in `tauri.conf.json`.
2. Installs Rust and upgrades `npm` (see the note below on why).
3. Runs `npm ci` and fetches the `zero` CLI sidecar binary.
4. Builds the signed AppImage via `scripts/build-appimage-in-container.sh`, using the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
5. Generates `latest.json` (the Tauri updater's manifest) via `scripts/generate-update-manifest.sh`.
6. Publishes the AppImage, its `.sig`, a `.sha256` checksum, and `latest.json` as assets of a new GitHub Release.

## What happens for existing installs

Once published, any zero-desktop instance running as a real AppImage (not `tauri dev`) detects the new version on its own — either on next startup or via "Check for updates" in Settings → General — downloads and installs it silently in the background, and only restarts once the user clicks "Restart now". Nothing is forced on the user without confirmation.

## Signing keys

Updates are signed with an Ed25519/minisign keypair. The public half is committed in `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`); the private half and its password exist only as the two GitHub Actions secrets above and in the maintainer's password manager — never in the repository. Losing the private key means no future release can be verified by already-installed apps; recovery requires generating a new keypair, embedding the new public key, and asking every user to reinstall once via `scripts/install.sh`.

## Known CI quirk: `npm ci` and optional peer dependencies

Fedora 43's `dnf`-provided `nodejs` can bundle an `npm` old enough to hit a known bug: `npm ci` wrongly demands that **optional** peer dependencies (e.g. `vite@8`'s optional `esbuild` peer) be present in the lockfile, even though they were never meant to be installed. The workflow runs `npm install -g npm@latest` right after installing `nodejs` via `dnf` specifically to avoid this.

If you ever see `npm error Missing: esbuild@... from lock file` in a release run again, this is almost certainly the cause, not a genuinely broken lockfile — confirm by running `npm ci` locally first: if it succeeds locally and only fails in CI, it's an `npm` version difference, not a lockfile problem.

## Scope: amd64/x86_64 only (for now)

The release pipeline currently publishes only the `linux-x86_64` platform in `latest.json`. No arm64 build has ever actually been published, and cross-building/emulating an AppImage under QEMU inside the same Fedora container that exists specifically to dodge an already-mysterious glibc/toolchain bug (see AGENTS.md's build rule) was judged too risky to bundle into the first version of this pipeline. arm64 support is a named follow-up — a matrix build plus a `linux-aarch64` entry in `latest.json` — not a silent omission.
