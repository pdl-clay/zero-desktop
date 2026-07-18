#!/usr/bin/env bash
set -euo pipefail

# Thin wrapper: enters the local "dev" distrobox container (Fedora 43) and
# runs the actual build recipe from scripts/build-appimage-in-container.sh
# there. See that script's header for why Fedora 43 specifically, and
# AGENTS.md rule 7 for the underlying host-build bug this works around.
#
# If TAURI_SIGNING_PRIVATE_KEY[_PASSWORD] are set in this shell's
# environment (needed for a signed build - see docs/en/architecture/
# decisions/005-tauri-updater-for-appimage-self-update.md), they are
# forwarded into the container explicitly, since distrobox does not import
# the host's environment by default.
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

echo "[build-appimage] building inside the '$CONTAINER' container..."
distrobox enter "$CONTAINER" -- env \
    ${TAURI_SIGNING_PRIVATE_KEY:+TAURI_SIGNING_PRIVATE_KEY="$TAURI_SIGNING_PRIVATE_KEY"} \
    ${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:+TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$TAURI_SIGNING_PRIVATE_KEY_PASSWORD"} \
    bash -c "cd '$PROJECT_DIR' && ./scripts/build-appimage-in-container.sh"
