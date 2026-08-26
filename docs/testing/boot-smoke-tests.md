# Boot Smoke Tests

The boot smoke test validates the Phase 1.2 boot contract under QEMU.

## Prerequisites

- Buildroot image artifacts must exist.
- `qemu-system-x86_64` must be installed.
- Run from a Linux environment or the Docker development container.

## Command

```bash
bash scripts/test-boot.sh
```

## Verified Stages

The test fails when any required stage is missing:

1. QEMU starts.
2. Linux kernel emits `Linux version`.
3. Aether init emits `AETHER_INIT_STARTED`.
4. Aether Core emits `AETHER_CORE_READY`.
5. Shell emits `AETHER_SHELL_READY`.
6. Networking emits `AETHER_NETWORK_READY`.
7. `eth0` is visible.
8. QEMU user-network gateway responds to ping.
9. `aether-shutdown` exits cleanly.

