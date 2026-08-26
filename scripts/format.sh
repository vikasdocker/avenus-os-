#!/usr/bin/env bash
# Format all Rust sources with rustfmt.
set -euo pipefail

cd "$(dirname "$0")/.."

cargo fmt --all

echo "format: OK"
