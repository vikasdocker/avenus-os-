#!/usr/bin/env bash
# Boot the Aether OS ISO under QEMU.
#
# Usage:
#   qemu-iso.sh                 interactive serial console
#   qemu-iso.sh --smoke [secs]  headless boot smoke test, greps for readiness
#
# The script picks the most recently built ISO in build/ if
# AETHER_ISO is not set.

set -euo pipefail

cd "$(dirname "$0")/../.."

if [[ -n "${AETHER_ISO:-}" ]]; then
    ISO="$AETHER_ISO"
elif [[ -f build/aether-os-0.1.0.iso ]]; then
    ISO="build/aether-os-0.1.0.iso"
else
    # Pick the freshest aether-os-*.iso.
    ISO="$(ls -1t build/aether-os-*.iso 2>/dev/null | head -1 || true)"
fi

if [[ -z "${ISO:-}" || ! -f "$ISO" ]]; then
    echo "no Aether OS ISO; run scripts/iso/build-iso.sh first" >&2
    exit 1
fi

GPU_ARGS="${AETHER_GPU_ARGS:--device virtio-gpu-pci,xres=1024,yres=768}"

if [[ "${1:-}" == "--smoke" ]]; then
    SECS="${2:-25}"
    LOG="$(pwd)/build/qemu-iso-smoke.log"
    timeout "$SECS" qemu-system-x86_64 \
        -m 512M -nographic -no-reboot \
        $GPU_ARGS \
        -cdrom "$ISO" -boot d >"$LOG" 2>&1 || true
    if grep -q "services running; control plane" "$LOG"; then
        echo "ISO SMOKE TEST PASS"
        grep -E "\[aether-init\]|\[system-core\]" "$LOG" | head -20
        exit 0
    else
        echo "ISO SMOKE TEST FAIL — last serial output:" >&2
        tail -30 "$LOG" >&2
        exit 1
    fi
fi

exec qemu-system-x86_64 \
    -m 512M -nographic -no-reboot \
    $GPU_ARGS \
    -cdrom "$ISO" -boot d
