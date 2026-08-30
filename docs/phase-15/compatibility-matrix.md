# Aether OS 0.2.0 — Compatibility Matrix

Last updated 2026-08-30. This matrix enumerates every target Aether OS
0.2.0 supports, the test platform that validates the support, and the
expected behavior on each tier.

## Tier 1 — Reference

These are the platforms Aether OS is *guaranteed* to work on. Every
release is validated against them.

| Target                | Validation                                          | Notes                                  |
| --------------------- | --------------------------------------------------- | -------------------------------------- |
| QEMU `virtio_gpu`     | `scripts/run/qemu.sh` + `qemu-visual-check.sh`      | Primary reference platform.            |
| QEMU `virtio_net`     | `scripts/run/qemu.sh` + `qemu-agent-validate.sh`    | DHCP via BusyBox `udhcpc`.             |
| QEMU text console     | `scripts/run/qemu.sh`                               | Headless, no GPU.                      |
| Linux host (x86_64)   | `scripts/demo.sh`                                   | Control plane on loopback port 4799.   |

## Tier 2 — Best-effort

These platforms are not part of the reference gate, but the kernel
configuration and drivers are known to be sufficient. Releases will
not be blocked on a regression here.

| Target                | Validation                                          | Notes                                  |
| --------------------- | --------------------------------------------------- | -------------------------------------- |
| QEMU with serial only | `scripts/run/qemu.sh --serial-only`                 | Used by `qemu-structured-output-validate.sh`. |
| Vagrant / VirtualBox  | Manual, `infra/buildroot/board/README.md`           | Untested in CI.                        |
| VMware                | Manual                                              | Untested in CI.                        |
| Docker dev shell      | `make docker-shell`                                 | Builds inside a Rust 1.82+ container.  |

## Tier 3 — Future

Documented in the roadmap; not in 0.2.0.

| Target                | Roadmap reference                                   | Notes                                  |
| --------------------- | --------------------------------------------------- | -------------------------------------- |
| Native DRM/KMS        | Phase 1.9 / Phase 6                                 | Deferred. The virtio path covers QEMU. |
| Real hardware (laptop, desktop, IoT) | Phase 15 hardware matrix             | Reference hardware list not yet ratified. |
| `aether-sandbox` enforced seccomp-bpf | Phase 11.4                  | Linux-only, deferred.                  |

## Micro-benchmarks

Recorded on the reference platform with `cargo run --release
-p aether-bench`. Numbers are illustrative; the harness is a smoke
measurement, not a regression gate.

| Operation                         | Iterations | ns / op     | op / s        |
| --------------------------------- | ---------- | ----------- | ------------- |
| audit chain record + verify       | 5,000      | ~460        | ~2.2 M        |
| sealed store seal + unseal        | 5,000      | ~1,050      | ~950 k        |
| SHA-256 over a 32-byte public key | 5,000      | ~37         | ~27 M         |
| `fingerprint::from_public_key`    | 5,000      | ~41         | ~24 M         |
| pairing `validate_acceptance`     | 5,000      | <1          | >5 G          |
| device registry register + get    | 256        | ~500        | ~2.0 M        |
| IPC request encode + decode       | 5,000      | ~1,180      | ~850 k        |

## Versions of components Aether 0.2.0 is built against

- Rust: 1.82+ (matches the `rust-toolchain.toml`).
- `ed25519-dalek`: 2 / 3.
- `serde` / `serde_json`: 1.x.
- `aes-gcm`: 0.10+.
- `sha2`: 0.10.
- Linux kernel: 6.8.0-138-generic (the initramfs staging default).
- QEMU: 8.x with `virtio_gpu` and `virtio_net` device support.

## Compatibility tests run as part of `release-validate.sh`

1. Workspace debug build.
2. Workspace release build.
3. Full test suite.
4. `cargo clippy --workspace --all-targets`.
5. `cargo fmt --all -- --check`.
6. Release staging (`scripts/release.sh`).
7. Python unit tests.
8. Workspace manifest completeness.
9. Phase 15 documentation existence.
