#!/usr/bin/env bash
# Build every Aether OS Rust component (debug profile by default).
set -euo pipefail

cd "$(dirname "$0")/.."

PROFILE="${1:-debug}"
if [[ "$PROFILE" == "release" ]]; then
    cargo build --workspace --release
else
    cargo build --workspace
fi

echo "build: OK ($PROFILE)"
