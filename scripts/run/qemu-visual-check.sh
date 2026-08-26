#!/usr/bin/env bash
# Boot Aether OS headless with the virtual GPU, then capture the actual
# QEMU display via monitor screendump and verify the graphical shell's
# splash is visible by counting its background/accent pixels.
set -uo pipefail
cd "$(dirname "$0")/../.."

KERNEL="${AETHER_KERNEL:-/home/vikas/aether-vmlinuz}"
INITRD=build/initramfs.cpio.gz
LOG=build/qemu-visual.log
SHOT=/tmp/aether-screen.ppm
MON=/tmp/aether-monitor.sock

rm -f "$MON" "$SHOT"

{
    sleep 22
    # NOTE: busybox truncates task names to 15 chars ("aether-graphica");
    # match the truncated substring via /proc instead of pgrep.
    printf 'grep -l graphica /proc/*/comm >/dev/null 2>&1 && echo GFX_PROC_RUNNING || echo GFX_PROC_MISSING\n'
    sleep 2
    printf 'ls -l /dev/dri/card0 /dev/fb0\n'
    sleep 2
    printf 'aetherctl status\n'
    sleep 3
} | timeout 50 qemu-system-x86_64 \
    -m 512M -nographic -no-reboot \
    -vga none \
    -device virtio-gpu-pci,xres=1024,yres=768 \
    -monitor "unix:$MON,server,nowait" \
    -kernel "$KERNEL" -initrd "$INITRD" \
    -append "console=ttyS0 tsc=unstable panic=-1" >"$LOG" 2>&1 &
QEMU_PID=$!

# Drive the HMP monitor over a unix socket: wait for boot, take a screenshot.
(
    for _ in $(seq 1 40); do
        [ -S "$MON" ] && break
        sleep 1
    done
    sleep 18
    python3 - "$MON" <<'PY'
import socket, sys, time
sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
sock.connect(sys.argv[1])
time.sleep(1)
sock.sendall(b"\nscreendump /tmp/aether-screen.ppm\n")
time.sleep(2)
sock.close()
PY
) &

wait $QEMU_PID

echo "== guest checks =="
tr -s '\r' '\n' <"$LOG" | grep -E 'GFX_PROC_RUNNING|GFX_PROC_MISSING|overall_health|card0|fb0' | head -8

echo "== display check =="
python3 - "$SHOT" <<'PY'
import sys

try:
    with open(sys.argv[1], "rb") as f:
        data = f.read()
except OSError:
    print("VISUAL FAIL: no screenshot captured")
    raise SystemExit(1)

assert data.startswith(b"P6"), "not a PPM"
parts = data.split(b"\n", 3)
w, h = map(int, parts[1].split())
px = parts[3]

bg = bytes((14, 17, 22))       # aether background
cyan = bytes((34, 211, 238))   # accent bar (PPM stores RGB)
total = w * h
bg_hits = sum(px[i:i+3] == bg for i in range(0, total * 3, 3))
cyan_hits = sum(px[i:i+3] == cyan for i in range(0, total * 3, 3))

print(f"screenshot {w}x{h}; bg coverage {100*bg_hits//total}%")
if bg_hits > total // 2 and cyan_hits > 500:
    print("VISUAL PASS: aether splash is on screen")
else:
    print("VISUAL FAIL: unexpected screen content")
PY
