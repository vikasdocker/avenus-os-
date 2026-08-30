# Changelog

All notable changes to Aether OS are recorded in this file.

## 0.2.1 - 2026-08-30

### Added

- **Bootable ISO pipeline** — `scripts/iso/build-iso.sh`
  assembles a hybrid ISO (`build/aether-os-<version>.iso`)
  using `grub-mkrescue` and `xorriso`. The ISO is bootable
  from optical media and from a USB stick via `dd` /
  Ventoy / Rufus-DD. The script builds the initramfs if
  it is missing, stages the kernel + initramfs into an
  isolinux tree, and writes a GRUB config with three
  menu entries (default boot, verbose boot, recovery
  shell). It is Linux-only by design: it requires
  `xorriso` and `grub-mkrescue` from
  `grub-pc-bin` / `grub-efi-amd64-bin` /
  `grub-common`.
- **QEMU-from-ISO runner** — `scripts/run/qemu-iso.sh`
  boots the freshly-built ISO under QEMU. It picks
  the most recent `build/aether-os-*.iso` if
  `AETHER_ISO` is not set, and supports the same
  `--smoke` headless gate the kernel+initramfs runner
  uses.
- **ISO assembly step in `release-validate.sh`** —
  A tenth gate that runs the ISO builder on Linux
  runners and skips it on Windows runners (and with
  `--skip-iso`). A pure-bash test confirms the new
  scripts exist, are executable on POSIX, carry the
  expected shebang, and expose the documented CLI.
- **Python release-script contract tests** —
  `tests/python/test_release_scripts.py` (19 tests)
  verifies the release pipeline scripts and the
  bootable-ISO contract.
- **Compatibility matrix refresh** —
  `docs/phase-15/compatibility-matrix.md` updated to
  the actual `aether-bench` numbers (audit chain
  ~2.2 M op/s, sealed store ~950 k op/s, SHA-256
  ~27 M op/s, fingerprint helper ~24 M op/s, pairing
  validate >5 G op/s, device registry ~2 M op/s, IPC
  ~850 k op/s). Removed the obsolete
  `clippy -D warnings` reference.
- **Workspace lint cleanup** — Replaced an unused
  test helper parameter in `aether-security::audit`
  with `_timestamp`, removed a never-read
  intermediate `Vec` in the tampered-chain test, and
  silenced a stale `let resp = ...` in a system-core
  duplicate-register test so the workspace is fully
  warning-clean.

### Verified

- 842 Rust tests passing, 0 failing.
- 25 Python tests passing, 0 failing.
- 0 clippy warnings across the workspace.
- `release-validate.sh` reports 10/10 (ISO step
  skips cleanly on Windows runners).

## 0.2.0 - 2026-08-30

### Added

- **Cross-device contract (`aether-device-core`)** — Device identity
  (`DeviceId`, `DeviceClass`, `DeviceFingerprint`), `DeviceRegistry`
  with a 256-entry bounded active set, typed pairing handshake
  (`PairingRequest` / `PairingAcceptance` / `PairingCode`), capability
  gating (`PairingGrant`), and `RemoteObservation` / `RemoteProposal`
  cross-device message envelopes. 60 unit tests.
- **System-core device IPC** — Six new commands on the TCP control
  plane: `device.list`, `device.register`, `device.pair.begin`,
  `device.pair.complete`, `device.revoke`, `device.unregister`. 8
  integration tests.
- **Micro-benchmark harness (`aether-bench`)** — Six benches covering
  audit chain throughput, sealed-store seal/unseal, SHA-256
  fingerprinting, pairing validation, device registry, and IPC
  request encode/decode. Run with `cargo run --release --bin
  aether-bench`.
- **Release validation script (`scripts/release-validate.sh`)** —
  9-step CI-friendly gate: debug build, release build, full test
  suite, clippy, rustfmt, release staging, Python tests, workspace
  manifest completeness, and Phase 15 documentation existence.
- **GitHub Actions CI (`.github/workflows/ci.yml`)** — Matrix on
  `ubuntu-latest` and `windows-latest`. Runs the build, tests,
  clippy, rustfmt, Python tests, repository contract tests,
  ShellCheck, and markdownlint on every push and pull request.
- **Phase 15 documentation** — `docs/RELEASE-NOTES.md` (0.2.0
  highlights, compatibility, known limitations, upgrade path),
  `docs/phase-15/compatibility-matrix.md` (Tier 1 reference,
  Tier 2 best-effort, Tier 3 future hardware), and
  `docs/phase-15/security-audit.md` (10-section security review
  of cryptographic primitives, key handling, capability policy,
  audit chain, IPC transport, cross-device security, supply
  chain, update mechanism, known limitations, audit sign-off).

### Verified

- 842 Rust tests passing, 0 failing.
- 0 clippy errors across the workspace.
- Release build clean (10 binaries).
- `aether-bench` runs end-to-end with reproducible numbers.
- `release-validate.sh` reports `9/9 passed`.

## Unreleased

### Fixed

- Repaired `libaether-graphics`: created the six missing modules (display, renderer,
  input, window, cursor, output), removed duplicate `PixelFormat`, replaced the
  unavailable `Display` derive with manual impls, fixed the `AetherError` conversion
  to use `ErrorKind`, and enabled the uuid `serde` feature.
- Removed a stray junk file from the repository root.

### Added

- Implemented `system/aether-init`: PID1 boot-stage machine, kernel-parameter
  parsing, early mounts, loopback bring-up, console session, and shutdown plan.
- Implemented `system/aether-system-core`: manifest loader, dependency graph with
  cycle/missing detection, service lifecycle manager with restart policies, and a
  TCP JSON control plane compatible with `aetherctl`.
- Implemented `system/aether-application-manager`: application registry with
  single-instance launch policy and REPL daemon.
- Implemented `services/aether-supervisor`: restart policies with exponential
  backoff and supervision daemon binary.
- Implemented `services/aether-agentd`: AI control-plane agent with bounded event
  ring, task state, and ndjson request loop.
- Implemented `sdk/rust/aether-sdk` TCP client and `sdk/python/aether_sdk`
  wire-protocol package (`AETHER/1`).
- Implemented `tools/aetherctl` control CLI.
- Added service manifests under `system/services.d/` and host demo script
  `scripts/demo.sh`.

### Changed

- Wired all new crates into the Cargo workspace with strict shared lints.
- Recreated `scripts/build.sh`, `test.sh`, `lint.sh`, `format.sh`, `clean.sh`,
  `release.sh`, initramfs assembly, and QEMU run/drive/smoke tooling.

### Verified

- Full workspace: clippy clean, 66 Rust tests passing; Python brain/SDK: 6 tests.
- Live control-plane demo on the host and full QEMU boot: PID1 stages, dependency-
  ordered service startup, in-VM `aetherctl status/restart/shutdown`, clean poweroff.

## 0.1.0 - 2026-08-10

### Added

- Created the Project Genesis repository foundation for Aether OS.
- Added Rust workspace with PID1, service daemons, SDK primitives, and domain crates.
- Added CMake native build for C system utilities.
- Added Python AI brain package and Python SDK.
- Added Qt6/QML shell source.
- Added Linux kernel configuration seed and kernel build helper.
- Added initramfs and development ISO creation pipeline.
- Added QEMU, VirtualBox, and VMware launchers.
- Added Docker development environment and devcontainer configuration.
- Added CI, lint, build, release, issue, and pull request automation.
- Added developer documentation, contribution policy, security policy, and repository standards.

