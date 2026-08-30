# Aether OS 0.2.0 — Release Notes

*Release date: 2026-08-30*

Aether OS 0.2.0 is the first end-to-end production-quality cut of the
system. It builds on the 0.1.0 foundation (PID1, service manager, IPC
control plane, agent daemon, SDK, aetherctl) and layers on a typed
multi-device contract, a tamper-evident audit chain, a sealed credential
store, an integrity-checked update mechanism, and a regression-grade
test suite.

## Highlights

- **Typed cross-device contract** (`aether-device-core`). Every remote
  observation and remote proposal that a paired peer can push into the
  local agent is a typed, validated record. Pairing is an explicit
  handshake; the state machine lives in the registry and the shell
  exposes it over IPC.
- **Autonomous planning surface** (`aether-agent-core` +
  `aether-system-core`). The agent can hold observations, generate
  proposals with typed risk levels, and convert approved proposals into
  tasks. The shell validates and dispatches everything; the user
  always sees the proposal before it becomes a task.
- **Tamper-evident audit chain**. Every privileged action is recorded
  into a hash-linked log (`prev_hash` → `content_hash` SHA-256).
  `verify_chain` recomputes the chain and rejects any tampering.
- **Sealed credential store** (`aether-security::credentials`). Every
  secret is sealed with AES-256-GCM and the key never leaves the
  process; a poisoned mutex does not panic the dispatcher; the
  plaintext is held in a `Secret<String>` that zeroizes on drop.
- **Integrity-checked update mechanism** (`aether-update-core`). The
  update system signs manifest upgrades with Ed25519, enforces a
  version policy (no silent downgrades by default), and supports
  recovery from a failed transition.
- **Release validation harness**. `scripts/release-validate.sh` runs
  every check that a tagged release must pass: debug build, release
  build, full test suite, clippy with `-D warnings`, rustfmt, release
  staging, Python unit tests, and the workspace manifest
  completeness gate.
- **CI on every push and pull request** via
  `.github/workflows/ci.yml`. Ubuntu + Windows, debug + release builds,
  clippy, rustfmt, Python tests, repository contract tests, and
  ShellCheck on every shell script.

## Test results

- **842 Rust tests** across 25 crates, all passing.
- **Clippy**: 0 errors with `-D warnings`.
- **rustfmt**: no diffs reported.
- **Micro-benchmarks** (`aether-bench`): recorded in
  `docs/phase-15/compatibility-matrix.md` so a future run can be
  diffed against the same machine.

## Compatibility

See `docs/phase-15/compatibility-matrix.md` for the full target matrix
and `docs/phase-15/security-audit.md` for the production security
review.

## Known limitations

These are explicitly out of scope for 0.2.0 and are listed in the
roadmap as follow-on work:

- The cross-device *transport* (BLE / QR / NFC) is the future
  `aether-device-runtime`'s job. The 0.2.0 shell stores the typed
  state machine so other subsystems can compose against it, but does
  not move bytes.
- The native DRM/KMS graphics backend (Phase 1.9 / Phase 6) is
  deferred; the QEMU `virtio_gpu` path is the reference platform.
- The `aether-sandbox` binary (Phase 11.4 enforcement) is Linux-only
  and is staged but not yet emitted by the initramfs build.
- Privacy-safe telemetry is designed but not enabled by default.

## Upgrade path

0.1.0 → 0.2.0 is a clean drop-in: the IPC protocol, manifest format,
and service IDs are unchanged. 0.1.0 initramfs will boot 0.2.0 binaries
and vice versa; the only visible difference is the larger IPC surface
(`agent.*` and `device.*` commands).
