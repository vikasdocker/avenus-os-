# Repository Standards

## Layout Standards

- `kernel/` owns Linux kernel configuration and build helpers.
- `core/` owns shared domain contracts.
- `services/` owns long-running system services.
- `system/` owns PID1, low-level utilities, and boot service descriptors.
- `brain/` owns Python AI orchestration and deterministic local brain behavior.
- `voice/`, `vision/`, `security/`, `storage/`, `network/`, `desktop/`, and `apps/`
  own domain contracts and future implementation boundaries.
- `sdk/` owns developer-facing libraries.
- `scripts/` owns reproducible automation.
- `infra/` owns non-image operational configuration.
- `docs/` owns requirements and engineering guides.

## Build Standards

- Cargo owns Rust crates.
- CMake owns C, C++, and Qt/QML build targets.
- Python uses standard project metadata in `pyproject.toml`.
- Shell scripts must use `set -euo pipefail`.
- Docker is the canonical cross-host build environment.

## Quality Standards

- No unchecked public behavior.
- No unbounded background work.
- No silent failure in boot, update, security, storage, or AI-control paths.
- No code without tests for behavior it owns.
- No generated release artifact committed to source control.
- No secrets, credentials, private keys, or user data in the repository.

## Review Standards

Every pull request must describe:

- Behavior changed
- Requirement IDs affected
- Tests run
- Security and privacy impact
- Performance and resource impact
- Rollback or recovery behavior

