#!/usr/bin/env bash
# Boot Aether OS headless with the virtual GPU, drive the AI-first shell:
# type "Hello Aether" on the serial console, then verify via monitor
# screendump that the agent's reply actually appeared on screen.
set -uo pipefail
cd "$(dirname "$0")/../.."

KERNEL="${AETHER_KERNEL:-/home/vikas/aether-vmlinuz}"
INITRD=build/initramfs.cpio.gz
LOG=build/qemu-visual.log
SHOT_BEFORE=/tmp/aether-before.ppm
SHOT_MID=/tmp/aether-mid.ppm
SHOT_AFTER=/tmp/aether-after.ppm
MON=/tmp/aether-monitor.sock

rm -f "$MON" "$SHOT_BEFORE" "$SHOT_MID" "$SHOT_AFTER"

# aether=single: graphical shell owns serial input exclusively
# (no competing console-session reader on ttyS0).
APPEND="console=ttyS0 tsc=unstable panic=-1 aether=single"

{
    # Boot + settle; shell paints splash and arms serial input.
    sleep 24
    printf 'grep -l graphica /proc/*/comm >/dev/null 2>&1 && echo GFX_PROC_RUNNING || echo GFX_PROC_MISSING\n'
    sleep 2
    printf 'aetherctl status\n'
    sleep 4
    # Full application lifecycle through the AI UI:
    printf 'Show my applications.\r'
    sleep 6
    printf 'Open Calculator.\r'
    sleep 8
    printf 'Is Calculator running?\r'
    sleep 6
    printf 'Close Calculator.\r'
    sleep 6
} | timeout 95 qemu-system-x86_64 \
    -m 512M -nographic -no-reboot \
    -vga none \
    -device virtio-gpu-pci,xres=1024,yres=768 \
    -netdev user,id=n0,hostfwd=tcp::14748-:4748,hostfwd=tcp::14747-:4747 \
    -device virtio-net-pci,netdev=n0 \
    -monitor "unix:$MON,server,nowait" \
    -kernel "$KERNEL" -initrd "$INITRD" \
    -append "$APPEND" >"$LOG" 2>&1 &
QEMU_PID=$!

screendump() {
    python3 - "$MON" "$1" <<'PY'
import socket, sys, time
sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
sock.connect(sys.argv[1])
time.sleep(1)
sock.sendall(f"screendump {sys.argv[2]}\n".encode())
time.sleep(2)
sock.close()
PY
}

(
    for _ in $(seq 1 40); do
        [ -S "$MON" ] && break
        sleep 1
    done
    # Before any capability message: splash only.
    sleep 20
    screendump "$SHOT_BEFORE"
    # Mid: after Open Calculator the app owns the surface.
    sleep 14
    screendump "$SHOT_MID"
    # After Close Calculator: shell reclaimed, conversation visible.
    sleep 22
    screendump "$SHOT_AFTER"
) &

wait $QEMU_PID

echo "== guest checks =="
tr -s '\r' '\n' <"$LOG" | grep -E 'GFX_PROC_RUNNING|GFX_PROC_MISSING|overall_health|ai interface ready|agent replied' | head -8

echo "== display check =="
python3 - "$SHOT_BEFORE" "$SHOT_MID" "$SHOT_AFTER" <<'PY'
import sys

def load(path):
    try:
        with open(path, "rb") as f:
            data = f.read()
    except OSError:
        return None, (0, 0)
    if not data.startswith(b"P6"):
        return None, (0, 0)
    parts = data.split(b"\n", 3)
    w, h = map(int, parts[1].split())
    return parts[3], (w, h)

before, dims_b = load(sys.argv[1])
mid, dims_m = load(sys.argv[2])
after, dims_a = load(sys.argv[3])

if before is None or mid is None or after is None or len({dims_b, dims_m, dims_a}) != 1:
    print("VISUAL FAIL: screenshots missing or size mismatch", dims_b, dims_m, dims_a)
    raise SystemExit(1)

w, h = dims_b
total = w * h
bg = bytes((14, 17, 22))
cyan = bytes((34, 211, 238))
panel = bytes((28, 34, 44))

def count(buf, color):
    return sum(buf[i:i+3] == color for i in range(0, total * 3, 3))

def changed(a, b):
    return sum(a[i:i+3] != b[i:i+3] for i in range(0, total * 3, 3))

bg_after = count(after, bg)
cyan_before, cyan_mid, cyan_after = (count(x, cyan) for x in (before, mid, after))
panel_mid = count(mid, panel)
diff_total = changed(before, after)

print(f"screenshot {w}x{h}; bg {100*bg_after//total}%; "
      f"cyan {cyan_before}->{cyan_mid}->{cyan_after}; "
      f"panel_px(mid)={panel_mid}; total_changed={diff_total}")

# Criteria:
# - app surface visible mid-run: large PANEL-colored region on screen
# - lifecycle closed: final screen differs from pre-message screen
#   (conversation history) and shell reclaimed (bg back above 50%).
if panel_mid > total // 10 and bg_after > total // 2 and diff_total > 2000:
    print("APP RUNTIME PASS: calculator surface rendered and closed cleanly")
else:
    print("VISUAL FAIL: application runtime did not complete its lifecycle")
PY
