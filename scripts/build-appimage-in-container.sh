#!/usr/bin/env bash
set -euo pipefail

# Actual AppImage build recipe. Assumes it is ALREADY running inside Fedora
# 43 with rustc/cargo/node/npx on PATH - it never enters a container itself.
# This is the single source of truth for "how to build zero-desktop's
# AppImage", used both by scripts/build-appimage.sh (which enters the local
# distrobox "dev" container first) and by .github/workflows/release.yml
# (whose job container IS Fedora 43 already, no distrobox needed there).
#
# Why Fedora 43 specifically, and not the host: see build-appimage.sh's
# header and AGENTS.md rule 7 - an AppImage built directly on a bleeding-edge
# host toolchain/glibc was broken at *runtime* (infinite re-exec loop at
# ~98% CPU, no visible error) despite building successfully. The exact same
# app, built inside this Fedora 43 environment, ran fine. Never build the
# release AppImage anywhere else.
#
# Two environment workarounds are still needed for the build itself:
#
#   - APPIMAGELAUNCHER_DISABLE=1: if AppImageLauncher is installed, it
#     intercepts *any* AppImage execution via a binfmt_misc handler and pops
#     up a GUI integration dialog. Tauri's bundler runs the linuxdeploy
#     AppImage (and its plugin AppImages) as subprocesses while packaging;
#     without a display those get hijacked and abort (SIGABRT), failing the
#     whole build with "failed to run linuxdeploy". This variable is
#     AppImageLauncher's own documented escape hatch to run the AppImage
#     directly instead of intercepting it.
#   - NO_STRIP=1: linuxdeploy bundles its own (old) copy of GNU strip, which
#     doesn't recognize the `.relr.dyn` ELF section emitted by newer
#     toolchains/glibc - this affects Fedora 43's own libraries too. Left on,
#     it aborts trying to strip perfectly valid modern system libraries.
#     Skipping the strip step just leaves debug symbols in the bundled
#     libraries (larger AppImage, no functional difference).
#
# Note: with bundle.createUpdaterArtifacts enabled in tauri.conf.json,
# `tauri build` now hard-fails without TAURI_SIGNING_PRIVATE_KEY (and
# TAURI_SIGNING_PRIVATE_KEY_PASSWORD, if the key has one) exported - this
# applies to every build, not just CI releases. See docs/en/architecture/
# decisions/005-tauri-updater-for-appimage-self-update.md.
#
# Usage: scripts/build-appimage-in-container.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# CI containers run as root (no `sudo` binary at all); the local distrobox
# container runs as a regular user who needs `sudo dnf`.
DNF="dnf"
[[ "$(id -u)" -ne 0 ]] && DNF="sudo dnf"

echo "[build-appimage-in-container] ensuring build dependencies are present..."
missing=()
for pkg_check in "node:nodejs" "cc:gcc" "patchelf:patchelf" "mksquashfs:squashfs-tools" "file:file"; do
    bin="${pkg_check%%:*}"; pkg="${pkg_check##*:}"
    command -v "$bin" >/dev/null 2>&1 || missing+=("$pkg")
done
pkg-config --exists gtk+-3.0 2>/dev/null || missing+=("gtk3-devel")
pkg-config --exists webkit2gtk-4.1 2>/dev/null || missing+=("webkit2gtk4.1-devel")
pkg-config --exists javascriptcoregtk-4.1 2>/dev/null || missing+=("javascriptcoregtk4.1-devel")
pkg-config --exists librsvg-2.0 2>/dev/null || missing+=("librsvg2-devel")
[[ -f /usr/lib64/libfuse.so.2 || -f /usr/lib/libfuse.so.2 ]] || missing+=("fuse" "fuse-libs")
if [[ ${#missing[@]} -gt 0 ]]; then
    echo "[build-appimage-in-container] installing missing packages: ${missing[*]}"
    $DNF install -y "${missing[@]}"
fi

echo "[build-appimage-in-container] checking sidecar..."
cd "$PROJECT_DIR"
triple="$(rustc -vV | sed -n 's/^host: //p')"
sidecar="src-tauri/binaries/zero-$triple"
if [[ ! -f "$sidecar" ]]; then
    echo "[build-appimage-in-container] sidecar not found at $sidecar, fetching..."
    ./scripts/fetch-zero-sidecar.sh
fi

echo "[build-appimage-in-container] building..."
export CARGO_TARGET_DIR="$PROJECT_DIR/src-tauri/target-container"
export APPIMAGELAUNCHER_DISABLE=1
export NO_STRIP=1
npx tauri build

echo "[build-appimage-in-container] done: src-tauri/target-container/release/bundle/appimage/"
