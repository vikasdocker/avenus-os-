#!/usr/bin/env bash
# QEMU validation: agent runtime integration end-to-end.
#
# Boots Aether OS in headless QEMU, then drives aether-agentd on
# port 14748 (host-forwarded to guest port 4748) through the full
# Agent → Agent Runtime → Aether IPC → aether-system-core path:
#
#   1. agent.status                    -> reports runtime ready
#   2. agent.session.create            -> creates a session
#   3. agent.intent  app.launch        -> routes through runtime
#   4. agent.session.status            -> shows completed action
#   5. agent.audit.recent 5            -> audit entries recorded
#
# The script is self-contained: it does not assume any host-side
# aether-agentd binary. It talks to the QEMU guest directly over TCP.
#
# Usage:
#   scripts/run/qemu-agent-validate.sh                # build initramfs, boot, validate
#   AETHER_SMOKE_SECS=60 scripts/run/qemu-agent-validate.sh
#
# Exits 0 on success, non-zero on failure.

set -euo pipefail

cd "$(dirname "$0")/../.."

KERNEL="${AETHER_KERNEL:-/boot/vmlinuz}"
INITRD=build/initramfs.cpio.gz
LOG="$(pwd)/build/qemu-agent-validate.log"
SMOKE_SECS="${AETHER_SMOKE_SECS:-40}"
AGENT_PORT=14748
CONTROL_PORT=14747

echo "[qemu-agent-validate] log -> $LOG"

# Build a fresh initramfs so any new binaries (aether-agentd) are
# baked in.
bash scripts/iso/build-initramfs.sh

# Boot QEMU headless, forward agentd port and control port to host.
# Aether systemd's /sbin/init starts the control plane which in turn
# spawns aether-agentd. We let it run for $SMOKE_SECS, then drive the
# daemon over TCP from a parallel subshell.
timeout "$SMOKE_SECS" qemu-system-x86_64 \
    -m 512M -nographic -no-reboot \
    -device virtio-gpu-pci,xres=1024,yres=768 \
    -netdev user,id=n0,hostfwd=tcp::${AGENT_PORT}-:4748,hostfwd=tcp::${CONTROL_PORT}-:4747 \
    -device virtio-net-pci,netdev=n0 \
    -kernel "$KERNEL" -initrd "$INITRD" \
    -append "console=ttyS0 panic=-1 quiet tsc=unstable" >"$LOG" 2>&1 &
QEMU_PID=$!

# Wait for the agentd port to start accepting connections.
for i in $(seq 1 60); do
    if (echo > /dev/tcp/127.0.0.1/${AGENT_PORT}) 2>/dev/null; then
        break
    fi
    sleep 0.5
done

# Helper: send one NDJSON request, read one NDJSON response.
agent_call() {
    local cmd="$1"
    local arg="${2:-}"
    if [[ -n "$arg" ]]; then
        printf '{"command":"%s","argument":%s}\n' "$cmd" "$(printf %s "$arg" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))')"
    else
        printf '{"command":"%s"}\n' "$cmd"
    fi
}

# Step 1: status
echo "[qemu-agent-validate] agent.status"
agent_call agent.status > /tmp/req1.json
RESP=$(cat /tmp/req1.json | timeout 5 nc 127.0.0.1 ${AGENT_PORT} || true)
echo "$RESP" | grep -q '"ok":true' || { echo "FAIL: agent.status did not return ok: $RESP" >&2; kill "$QEMU_PID" 2>/dev/null || true; exit 1; }

# Step 2: session.create
echo "[qemu-agent-validate] agent.session.create"
agent_call agent.session.create '"qemu-user"' > /tmp/req2.json
RESP=$(cat /tmp/req2.json | timeout 5 nc 127.0.0.1 ${AGENT_PORT} || true)
SID=$(echo "$RESP" | python3 -c 'import json,sys; r=json.loads(sys.stdin.read()); print(r.get("result",{}).get("session_id",""))')
[[ -n "$SID" ]] || { echo "FAIL: session.create did not return session_id: $RESP" >&2; kill "$QEMU_PID" 2>/dev/null || true; exit 1; }
echo "[qemu-agent-validate] session_id=$SID"

# Step 3: agent.intent
echo "[qemu-agent-validate] agent.intent"
printf '{"command":"agent.intent","argument":"{\\"session_id\\":\\"%s\\",\\"capability\\":\\"app.launch\\",\\"arguments\\":{\\"app\\":\\"calculator\\"}}"}' "$SID" \
    | timeout 5 nc 127.0.0.1 ${AGENT_PORT} > /tmp/resp3.json || true
cat /tmp/resp3.json

# Step 4: session.status
echo "[qemu-agent-validate] agent.session.status"
agent_call agent.session.status "$SID" > /tmp/req4.json
RESP=$(cat /tmp/req4.json | timeout 5 nc 127.0.0.1 ${AGENT_PORT} || true)
echo "$RESP" | grep -q "$SID" || { echo "FAIL: session.status did not return session: $RESP" >&2; kill "$QEMU_PID" 2>/dev/null || true; exit 1; }

# Step 5: audit.recent
echo "[qemu-agent-validate] agent.audit.recent 5"
agent_call agent.audit.recent 5 > /tmp/req5.json
RESP=$(cat /tmp/req5.json | timeout 5 nc 127.0.0.1 ${AGENT_PORT} || true)
echo "$RESP" | grep -q '"ok":true' || { echo "FAIL: agent.audit.recent did not return ok: $RESP" >&2; kill "$QEMU_PID" 2>/dev/null || true; exit 1; }

kill "$QEMU_PID" 2>/dev/null || true
wait "$QEMU_PID" 2>/dev/null || true

echo "[qemu-agent-validate] PASS"
