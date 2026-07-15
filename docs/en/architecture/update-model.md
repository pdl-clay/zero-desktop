# Update Model

This document defines how **zero-desktop** manages its own updates without interfering with zero's official update mechanism (`zero update`).

## 1. Core Principle

> The lifecycle of **zero-desktop** is separate from the lifecycle of the **zero CLI**.

| Component      | Updated by              | Mechanism                                  |
| -------------- | ----------------------- | ------------------------------------------ |
| `zero-desktop` | zero-desktop            | Tauri updater                              |
| `zero` CLI     | User or official script | `zero update --check`, npm, install script |

## 2. zero-desktop Updates

During the Linux alpha, zero-desktop is distributed as an **AppImage** and updated by re-running the install script:

```bash
curl -fsSL https://raw.githubusercontent.com/pdl-clay/zero-desktop/main/scripts/install.sh | bash
```

See [`docs/en/distribution/linux-installation.md`](../distribution/linux-installation.md) for details.

In the future, zero-desktop may adopt the **official Tauri updater** for in-app updates:

- JSON endpoint with release metadata.
- Signature verification (public key embedded in the app).
- Silent download and installation when a new version is available.
- UI notification when an update is ready.

Configuration details (endpoint URL, public key) will be defined later, before the first stable release.

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
