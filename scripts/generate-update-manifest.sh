#!/usr/bin/env bash
set -euo pipefail

# Assembles the Tauri updater's static latest.json from a Linux x86_64
# build's .sig file. Run after build-appimage-in-container.sh has produced
# both the AppImage and its .sig (requires TAURI_SIGNING_PRIVATE_KEY[_PASSWORD]
# to have been set during that build - see docs/en/architecture/decisions/
# 005-tauri-updater-for-appimage-self-update.md).
#
# Only publishes the "linux-x86_64" platform key today - arm64 is an
# explicit, named follow-up (see that ADR), not silently missing.
#
# Usage:
#   scripts/generate-update-manifest.sh \
#     --version 0.1.0-alpha.2 \
#     --tag v0.1.0-alpha.2 \
#     --sig-file path/to/zero-desktop_0.1.0-alpha.2_amd64.AppImage.sig \
#     --asset-name zero-desktop_0.1.0-alpha.2_amd64.AppImage \
#     --out latest.json \
#     [--notes "release notes"]

REPO="${ZERO_DESKTOP_REPO:-pdl-clay/zero-desktop}"
NOTES=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --tag) TAG="$2"; shift 2 ;;
        --sig-file) SIG_FILE="$2"; shift 2 ;;
        --asset-name) ASSET_NAME="$2"; shift 2 ;;
        --out) OUT="$2"; shift 2 ;;
        --notes) NOTES="$2"; shift 2 ;;
        *) echo "unknown argument: $1" >&2; exit 1 ;;
    esac
done

: "${VERSION:?missing --version}" "${TAG:?missing --tag}" "${SIG_FILE:?missing --sig-file}" \
  "${ASSET_NAME:?missing --asset-name}" "${OUT:?missing --out}"

[[ -f "$SIG_FILE" ]] || { echo "[generate-update-manifest] signature file not found: $SIG_FILE" >&2; exit 1; }

SIGNATURE="$(cat "$SIG_FILE")"
PUB_DATE="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
URL="https://github.com/$REPO/releases/download/$TAG/$ASSET_NAME"

# python3 is a hard dependency of dnf itself, so it's always present in the
# Fedora 43 build environment with no extra install step.
python3 - "$VERSION" "$NOTES" "$PUB_DATE" "$SIGNATURE" "$URL" "$OUT" <<'PY'
import json, sys

version, notes, pub_date, signature, url, out = sys.argv[1:7]
manifest = {
    "version": version,
    "notes": notes,
    "pub_date": pub_date,
    "platforms": {
        "linux-x86_64": {"signature": signature, "url": url},
    },
}
with open(out, "w") as f:
    json.dump(manifest, f, indent=2)
    f.write("\n")
PY

echo "[generate-update-manifest] wrote $OUT"
