# Filesystem Development Guide

## Components

| Component | Path | Responsibility |
| --- | --- | --- |
| Storage crate | `storage/aether-storage` | Filesystem domain model, path policy, operations, storage information, and daemon binary. |
| System Core IPC | `system/aether-system-core/src/ipc.rs` | Parses `fs ...` requests and forwards authorized requests to the daemon. |
| Permission manager | `system/aether-system-core/src/permission.rs` | Maps filesystem requests to capability and risk decisions. |
| CLI | `tools/aetherctl` | Provides operator commands that use System Core IPC. |
| Service manifest | `system/services.d/aether-filesystem.aether-service` | Registers the filesystem daemon with the service manager. |

## Local Commands

From a running Aether image:

```sh
aetherctl fs capabilities
aetherctl fs health
aetherctl fs list tmp
aetherctl fs stat tmp
aetherctl fs search tmp aether
aetherctl fs storage
aetherctl fs mounts
```

Destructive commands are explicit:

```sh
aetherctl fs delete tmp/example.txt
aetherctl fs delete-recursive tmp/example-dir
```

## Validation

Phase 1.5 validation includes:

| Command | Purpose |
| --- | --- |
| `python3 tools/aether-doctor.py` | Verify development environment tools. |
| `cargo test --workspace` | Run Rust unit and integration tests, including storage security tests. |
| `bash scripts/test.sh` | Run the complete repository regression suite. |
| `bash scripts/lint.sh` | Run formatting, Clippy, shell, and repository policy checks. |
| `bash scripts/build.sh` | Build the native workspace. |
| `bash scripts/build/build.sh` | Build the Buildroot image. |
| `bash scripts/test-boot.sh` | Boot the image and verify runtime filesystem service behavior. |

## Benchmarking

The storage daemon exposes a benchmark mode:

```sh
aether-filesystemd --scope-root / --benchmark
```

The benchmark reports operation, latency in microseconds, and bytes. It measures baseline write, read, directory list, stat, and search behavior.
