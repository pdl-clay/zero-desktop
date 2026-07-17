#!/usr/bin/env bash
set -euo pipefail

# Builds the Linux AppImage bundle with the my-zero sidecar embedded
# (bundle.externalBin in src-tauri/tauri.conf.json).
#
# Wraps `tauri build` with two environment workarounds needed on Linux:
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
#     toolchains/glibc. Left on, it aborts trying to strip perfectly valid
#     modern system libraries. Skipping the strip step just leaves debug
#     symbols in the bundled libraries (larger AppImage, no functional
#     difference).
#
# Usage: scripts/build-appimage.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
SIDECAR="src-tauri/binaries/zero-${TRIPLE}"
if [[ ! -f "$SIDECAR" ]]; then
    echo "[build-appimage] sidecar not found at $SIDECAR, fetching..."
    "$SCRIPT_DIR/fetch-zero-sidecar.sh"
fi

APPIMAGELAUNCHER_DISABLE=1 NO_STRIP=1 npx tauri build
