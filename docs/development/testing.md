# Testing Strategy

The test strategy validates each language domain, repository contracts, boot artifacts,
and Phase 1.4 System Core hardening.

| Test Area | Command | Purpose |
| --- | --- | --- |
| Rust unit tests | `cargo test --workspace` | Validate PID1 parsing, service protocol, supervisor behavior, and SDK contracts. |
| C build and tests | `cmake --build ...` and `ctest` | Validate system utilities and health report behavior. |
| Python unit tests | `python -m unittest discover -s tests/python` | Validate deterministic AI brain behavior and Python SDK behavior. |
| Integration tests | `python -m unittest discover -s tests/integration` | Validate cross-repository structure and workspace contracts. |
| Smoke tests | `python -m unittest discover -s tests/smoke` | Validate bootstrapping inputs for the first ISO path. |
| Boot tests | `bash scripts/test-boot.sh` | Boot the Buildroot image under QEMU and verify kernel, init, service, shell, network, and shutdown. |
| Repository policy | `python tests/repository/test_policy.py` | Validate file policy, service descriptors, and script quality guardrails. |
| Initramfs build | `bash scripts/iso/build-initramfs.sh` | Validate that boot payload assembly succeeds. |
| ISO build | `bash scripts/iso/build-iso.sh` | Validate bootable ISO assembly when a kernel image is available. |
| Phase 1.4 hardening contracts | `python -m unittest discover -s tests/integration` | Validate manifest security declarations, resource bounds, IPC hardening declarations, and audit/permission modules. |

Phase 1.4 cannot be closed using source-level tests alone. Release candidates must run:

```bash
cargo test --workspace
bash scripts/test.sh
bash scripts/lint.sh
bash scripts/build.sh
bash scripts/build/build.sh
bash scripts/test-boot.sh
```

After QEMU boot, validate runtime hardening:

```sh
aetherctl services
aetherctl system status
aetherctl system audit
stat -c '%a %n' /run/aether/ipc/aether-system-core.sock
```

The expected IPC socket permission is `600`. Invalid and oversized IPC requests must
return errors without terminating Aether System Core.
