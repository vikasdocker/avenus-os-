# Developing Aether Services

## Service Identity

Every service must have a stable dotted identifier using lowercase ASCII letters, digits, hyphens, and dots. Examples:

| Valid | Invalid |
| --- | --- |
| `aether.core` | `Aether.Core` |
| `aether.browser-agent` | `aether_browser_agent` |

## Creating a Manifest

Create a `.aether-service` file in `system/services.d` for source-level development and in the Buildroot rootfs overlay when the service must appear in the boot image.

Minimum process service manifest:

```text
schema_version=1
service_id=aether.worker
name=Aether Worker
version=0.1.0
description=Runs the worker process.
service_type=process
command=/usr/sbin/aether-worker
dependencies=aether.config
startup_priority=50
restart_policy=on-failure
restart_limit=3
restart_backoff_ms=500
health_check=file:/run/aether/health/aether-worker.json
config_path=/etc/aether/aether-worker.conf
security_identity=aether-worker
requires_root=false
sandbox_profile=system-service
permission_profile=service-runtime
ipc_access=local-private
ipc_endpoints=unix:/run/aether/ipc/aether-worker.sock
capabilities=health,ipc
resource_cpu_weight=100
resource_memory_max_kib=0
resource_process_limit=32
resource_io_weight=100
shutdown_timeout_ms=5000
```

## Validation

Run host-level validation:

```bash
bash scripts/test.sh
```

When Cargo is available, run Rust tests:

```bash
cargo test --workspace
```

Validate manifests without starting services:

```bash
aether-system-core --check --manifest-dir system/services.d
```

## Runtime Inspection

After boot or after starting `aether-system-core` locally:

```bash
aetherctl services
aetherctl service status aether.core
aetherctl health
aetherctl system status
aetherctl system metrics
aetherctl system audit
```

## Benchmarking

Measure the live IPC control path:

```bash
python tools/bench-system-core.py --iterations 100
```

The benchmark emits JSON. If `aetherctl` or the running socket is unavailable, it reports the missing runtime condition instead of inventing measurements.

## Service Design Rules

| Rule | Requirement |
| --- | --- |
| Startup | A service must not assume dependencies are available unless declared in its manifest. |
| Health | A process service should publish a health file or a future health endpoint. |
| Shutdown | A process service should handle termination and flush state before its `shutdown_timeout_ms`. |
| Configuration | Normal config files must not contain passwords, tokens, private keys, or credentials. |
| Logging | Logs must be structured and must avoid sensitive user data. |
| IPC | Tools must communicate through Aether IPC instead of editing manager internals or runtime files directly. |
