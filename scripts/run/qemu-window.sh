#!/usr/bin/env bash
# Boot Aether OS in a VISIBLE QEMU window so you can watch the boot live.
# Requires WSLg (Windows 11) or any X server. Console session runs inside.
#
# Usage: scripts/run/qemu-window.sh
set -euo pipefail

cd "$(dirname "$0")/../.."

KERNEL="${AETHER_KERNEL:-/home/vikas/aether-vmlinuz}"
INITRD=build/initramfs.cpio.gz

# Userspace logs go to /dev/console = last console= (tty0 -> the window).
# quiet+tsc=unstable: suppress kernel chatter and emulated-TSC watchdog churn,
# leaving a clean Aether-only boot screen.
APPEND="${AETHER_APPEND:-console=ttyS0 console=tty0 quiet tsc=unstable panic=-1}"

if [[ -z "${DISPLAY:-}" ]]; then
    export DISPLAY=:0
fi

if [[ ! -f "$INITRD" ]]; then
    echo "no $INITRD; run scripts/iso/build-initramfs.sh first" >&2
    exit 1
fi

exec qemu-system-x86_64 \
    -m 512M \
    -display gtk,gl=off,zoom-to-fit=on \
    -vga none \
    -device virtio-gpu-pci,xres=1024,yres=768 \
    -netdev user,id=n0,hostfwd=tcp::14748-:4748,hostfwd=tcp::14747-:4747 \
    -device virtio-net-pci,netdev=n0 \
    -no-reboot \
    -kernel "$KERNEL" -initrd "$INITRD" \
    -append "$APPEND"
