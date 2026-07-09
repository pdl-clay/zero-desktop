# 002 — Linux Distribution via AppImage + Install Script

## Status

Accepted

## Context

During the alpha phase, zero-desktop targets Linux first. We need a distribution method that is:

- Simple for users to install.
- Universal across Linux distributions.
- Easy for the team to build and maintain.
- Capable of providing system menu integration without relying on package managers.

## Decision

Use **AppImage** as the sole alpha distribution format for Linux, combined with an **install script** that handles system integration.

The install script:

1. Detects the architecture (`x86_64`, `aarch64`).
2. Downloads the latest AppImage from GitHub Releases.
3. Installs it to `~/.local/apps/zero-desktop/`.
4. Creates a symlink in `~/.local/bin/`.
5. Creates a `.desktop` entry in `~/.local/share/applications/`.
6. Updates the desktop database.

Other formats such as `.deb`, `.rpm`, Flatpak, and Snap are deferred to later releases.

## Consequences

- Users can install with a single command.
- No root privileges are required.
- AppImage works on most Linux distributions.
- The install script compensates for AppImage's lack of automatic system integration.
- The release pipeline only needs to produce AppImage artifacts, reducing CI complexity.
- Windows and macOS ports will be planned after the Linux alpha stabilizes.
