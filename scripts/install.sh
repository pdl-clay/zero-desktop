#!/usr/bin/env bash
set -euo pipefail

# zero-desktop install script for Linux
# Usage: curl -fsSL https://raw.githubusercontent.com/pdl-clay/zero-desktop/main/scripts/install.sh | bash

REPO="${ZERO_DESKTOP_REPO:-pdl-clay/zero-desktop}"
BASE_URL="${ZERO_DESKTOP_BASE_URL:-https://github.com}"
INSTALL_DIR="${ZERO_DESKTOP_INSTALL_DIR:-$HOME/.local/apps/zero-desktop}"
BIN_DIR="${ZERO_DESKTOP_BIN_DIR:-$HOME/.local/bin}"
APP_DIR="${ZERO_DESKTOP_APP_DIR:-$HOME/.local/share/applications}"
ICON_DIR="${ZERO_DESKTOP_ICON_DIR:-$HOME/.local/share/icons/hicolor/256x256/apps}"

APP_NAME="zero-desktop"
DESKTOP_FILE="$APP_DIR/${APP_NAME}.desktop"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() {
    printf "${GREEN}[info]${NC} %s\n" "$1"
}

warn() {
    printf "${YELLOW}[warn]${NC} %s\n" "$1"
}

error() {
    printf "${RED}[error]${NC} %s\n" "$1" >&2
}

detect_arch() {
    local arch
    arch="$(uname -m)"
    case "$arch" in
        x86_64)
            echo "x86_64"
            ;;
        aarch64|arm64)
            echo "aarch64"
            ;;
        *)
            error "Unsupported architecture: $arch"
            exit 1
            ;;
    esac
}

# Maps uname's arch name to the Debian/dpkg-style arch tauri-bundler uses
# when naming the AppImage it produces (<productName>_<version>_<arch>.AppImage
# - see src-tauri/tauri.conf.json's productName/version and
# `npm run build:appimage`'s output). This is a different vocabulary than
# detect_arch's uname-derived name, which is used for the user-facing
# error message and PATH/symlink naming instead.
release_asset_arch() {
    case "$1" in
        x86_64)
            echo "amd64"
            ;;
        aarch64)
            echo "arm64"
            ;;
        *)
            error "Unsupported architecture for release asset: $1"
            exit 1
            ;;
    esac
}

download() {
    local url="$1"
    local output="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "$output"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$url" -O "$output"
    else
        error "curl or wget is required"
        exit 1
    fi
}

fetch_latest_version() {
    if [[ -n "${ZERO_DESKTOP_VERSION:-}" ]]; then
        echo "$ZERO_DESKTOP_VERSION"
        return
    fi
    local api_url="https://api.github.com/repos/$REPO/releases/latest"
    local version
    version="$(download "$api_url" - | grep -oP '"tag_name":\s*"\K[^"]+' || true)"
    if [[ -z "$version" ]]; then
        error "Could not determine latest version from $api_url"
        exit 1
    fi
    echo "$version"
}

ensure_dir() {
    mkdir -p "$1"
}

main() {
    local arch
    arch="$(detect_arch)"
    info "Detected architecture: $arch"

    local version
    version="$(fetch_latest_version)"
    info "Latest version: $version"

    # tauri-bundler names the AppImage from tauri.conf.json's bare version
    # (no leading "v"), unlike the git tag / GitHub release name.
    local bare_version="${version#v}"
    local asset_arch
    asset_arch="$(release_asset_arch "$arch")"
    local asset_name="${APP_NAME}_${bare_version}_${asset_arch}.AppImage"
    local download_url="$BASE_URL/$REPO/releases/download/$version/$asset_name"
    local checksum_url="$download_url.sha256"

    info "Downloading $asset_name..."
    ensure_dir "$INSTALL_DIR"
    local temp_dir=""
    temp_dir="$(mktemp -d)"
    trap 'rm -rf "${temp_dir:-}"' EXIT

    local appimage_path="$INSTALL_DIR/$APP_NAME.AppImage"
    download "$download_url" "$temp_dir/$asset_name"

    # Verify checksum if available
    if download "$checksum_url" "$temp_dir/$asset_name.sha256" 2>/dev/null; then
        info "Verifying checksum..."
        (cd "$temp_dir" && sha256sum -c "$asset_name.sha256")
    else
        warn "Checksum not available, skipping verification"
    fi

    # Replace existing AppImage atomically
    mv "$temp_dir/$asset_name" "$appimage_path"
    chmod +x "$appimage_path"
    info "Installed AppImage to $appimage_path"

    # Ensure bin dir exists and install a launcher wrapper (not a plain symlink -
    # a symlink can't carry the APPIMAGELAUNCHER_DISABLE env var below).
    ensure_dir "$BIN_DIR"
    cat > "$BIN_DIR/$APP_NAME" <<EOF
#!/usr/bin/env bash
# See the APPIMAGELAUNCHER_DISABLE note below.
exec env APPIMAGELAUNCHER_DISABLE=1 "$appimage_path" "\$@"
EOF
    chmod +x "$BIN_DIR/$APP_NAME"
    info "Created launcher $BIN_DIR/$APP_NAME"

    # Create .desktop entry
    #
    # APPIMAGELAUNCHER_DISABLE=1: if AppImageLauncher is installed, it registers
    # a binfmt_misc handler that intercepts *any* AppImage execution and hands
    # control to its own (Qt-based) integration-prompt GUI instead of actually
    # running the AppImage. On systems missing that GUI's platform plugin (e.g.
    # no qt5-wayland on a Wayland session) it just aborts, and the failure looks
    # like a crash in zero-desktop itself (a Qt platform-plugin error, even
    # though this app doesn't use Qt at all) instead of what it is - a
    # completely unrelated launcher tool failing to start. This env var is
    # AppImageLauncher's own documented escape hatch to run the AppImage
    # directly, bypassing the interception (same fix used for the linuxdeploy
    # subprocess during the build - see scripts/build-appimage.sh).
    ensure_dir "$APP_DIR"
    cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Name=Zero Desktop
Comment=Desktop GUI for the zero coding agent
Exec=env APPIMAGELAUNCHER_DISABLE=1 $appimage_path %U
Icon=$APP_NAME
Type=Application
Terminal=false
Categories=Development;Utility;
StartupNotify=true
EOF
    info "Created desktop entry $DESKTOP_FILE"

    # Try to install icon (optional, AppImage may bundle its own)
    ensure_dir "$ICON_DIR"
    if [[ -f "$INSTALL_DIR/icon.png" ]]; then
        cp "$INSTALL_DIR/icon.png" "$ICON_DIR/$APP_NAME.png"
    fi

    # Update desktop database
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true
        info "Updated desktop database"
    fi

    info "Installation complete!"
    info "You can now run: $APP_NAME"

    if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
        warn "$BIN_DIR is not in your PATH. Add it to your shell profile:"
        warn "  export PATH=\"$BIN_DIR:\$PATH\""
    fi
}

main "$@"
