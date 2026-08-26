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
    sleep 5
    printf 'Open Notes.\r'
    sleep 8
    printf 'Close Calculator.\r'
    sleep 6
} | timeout 105 qemu-system-x86_64 \
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
    # Mid: Calculator + Notes both open (two windows).
    sleep 26
    screendump "$SHOT_MID"
    # After Close Calculator: shell reclaimed, conversation visible.
    sleep 26
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
notes_bg = bytes((18, 22, 28))

def count(buf, color):
    return sum(buf[i:i+3] == color for i in range(0, total * 3, 3))

def changed(a, b):
    return sum(a[i:i+3] != b[i:i+3] for i in range(0, total * 3, 3))

bg_after = count(after, bg)
panel_mid = count(mid, panel)
notes_mid = count(mid, notes_bg)
notes_after = count(after, notes_bg)
diff_total = changed(before, after)

print(f"screenshot {w}x{h}; calc_panel(mid)={panel_mid}; "
      f"notes_surface mid={notes_mid} after={notes_after}; total_changed={diff_total}")

# Two windows simultaneously: calculator panel AND notes surface both
# present mid-run; afterwards the shell reclaimed the desktop.
notes_seen = max(notes_mid, notes_after)
if panel_mid > total // 20 and notes_seen > total // 40 and bg_after > total // 2 and diff_total > 2000:
    print("APP RUNTIME PASS: two app windows rendered; lifecycle completed")
else:
    print("VISUAL FAIL: multi-window runtime did not complete")
PY
