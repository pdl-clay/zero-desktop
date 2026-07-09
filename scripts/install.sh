#!/usr/bin/env bash
set -euo pipefail

# zero-desktop install script for Linux
# Usage: curl -fsSL https://raw.githubusercontent.com/Gitlawb/zero-desktop/main/scripts/install.sh | bash

REPO="${ZERO_DESKTOP_REPO:-Gitlawb/zero-desktop}"
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

    local asset_name="${APP_NAME}-${version}-linux-${arch}.AppImage"
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

    # Ensure bin dir exists and create symlink
    ensure_dir "$BIN_DIR"
    if [[ -L "$BIN_DIR/$APP_NAME" ]]; then
        rm "$BIN_DIR/$APP_NAME"
    fi
    ln -s "$appimage_path" "$BIN_DIR/$APP_NAME"
    info "Created symlink $BIN_DIR/$APP_NAME"

    # Create .desktop entry
    ensure_dir "$APP_DIR"
    cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Name=Zero Desktop
Comment=Desktop GUI for the zero coding agent
Exec=$appimage_path %U
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
