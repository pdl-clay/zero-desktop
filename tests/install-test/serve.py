#!/usr/bin/env python3
"""Simple local server to simulate a GitHub release for install script tests."""

import hashlib
import http.server
import os
import socketserver
import sys

PORT = int(os.environ.get("PORT", "9876"))
REPO = os.environ.get("REPO", "Gitlawb/zero-desktop")
VERSION = os.environ.get("VERSION", "v0.1.0")
ARCH = os.environ.get("ARCH", "x86_64")

HERE = os.path.dirname(os.path.abspath(__file__))
FAKE_APPIMAGE = os.path.join(HERE, "zero-desktop-v0.1.0-linux-x86_64.AppImage")


def ensure_fake_appimage():
    if os.path.exists(FAKE_APPIMAGE):
        return
    source = os.path.join(HERE, "fake-appimage.sh")
    with open(source, "rb") as f:
        data = f.read()
    with open(FAKE_APPIMAGE, "wb") as f:
        f.write(data)
    os.chmod(FAKE_APPIMAGE, 0o755)

    checksum = hashlib.sha256(data).hexdigest()
    with open(FAKE_APPIMAGE + ".sha256", "w") as f:
        f.write(f"{checksum}  {os.path.basename(FAKE_APPIMAGE)}\n")


PROJECT_DIR = os.path.abspath(os.path.join(HERE, "../.."))
INSTALL_SCRIPT = os.path.join(PROJECT_DIR, "scripts", "install.sh")


class Handler(http.server.SimpleHTTPRequestHandler):
    def translate_path(self, path):
        # Serve the install script from the project root
        if path == "/scripts/install.sh":
            return INSTALL_SCRIPT
        # Map /Gitlawb/zero-desktop/releases/download/v0.1.0/<file> to local files
        expected_prefix = f"/{REPO}/releases/download/{VERSION}/"
        if path.startswith(expected_prefix):
            filename = path[len(expected_prefix):]
            return os.path.join(HERE, filename)
        return super().translate_path(path)

    def log_message(self, format, *args):
        print(f"[serve] {self.address_string()} {format % args}")


class ReusableTCPServer(socketserver.TCPServer):
    allow_reuse_address = True


if __name__ == "__main__":
    ensure_fake_appimage()
    with ReusableTCPServer(("", PORT), Handler) as httpd:
        print(f"Serving fake release at http://localhost:{PORT}/{REPO}/releases/download/{VERSION}/")
        httpd.serve_forever()
