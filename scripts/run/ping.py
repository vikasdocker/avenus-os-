import socket, json, sys

def call(port, req, timeout=5):
    s = socket.create_connection(("127.0.0.1", port), timeout=timeout)
    s.settimeout(timeout)
    s.sendall((json.dumps(req) + "\n").encode())
    buf = b""
    while b"\n" not in buf:
        c = s.recv(4096)
        if not c:
            break
        buf += c
    s.close()
    return buf.decode().strip()

print("CTRL :", call(14747, {"service_id": "aether-system-core", "command": "status", "parameters": {}})[:160])
print("AGENT:", call(14748, {"command": "status"})[:120])
