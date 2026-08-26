#!/usr/bin/env bash
# Live demo: boot the Aether control plane on this host.
# Starts aether-system-core with the bundled service manifests, then drives
# it with aetherctl: status -> restart a service -> status -> shutdown.
set -euo pipefail

cd "$(dirname "$0")/.."

cargo build --release -p aether-system-core -p aetherctl

export AETHER_CONTROL_PORT="${AETHER_CONTROL_PORT:-4799}"
MANIFESTS="$(pwd)/system/services.d"

./target/release/aether-system-core "$MANIFESTS" &
CORE_PID=$!
trap 'kill "$CORE_PID" 2>/dev/null || true' EXIT
sleep 0.5

CTL=./target/release/aetherctl

echo "== aetherctl status =="
$CTL status

echo "== aetherctl restart aether-agentd =="
$CTL restart aether-agentd

echo "== aetherctl shutdown =="
$CTL shutdown

wait "$CORE_PID" 2>/dev/null || true
echo "demo: OK"
