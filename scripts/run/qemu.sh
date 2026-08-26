#!/usr/bin/env bash
# Boot the Aether OS initramfs under QEMU with a serial console.
#
# Usage:
#   qemu.sh                 interactive serial console
#   qemu.sh --smoke [secs]  headless boot smoke test, greps for readiness
set -euo pipefail

cd "$(dirname "$0")/../.."

KERNEL="${AETHER_KERNEL:-/boot/vmlinuz}"
INITRD=build/initramfs.cpio.gz
APPEND="${AETHER_APPEND:-console=ttyS0 panic=-1}"

if [[ ! -f "$INITRD" ]]; then
    echo "no $INITRD; run scripts/iso/build-initramfs.sh first" >&2
    exit 1
fi

if [[ "${1:-}" == "--smoke" ]]; then
    SECS="${2:-25}"
    LOG="$(pwd)/build/qemu-smoke.log"
    timeout "$SECS" qemu-system-x86_64 \
        -m 512M -nographic -no-reboot \
        -kernel "$KERNEL" -initrd "$INITRD" \
        -append "$APPEND" >"$LOG" 2>&1 || true
    if grep -q "services running; control plane" "$LOG"; then
        echo "SMOKE TEST PASS"
        grep -E "\[aether-init\]|\[system-core\]" "$LOG" | head -20
        exit 0
    else
        echo "SMOKE TEST FAIL — last serial output:" >&2
        tail -30 "$LOG" >&2
        exit 1
    fi
fi

exec qemu-system-x86_64 \
    -m 512M -nographic -no-reboot \
    -kernel "$KERNEL" -initrd "$INITRD" \
    -append "$APPEND"
