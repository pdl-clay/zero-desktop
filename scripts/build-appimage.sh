#!/usr/bin/env bash
set -euo pipefail

# Builds the Linux AppImage bundle with the my-zero sidecar embedded
# (bundle.externalBin in src-tauri/tauri.conf.json).
#
# ALWAYS builds inside the "dev" distrobox container (Fedora), never
# directly on the host. This isn't just about avoiding AppImageLauncher
# build-time interference (see APPIMAGELAUNCHER_DISABLE below) - an
# AppImage built directly on this host's bleeding-edge toolchain/glibc
# was broken at *runtime* in a way that never showed an error: the
# AppImage runtime process looped forever re-exec'ing itself
# (env -> bash -> env -> ...) at ~98% CPU and never got the actual app
# running. The exact same app, built inside the Fedora container instead,
# ran fine. Root cause not fully understood, but reproducible - so the
# rule is simply: never build the release AppImage on the host directly.
#
# Two environment workarounds are still needed for the build itself
# (both apply inside the container too, and are harmless if not needed):
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
#     toolchains/glibc - this affects Fedora 43's own libraries too, not
#     just this host's. Left on, it aborts trying to strip perfectly valid
#     modern system libraries. Skipping the strip step just leaves debug
#     symbols in the bundled libraries (larger AppImage, no functional
#     difference).
#
# Usage: scripts/build-appimage.sh

CONTAINER="dev"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

if ! command -v distrobox >/dev/null 2>&1; then
    echo "[build-appimage] distrobox is required (this always builds inside the '$CONTAINER' container)" >&2
    exit 1
fi

if ! distrobox list 2>/dev/null | awk -F'|' '{gsub(/ /,"",$2); print $2}' | grep -qx "$CONTAINER"; then
    echo "[build-appimage] distrobox container '$CONTAINER' not found. Create it first, e.g.:" >&2
    echo "  distrobox create --name $CONTAINER --image quay.io/fedora/fedora:43" >&2
    exit 1
fi

echo "[build-appimage] ensuring build dependencies are present in the '$CONTAINER' container..."
distrobox enter "$CONTAINER" -- bash -c '
set -euo pipefail
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
    echo "[build-appimage] installing missing packages: ${missing[*]}"
    sudo dnf install -y "${missing[@]}"
fi
'

echo "[build-appimage] checking sidecar..."
distrobox enter "$CONTAINER" -- bash -c "
set -euo pipefail
cd '$PROJECT_DIR'
triple=\"\$(rustc -vV | sed -n 's/^host: //p')\"
sidecar=\"src-tauri/binaries/zero-\$triple\"
if [[ ! -f \"\$sidecar\" ]]; then
    echo '[build-appimage] sidecar not found at '\"\$sidecar\"', fetching...'
    ./scripts/fetch-zero-sidecar.sh
fi
"

echo "[build-appimage] building inside the '$CONTAINER' container..."
distrobox enter "$CONTAINER" -- bash -c "
set -euo pipefail
cd '$PROJECT_DIR'
export CARGO_TARGET_DIR='$PROJECT_DIR/src-tauri/target-container'
export APPIMAGELAUNCHER_DISABLE=1
export NO_STRIP=1
npx tauri build
"

echo "[build-appimage] done: src-tauri/target-container/release/bundle/appimage/"
