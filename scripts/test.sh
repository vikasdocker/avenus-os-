#!/usr/bin/env bash
# Run the full Aether OS test matrix: Rust + Python.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "== cargo tests =="
cargo test --workspace

echo "== python tests =="
export PYTHONPATH="brain:sdk/python${PYTHONPATH:+:$PYTHONPATH}"
python3 -m unittest discover -s tests/python

echo "test: OK"
