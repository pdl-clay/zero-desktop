# Update Model

This document defines how **zero-desktop** manages its own updates without interfering with zero's official update mechanism (`zero update`).

## 1. Core Principle

> The lifecycle of **zero-desktop** is separate from the lifecycle of the **zero CLI**.

| Component      | Updated by              | Mechanism                                  |
| -------------- | ----------------------- | ------------------------------------------ |
| `zero-desktop` | zero-desktop            | Tauri updater                              |
| `zero` CLI     | User or official script | `zero update --check`, npm, install script |

## 2. zero-desktop Updates

zero-desktop is distributed as an **AppImage** and updates itself in-app using the **official Tauri updater** (`tauri-plugin-updater` + `tauri-plugin-process`). The first install still goes through the install script:

```bash
curl -fsSL https://raw.githubusercontent.com/pdl-clay/zero-desktop/main/scripts/install.sh | bash
```

See [`docs/en/distribution/linux-installation.md`](../distribution/linux-installation.md) for details. Full rationale for the choices below: [ADR 005](./decisions/005-tauri-updater-for-appimage-self-update.md).

### 2.1 Flow

- **Endpoint**: `https://github.com/pdl-clay/zero-desktop/releases/latest/download/latest.json` — a static file published alongside every GitHub Release (see `.github/workflows/release.yml`). GitHub's "latest" alias always resolves it to the newest release, so no template variables are needed.
- **Check**: on every startup, and on demand via Settings → General → About → "Check for updates".
- **Download + install**: silent, in the background, as soon as a newer version is found — no user confirmation needed to fetch the update.
- **Restart**: never automatic. Once the update is installed, a dismissible notification offers a "Restart now" button; the app keeps running the old version until the user explicitly clicks it.

### 2.2 AppImage-only activation

Self-update only makes sense when the app is actually running as the packaged AppImage — the install step overwrites the file at the `$APPIMAGE` env var, which only exists in that case. This is gated in two places:

- **Rust**: `tauri-plugin-updater` is only registered in `src-tauri/src/lib.rs`'s `setup()` when `$APPIMAGE` is present.
- **Frontend**: an `is_appimage` command lets the UI hide "Check for updates" entirely outside a real AppImage run (e.g. `tauri dev`).

### 2.3 Signing

Updates are signed with an Ed25519/minisign keypair generated via `tauri signer generate`. The public key is committed in `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`); the private key and its password live only in GitHub Actions secrets (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) and the maintainer's password manager — never in the repository.

## 3. zero CLI Detection and Installation

### 3.1 Detection

At startup, the `locator` module (`src-tauri/src/locator.rs`) searches for the `zero` binary:

1. In every directory on `PATH` (via the `which` crate).
2. In the isolated zero-desktop cache (`~/.local/share/zero-desktop/bin/zero`).
3. Via `zero --version` to confirm it is executable.

### 3.2 When Not Found

The UI presents three options:

1. **Manual instructions**: shows zero's official install command for a global install.
2. **Assisted global installation**: runs zero's official install script (`scripts/install.sh` or `scripts/install.ps1`), placing `zero` in `~/.local/bin` or `%LOCALAPPDATA%\zero\bin`.
3. **Isolated installation**: downloads the zero release binary directly into the zero-desktop cache without changing PATH.

### 3.3 Non-Conflict Policy

- zero-desktop **never** replaces a `zero` binary found on PATH.
- zero-desktop **never** runs `zero update` automatically.
- zero-desktop may, at the user's request, run `zero update --check` only to **inform** whether an update is available.

## 4. Isolated Cache

Default isolated cache locations:

| System  | Path                                                  |
| ------- | ----------------------------------------------------- |
| Linux   | `~/.local/share/zero-desktop/bin/zero`                |
| macOS   | `~/Library/Application Support/zero-desktop/bin/zero` |
| Windows | `%LOCALAPPDATA%\zero-desktop\bin\zero.exe`            |

The isolated cache is only used when:

- There is no `zero` on PATH.
- The user explicitly chose isolated installation.

## 5. Application Data Directory

zero-desktop stores its own runtime data under `~/.local/share/zero-desktop/` (Linux). Subdirectories and files:

| Path                                | Purpose                                               |
| ----------------------------------- | ----------------------------------------------------- |
| `bin/zero`                          | Isolated zero CLI binary (when PATH fallback is used) |
| `session-history/<sessionId>.jsonl` | Per-session rich event log (ACP mode)                 |
| `session-titles.json`               | User-set or auto-derived session titles               |
| `session-models.json`               | Which model answered each session                     |
| `mcp-status-cache.json`             | Last-known MCP backend health statuses                |

## 6. Compatibility Check

In the future, zero-desktop may declare a minimum zero CLI version. At startup:

- If the detected version is below the minimum, warn the user.
- Suggest updating via zero's official mechanism.

## 7. Security

- All downloads use HTTPS.
- SHA256 checksum verification when available in zero releases.
- No scripts are executed without user consent.

## 8. References

- [Tauri Updater Plugin](https://tauri.app/plugin/updater/)
- [Zero Update Flow](https://github.com/Gitlawb/zero/blob/main/docs/UPDATE.md)
- [Zero Install Scripts](https://github.com/Gitlawb/zero/blob/main/docs/INSTALL.md)
