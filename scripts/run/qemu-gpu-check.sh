#!/usr/bin/env bash
# Headless GPU verification boot: attaches virtio-gpu-pci and probes for
# /dev/dri/card0 and /proc/fb from inside the guest over the serial console.
set -uo pipefail
cd "$(dirname "$0")/../.."

KERNEL="${AETHER_KERNEL:-/home/vikas/aether-vmlinuz}"
INITRD=build/initramfs.cpio.gz
LOG=build/qemu-gpu-check.log

{
    sleep 20
    printf 'cat /proc/fb\n'
    sleep 2
    printf 'ls -l /dev/dri/ /dev/fb0\n'
    sleep 2
    printf 'dmesg | grep -iE "drm|virtio.gpu" | tail -6\n'
    sleep 3
} | timeout 45 qemu-system-x86_64 \
    -m 512M -nographic -no-reboot \
    -device virtio-gpu-pci,xres=1024,yres=768 \
    -kernel "$KERNEL" -initrd "$INITRD" \
    -append "console=ttyS0 tsc=unstable panic=-1" >"$LOG" 2>&1

echo "== /proc/fb =="
tr -s '\r' '\n' <"$LOG" | grep -A1 '~ # cat /proc/fb' | head -4
echo "== device nodes =="
tr -s '\r' '\n' <"$LOG" | grep -A5 'ls -l /dev/dri' | head -8
echo "== drm dmesg =="
tr -s '\r' '\n' <"$LOG" | grep -iE '\[drm\]|virtio_gpu' | tail -6

GUEST_LOG="$(tr -s '\r' '\n' <"$LOG")"
# NOTE: do not pipe into `grep -q` here — under `set -o pipefail` the early
# exit makes tr report SIGPIPE (141) and the check would always fail.
if [[ "$GUEST_LOG" == *card0* && "$GUEST_LOG" == *"Initialized virtio_gpu"* ]]; then
    echo "GPU CHECK PASS"
else
    echo "GPU CHECK FAIL"
fi
