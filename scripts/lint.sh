#!/usr/bin/env bash
# Lint the whole repository: clippy for Rust, ruff (when present) for Python.
set -euo pipefail

cd "$(dirname "$0")/.."

cargo clippy --workspace --all-targets -- -D warnings

if command -v ruff >/dev/null 2>&1; then
    ruff check brain sdk/python tools tests/python
else
    echo "lint: ruff not installed; skipped python lint"
fi

echo "lint: OK"
