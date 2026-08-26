# Changelog

All notable changes to Aether OS are recorded in this file.

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

