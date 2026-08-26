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
SHOT_AFTER=/tmp/aether-after.ppm
MON=/tmp/aether-monitor.sock

rm -f "$MON" "$SHOT_BEFORE" "$SHOT_AFTER"

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
    # The AI conversation test: UI -> Agent -> Provider -> Agent -> UI.
    printf 'Hello Aether\r'
    sleep 8
} | timeout 60 qemu-system-x86_64 \
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
    # Before the message: splash only.
    sleep 20
    screendump "$SHOT_BEFORE"
    sleep 10
    # After "Hello Aether": user line + AI reply rendered.
    screendump "$SHOT_AFTER"
) &

wait $QEMU_PID

echo "== guest checks =="
tr -s '\r' '\n' <"$LOG" | grep -E 'GFX_PROC_RUNNING|GFX_PROC_MISSING|overall_health|ai interface ready|agent replied' | head -8

echo "== display check =="
python3 - "$SHOT_BEFORE" "$SHOT_AFTER" <<'PY'
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
after, dims_a = load(sys.argv[2])

if before is None or after is None or dims_b != dims_a:
    print("VISUAL FAIL: screenshots missing or size mismatch", dims_b, dims_a)
    raise SystemExit(1)

w, h = dims_b
total = w * h
bg = bytes((14, 17, 22))
cyan = bytes((34, 211, 238))
bg_hits = sum(after[i:i+3] == bg for i in range(0, total * 3, 3))
def cyan_count(buf):
    return sum(buf[i:i+3] == cyan for i in range(0, total * 3, 3))
cyan_before = cyan_count(before)
cyan_after = cyan_count(after)
diff = sum(
    after[i:i+3] != before[i:i+3] for i in range(0, total * 3, 3)
)

print(f"screenshot {w}x{h}; bg coverage {100*bg_hits//total}%; "
      f"changed px: {diff}; cyan before={cyan_before} after={cyan_after}")
# Cyan is reserved for the fixed top bar + AI reply lines, so a jump in
# cyan proves an AI response was rendered (not just echoed user input).
if bg_hits > total // 2 and diff > 2000 and cyan_after > cyan_before + 300:
    print("VISUAL PASS: ai reply rendered on screen")
else:
    print("VISUAL FAIL: no visible conversational change")
PY
