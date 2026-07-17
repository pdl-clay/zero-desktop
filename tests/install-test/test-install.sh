#!/usr/bin/env bash
set -euo pipefail

# Test install.sh inside multiple Linux containers.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
PORT="${PORT:-9876}"
BASE_URL="http://localhost:${PORT}"

DISTROS=(
    "fedora:latest"
    "debian:bookworm"
    "ubuntu:latest"
    "archlinux:latest"
)

cleanup() {
    if [[ -n "${SERVER_PID:-}" ]]; then
        kill "$SERVER_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

# Start local fake release server
info() {
    echo "[test] $1"
}

info "Starting fake release server on port $PORT..."
python3 "$SCRIPT_DIR/serve.py" &
SERVER_PID=$!
sleep 2

# Verify server is up
if ! curl -fsSL "$BASE_URL/pdl-clay/zero-desktop/releases/download/v0.1.0-alpha.1/zero-desktop_0.1.0-alpha.1_amd64.AppImage" >/dev/null; then
    echo "[test] Server did not start" >&2
    exit 1
fi
info "Server is up"

FAILED=0

for image in "${DISTROS[@]}"; do
    info "Testing on $image..."
    distro_name="${image%%:*}"

    if ! podman run --rm --network host --hostname "$distro_name-test" -e "ZERO_DESKTOP_BASE_URL=$BASE_URL" -e "ZERO_DESKTOP_VERSION=v0.1.0-alpha.1" "$image" bash -c "
set -e
# Install dependencies
case '$distro_name' in
    fedora)
        dnf install -y curl bash >/dev/null 2>&1
        ;;
    debian|ubuntu)
        apt-get update >/dev/null 2>&1
        apt-get install -y curl bash ca-certificates desktop-file-utils >/dev/null 2>&1
        ;;
    archlinux)
        pacman -Sy --noconfirm curl bash >/dev/null 2>&1
        ;;
esac

# Download and run install script
curl -fsSL '$BASE_URL/scripts/install.sh' -o /tmp/install.sh
bash /tmp/install.sh

# Verify installation
if [[ ! -x \$HOME/.local/apps/zero-desktop/zero-desktop.AppImage ]]; then
    echo '[test] AppImage not installed'
    exit 1
fi
if [[ ! -L \$HOME/.local/bin/zero-desktop ]]; then
    echo '[test] Symlink not created'
    exit 1
fi
if [[ ! -f \$HOME/.local/share/applications/zero-desktop.desktop ]]; then
    echo '[test] Desktop entry not created'
    exit 1
fi

echo '[test] Installation verified on $image'
"; then
        FAILED=$((FAILED + 1))
        echo "[test] FAILED: $image" >&2
    else
        info "PASSED: $image"
    fi
done

if [[ $FAILED -gt 0 ]]; then
    echo "[test] $FAILED distro(s) failed" >&2
    exit 1
fi

info "All distros passed"
