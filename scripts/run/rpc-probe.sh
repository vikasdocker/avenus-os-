#!/usr/bin/env bash
# Standalone RPC lifecycle probe: boots its own VM (no UI typing) and
# drives DISCOVER -> LAUNCH -> QUERY -> CLOSE through the control plane.
set -uo pipefail
cd "$(dirname "$0")/../.."

KERNEL="${AETHER_KERNEL:-/home/vikas/aether-vmlinuz}"
INITRD=build/initramfs.cpio.gz
LOG=build/qemu-rpcprobe.log
MON=/tmp/aether-monitor.sock

pkill -f qemu-system 2>/dev/null || true
rm -f "$MON"
sleep 1

{
    sleep 26
} | timeout 40 qemu-system-x86_64 \
    -m 512M -nographic -no-reboot \
    -vga none \
    -device virtio-gpu-pci,xres=1024,yres=768 \
    -netdev user,id=n0,hostfwd=tcp::14748-:4748,hostfwd=tcp::14747-:4747 \
    -device virtio-net-pci,netdev=n0 \
    -kernel "$KERNEL" -initrd "$INITRD" \
    -append "console=ttyS0 tsc=unstable panic=-1" >"$LOG" 2>&1 &
QEMU_PID=$!

python3 - <<'PY'
import socket, json, sys, time

# Wait for guest control plane through the forward.
def call(port, req, timeout=8, retries=10):
    for _ in range(retries):
        try:
            s = socket.create_connection(("127.0.0.1", port), timeout=timeout)
            s.settimeout(timeout)
            s.sendall((json.dumps(req) + "\n").encode())
            buf = b""
            while b"\n" not in buf:
                chunk = s.recv(4096)
                if not chunk:
                    break
                buf += chunk
            s.close()
            return json.loads(buf.decode().strip())
        except OSError:
            time.sleep(1)
        except json.JSONDecodeError:
            return {"ok": False, "error": {"message": "bad json"}}
    return {"ok": False, "error": {"message": "guest unreachable"}}

time.sleep(4)
print("CAP system.status :", str(call(14747, {"service_id": "ai", "command": "system.status", "parameters": {}}))[:110])
print("CAP app.list      :", json.dumps(call(14747, {"service_id": "ai", "command": "app.list", "parameters": {}}))[:200])
print("CAP app.launch    :", json.dumps(call(14747, {"service_id": "ai", "command": "app.launch", "parameters": {"app": "calculator"}})))
st = call(14747, {"service_id": "ai", "command": "app.status", "parameters": {"app": "calculator"}})
print("CAP app.status    :", json.dumps(st))
instance = None
try:
    for inst in st["result"]["report"]["instances"]:
        if inst["state"].upper() == "RUNNING":
            instance = inst["instance_id"]
except (KeyError, TypeError):
    pass
if instance is not None:
    print("CAP app.close     :", json.dumps(call(14747, {"service_id": "ai", "command": "app.close", "parameters": {"instance": instance}})))
    print("CAP app.status x2 :", json.dumps(call(14747, {"service_id": "ai", "command": "app.status", "parameters": {"app": "calculator"}})))
else:
    print("CAP close         : SKIPPED (no RUNNING instance)")
ctl = call(14747, {"service_id": "aether-system-core", "command": "status", "parameters": {}})
apps = ctl.get("result", {}).get("applications", {})
print("SYSTEM STATUS     :", apps)
PY

kill $QEMU_PID 2>/dev/null || true
