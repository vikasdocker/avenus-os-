#!/usr/bin/env bash
# Drive the booted Aether OS over the QEMU serial console:
# status -> restart service -> shutdown control plane -> power off VM.
set -uo pipefail
cd "$(dirname "$0")/../.."

KERNEL="${AETHER_KERNEL:-/home/vikas/aether-vmlinuz}"
INITRD=build/initramfs.cpio.gz
LOG=build/qemu-interactive.log

{
    sleep 25
    printf '\n'
    printf 'aetherctl status\n'
    sleep 5
    printf 'aetherctl restart aether-agentd\n'
    sleep 3
    printf 'aetherctl shutdown\n'
    sleep 4
    printf 'poweroff -f\n'
    sleep 3
} | timeout 60 qemu-system-x86_64 \
    -m 512M -nographic -no-reboot \
    -kernel "$KERNEL" -initrd "$INITRD" \
    -append "console=ttyS0 quiet panic=-1" >"$LOG" 2>&1

echo "== health snapshot from inside the VM =="
tr -s '\r' '\n' <"$LOG" | grep -B2 -A6 overall_health | head -14

echo "== lifecycle results =="
tr -s '\r' '\n' <"$LOG" | grep -E '"state"|Power down|SHUTTING_DOWN|RUNNING' | head -8
