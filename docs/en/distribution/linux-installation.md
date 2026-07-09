# Linux Installation

This document describes how users install **zero-desktop** on Linux during the alpha phase.

## Recommended method: install script

The fastest way to install zero-desktop on any Linux distribution is via the official install script:

```bash
curl -fsSL https://raw.githubusercontent.com/<org>/zero-desktop/main/scripts/install.sh | bash
```

### What the script does

1. Detects the system architecture (`x86_64` or `aarch64`).
2. Fetches the latest release from GitHub.
3. Downloads the matching AppImage.
4. Installs the AppImage to `~/.local/apps/zero-desktop/zero-desktop.AppImage`.
5. Makes it executable.
6. Creates a symlink at `~/.local/bin/zero-desktop`.
7. Creates a `.desktop` entry at `~/.local/share/applications/zero-desktop.desktop`.
8. Updates the desktop database so the app appears in the system menu.

### Requirements

- `curl` or `wget`
- `~/.local/bin` in your `PATH`
- A desktop environment that reads `~/.local/share/applications` (GNOME, KDE, XFCE, etc.)

## Manual installation

If you prefer not to run the script, you can download the AppImage manually from the [GitHub Releases](https://github.com/<org>/zero-desktop/releases) page:

```bash
chmod +x zero-desktop-vX.Y.Z-linux-x86_64.AppImage
./zero-desktop-vX.Y.Z-linux-x86_64.AppImage
```

To integrate it into your system menu, copy the AppImage to `~/.local/apps/zero-desktop/` and create a `.desktop` file manually.

## First run

On first launch, zero-desktop checks whether the [zero](https://github.com/Gitlawb/zero) CLI is installed on your `PATH`. If it is not found, an installation assistant offers two options:

1. **Global installation** — runs zero's official install script, placing `zero` in `~/.local/bin`.
2. **Isolated installation** — downloads `zero` to the zero-desktop cache (`~/.local/share/zero-desktop/bin/zero`) without touching system directories.

## Updating

Re-run the install script to update to the latest version:

```bash
curl -fsSL https://raw.githubusercontent.com/<org>/zero-desktop/main/scripts/install.sh | bash
```

The script replaces the existing AppImage while preserving your local data.

## Uninstalling

Run:

```bash
zero-desktop --uninstall
```

Or manually remove:

```bash
rm -rf ~/.local/apps/zero-desktop
rm ~/.local/bin/zero-desktop
rm ~/.local/share/applications/zero-desktop.desktop
rm -rf ~/.local/share/zero-desktop
```

## Supported architectures

| Architecture | Package name |
|---|---|
| x86_64 | `zero-desktop-vX.Y.Z-linux-x86_64.AppImage` |
| aarch64 | `zero-desktop-vX.Y.Z-linux-aarch64.AppImage` |

## Notes

- AppImage is the only alpha distribution format.
- `.deb`, `.rpm`, Flatpak, Snap, and other formats are planned for later releases.
- The install script does not require root privileges.
