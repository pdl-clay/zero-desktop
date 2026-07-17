#!/usr/bin/env bash
set -euo pipefail

# Downloads the "my-zero" fork's `zero` binary (our patched build - advisor
# mode's Task tool support, plan mode via ACP) from its GitHub Releases and
# places it at src-tauri/binaries/zero-<target-triple>[.exe], the naming
# Tauri's `bundle.externalBin` expects (see tauri.conf.json). Run this
# before `tauri build`/`tauri dev` whenever the binaries/ directory is
# empty (it's gitignored - the binary itself is never committed).
#
# Usage: scripts/fetch-zero-sidecar.sh [version]
#   version defaults to the latest release if omitted.

REPO="${MY_ZERO_REPO:-pdl-clay/my-zero}"
BASE_URL="${MY_ZERO_BASE_URL:-https://github.com}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_DIR="$SCRIPT_DIR/../src-tauri/binaries"

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'
info() { printf "${GREEN}[info]${NC} %s\n" "$1"; }
error() { printf "${RED}[error]${NC} %s\n" "$1" >&2; }

download() {
    local url="$1" output="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "$output"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$url" -O "$output"
    else
        error "curl or wget is required"
        exit 1
    fi
}

# Maps `rustc -vV`'s host triple to my-zero's release platform/arch/archive
# naming (internal/release/release.go: ReleasePlatform/ReleaseArch/
# ReleaseArchiveExtension in the fork) - zero-desktop's own Rust target
# triple, not Go's GOOS/GOARCH, is the input since that's what
# `bundle.externalBin`'s file naming (zero-<target-triple>) is keyed on.
resolve_target() {
    local triple
    triple="$(rustc -vV | sed -n 's/^host: //p')"
    case "$triple" in
        x86_64-unknown-linux-gnu)   echo "$triple linux x64 tar.gz" ;;
        aarch64-unknown-linux-gnu)  echo "$triple linux arm64 tar.gz" ;;
        x86_64-apple-darwin)        echo "$triple macos x64 tar.gz" ;;
        aarch64-apple-darwin)       echo "$triple macos arm64 tar.gz" ;;
        x86_64-pc-windows-msvc)     echo "$triple windows x64 zip" ;;
        *)
            error "no my-zero release mapping for target triple: $triple"
            exit 1
            ;;
    esac
}

main() {
    local version="${1:-}"
    if [[ -z "$version" ]]; then
        version="$(download "https://api.github.com/repos/$REPO/releases/latest" - | grep -oP '"tag_name":\s*"\K[^"]+' || true)"
        if [[ -z "$version" ]]; then
            error "could not determine the latest my-zero release"
            exit 1
        fi
    fi
    # Strip a leading "v" - ReleasePackageName embeds the bare version.
    local bare_version="${version#v}"

    read -r triple platform arch ext <<<"$(resolve_target)"
    info "target: $triple ($platform-$arch), my-zero $version"

    local asset="zero-v${bare_version}-${platform}-${arch}.${ext}"
    local url="$BASE_URL/$REPO/releases/download/$version/$asset"

    local temp_dir
    temp_dir="$(mktemp -d)"
    trap 'rm -rf "$temp_dir"' EXIT

    info "downloading $asset..."
    download "$url" "$temp_dir/$asset"
    if download "$url.sha256" "$temp_dir/$asset.sha256" 2>/dev/null; then
        info "verifying checksum..."
        (cd "$temp_dir" && sha256sum -c "$asset.sha256")
    fi

    info "extracting..."
    if [[ "$ext" == "zip" ]]; then
        unzip -q "$temp_dir/$asset" -d "$temp_dir/extracted"
    else
        mkdir -p "$temp_dir/extracted"
        tar -xzf "$temp_dir/$asset" -C "$temp_dir/extracted"
    fi

    local bin_name="zero"
    [[ "$platform" == "windows" ]] && bin_name="zero.exe"
    local found
    found="$(find "$temp_dir/extracted" -type f -name "$bin_name" | head -n1)"
    if [[ -z "$found" ]]; then
        error "could not find $bin_name inside $asset"
        exit 1
    fi

    mkdir -p "$OUT_DIR"
    local dest="$OUT_DIR/zero-${triple}"
    [[ "$platform" == "windows" ]] && dest="$dest.exe"
    cp "$found" "$dest"
    chmod +x "$dest"
    info "installed sidecar at $dest"
}

main "$@"
