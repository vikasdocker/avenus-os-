#!/usr/bin/env bash
# Remove build artifacts.
set -euo pipefail

cd "$(dirname "$0")/.."

cargo clean
rm -rf build/rootfs build/initramfs artifacts/*.iso 2>/dev/null || true

echo "clean: OK"
