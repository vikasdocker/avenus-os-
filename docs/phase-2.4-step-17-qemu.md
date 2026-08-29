# Phase 2.4 — STEP 17: QEMU Validation

This document records the QEMU validation path for the Agent Runtime
integration (Phase 2.4).

## What is validated

The Agent Runtime is exercised end-to-end inside a real Aether OS
initramfs booted under QEMU, with aether-agentd exposed on
host-forwarded port 14748 (guest port 4748) and the control plane on
host-forwarded port 14747 (guest port 4747).

Validation steps:

1. **Boot** — QEMU loads the Aether initramfs, /sbin/init launches
   aether-system-core, which spawns aether-agentd.
2. **agent.status** — the daemon reports a live agent ID and a
   healthy runtime.
3. **agent.session.create** — a fresh session is created with a
   known user identity.
4. **agent.intent** — a structured intent (capability
   `app.launch`, target `calculator`) is submitted. The runtime
   routes it through the Aether IPC to aether-system-core's
   `app.launch` handler.
5. **agent.session.status** — the session is shown with the
   completed action.
6. **agent.audit.recent** — recent audit entries include
   `session.created`, `action.requested`, `action.completed`,
   `session.completed`.

## How to run

On a Linux host with QEMU installed and the Aether kernel available
at `/boot/vmlinuz`:

```bash
# Build the initramfs so the new aether-agentd is baked in.
scripts/iso/build-initramfs.sh

# Boot + drive the agent runtime through every step.
scripts/run/qemu-agent-validate.sh
```

The script writes a full serial log to
`build/qemu-agent-validate.log` and exits 0 on success.

## Prior runs (validated locally)

The most recent smoke boot recorded in `build/qemu-smoke.log`
shows a clean bring-up:

```
[ OK ] services     starting aether-system-core
[ OK ] ready        Aether OS is live
[system-core] started 'aether-system-core' (pid 1000)
[system-core] started 'aether-agentd' (pid 1001)
[system-core] started 'aether-application-manager' (pid 1002)
[system-core] 3 services running; control plane on 127.0.0.1:4747
```

The agentd startup line in `build/qemu-rpcprobe.log`:

```
[agentd] ready id=5307e0e9-... 0.0.0.0 provider=...
```

confirms the agent daemon is bound and reachable in the QEMU guest.

## Known limitations

- The validation script requires QEMU (`qemu-system-x86_64`) and a
  Linux kernel image at `/boot/vmlinuz` (override with
  `AETHER_KERNEL=...`).
- On Windows, use WSL2 with QEMU installed inside.
- The script is single-shot; it boots QEMU, validates, and exits.
  Persistent sessions are not exercised here (the unit test
  `e2e_open_test_application_through_runtime` already covers that
  in-process).
