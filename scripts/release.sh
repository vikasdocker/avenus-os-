#!/usr/bin/env bash
# Build the release binaries and assemble the Aether userspace staging tree.
# Produces build/stage/{bin,services.d} ready for initramfs packaging.
set -euo pipefail

cd "$(dirname "$0")/.."

cargo build --workspace --release

STAGE=build/stage
rm -rf "$STAGE"
mkdir -p "$STAGE/bin" "$STAGE/services.d"

for bin in aether-init aether-system-core aether-application-manager \
           aethersh aether-supervisor aether-agentd aetherctl \
           aether-sandbox; do
    cp "target/release/$bin" "$STAGE/bin/"
done

cp system/services.d/*.json "$STAGE/services.d/"

echo "stage: $STAGE"
ls -la "$STAGE/bin"
