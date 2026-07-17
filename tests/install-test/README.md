# Install Script Tests

This directory contains tests for `scripts/install.sh`.

## Local Test

You can test the install script locally against a fake release server:

```bash
# Terminal 1: start fake release server
python3 tests/install-test/serve.py

# Terminal 2: run install script against local server
export ZERO_DESKTOP_BASE_URL=http://localhost:9876
export ZERO_DESKTOP_VERSION=v0.1.0-alpha.1
bash scripts/install.sh
```

## Container Tests

Run the automated container test script with **Podman**:

```bash
bash tests/install-test/test-install.sh
```

This tests the install script on:

- Fedora
- Debian
- Ubuntu
- Arch Linux

## CI

Container tests also run on every push to `main` and on pull requests that modify `scripts/install.sh` or `tests/install-test/**`. The CI uses Podman as well.

See [`.github/workflows/test-install.yml`](../../.github/workflows/test-install.yml).
