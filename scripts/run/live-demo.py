#!/usr/bin/env python3
"""Live demo driver: opens Calculator and Notes in the running VM
through the host-forwarded control plane, leaving them on screen."""
import json
import socket
import sys
import time

def call(port, req, timeout=8):
    for _ in range(10):
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
    return {"ok": False}

# Wait for full boot (services + control plane).
time.sleep(20)

st = call(14747, {"service_id": "aether-system-core", "command": "system.status", "parameters": {}})
apps = st.get("result", {}).get("applications", {})
print(f"[demo] control plane up - applications installed={apps.get('installed')}")

r = call(14747, {"service_id": "ai", "command": "app.launch", "parameters": {"app": "calculator"}})
print("[demo] open calculator:", "OK" if r.get("ok") else r)
time.sleep(6)

r = call(14747, {"service_id": "ai", "command": "app.launch", "parameters": {"app": "notes"}})
print("[demo] open notes     :", "OK" if r.get("ok") else r)
time.sleep(4)

wl = call(14748, {"op": "window.list"})
wins = wl.get("windows", [])
print(f"[demo] windows open   : {[w['title'] for w in wins]}")
