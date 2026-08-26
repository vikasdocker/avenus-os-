#!/usr/bin/env bash
# Host-side live probe: while the visual-check VM runs, connect from the
# HOST through the QEMU hostfwd straight into the guest agent on 4748.
set -uo pipefail
cd "$(dirname "$0")/../.."

bash scripts/run/qemu-visual-check.sh >build/qemu-visual-run.log 2>&1 &
CHECK_PID=$!

sleep 40   # boot + settle; agentd listening by now

python3 - <<'PY'
import socket, json

def call(port, req, timeout=8):
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
        return buf.decode().strip()
    except OSError as e:
        return f"PROBE ERROR: {e}"

print("GUEST AGENT status :", call(14748, {"command": "status"}))
print("GUEST AGENT chat   :", call(14748, {"command": "chat", "argument": "HOST PROBE"}))
print("CONTROL PLANE      :", call(14747, {"service_id": "aether-system-core", "command": "status", "parameters": {}})[:120])
PY

wait $CHECK_PID
grep -E 'VISUAL|changed|cyan' build/qemu-visual-run.log | head -3
