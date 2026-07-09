# Update Model

This document defines how **zero-desktop** manages its own updates without interfering with zero's official update mechanism (`zero update`).

## 1. Core Principle

> The lifecycle of **zero-desktop** is separate from the lifecycle of the **zero CLI**.

| Component | Updated by | Mechanism |
|---|---|---|
| `zero-desktop` | zero-desktop | Tauri updater |
| `zero` CLI | User or official script | `zero update --check`, npm, install script |

## 2. zero-desktop Updates

zero-desktop will use the **official Tauri updater**:

- JSON endpoint with release metadata.
- Signature verification (public key embedded in the app).
- Silent download and installation when a new version is available.
- UI notification when an update is ready.

Configuration details (endpoint URL, public key) will be defined later, before the first release.

## 3. zero CLI Detection and Installation

### 3.1 Detection

At startup, `ZeroLocator` searches for the `zero` binary:

1. In every directory on `PATH`.
2. In the isolated zero-desktop cache.
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

| System | Path |
|---|---|
| Linux | `~/.local/share/zero-desktop/bin/zero` |
| macOS | `~/Library/Application Support/zero-desktop/bin/zero` |
| Windows | `%LOCALAPPDATA%\zero-desktop\bin\zero.exe` |

The isolated cache is only used when:

- There is no `zero` on PATH.
- The user explicitly chose isolated installation.

## 5. Compatibility Check

In the future, zero-desktop may declare a minimum zero CLI version. At startup:

- If the detected version is below the minimum, warn the user.
- Suggest updating via zero's official mechanism.

## 6. Security

- All downloads use HTTPS.
- SHA256 checksum verification when available in zero releases.
- No scripts are executed without user consent.

## 7. References

- [Tauri Updater Plugin](https://tauri.app/plugin/updater/)
- [Zero Update Flow](https://github.com/Gitlawb/zero/blob/main/docs/UPDATE.md)
- [Zero Install Scripts](https://github.com/Gitlawb/zero/blob/main/docs/INSTALL.md)
