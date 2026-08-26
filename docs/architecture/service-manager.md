# Service Manager

## Mission

The Service Manager owns the runtime lifecycle of Aether OS userspace services. It is intentionally independent from AI-specific systems so future AI, desktop, voice, vision, security, storage, and application services can be supervised through one consistent control plane.

## Service Manifest Contract

Service manifests use the `.aether-service` extension and a strict `key=value` format.

| Field | Required | Meaning |
| --- | --- | --- |
| `schema_version` | Yes | Manifest schema version. Current value is `1`. |
| `service_id` | Yes | Stable dotted identifier such as `aether.core`. |
| `name` | Yes | Human-readable service name. |
| `version` | Yes | Service version supplied by the package. |
| `description` | Yes | Operational description. |
| `service_type` | Yes | `internal` or `process`. |
| `command` | Required for `process` | Command executed for process services. |
| `dependencies` | Yes | Comma-separated service IDs that must run first. |
| `startup_priority` | Yes | Numeric tie-breaker for deterministic graph roots. |
| `restart_policy` | Yes | `never`, `on-failure`, or `always`. |
| `restart_limit` | Yes | Maximum restarts before the service is marked failed. |
| `restart_backoff_ms` | Yes | Base restart delay in milliseconds. |
| `health_check` | Yes | `none` or `file:<path>`. |
| `config_path` | Optional | Non-secret configuration location. |
| `security_identity` | Yes | Declared runtime identity boundary. |
| `ipc_endpoints` | Yes | Comma-separated declared IPC endpoints. |
| `capabilities` | Yes | Comma-separated declared service capabilities. |
| `resource_cpu_weight` | Yes | Declared CPU scheduling weight. |
| `resource_memory_max_kib` | Yes | Declared memory ceiling, where `0` means not enforced in this phase. |
| `resource_process_limit` | Yes | Declared process ceiling for future pids-controller enforcement. |
| `resource_io_weight` | Yes | Declared IO scheduling weight for future cgroup enforcement. |
| `requires_root` | Yes | Whether the service requests root-equivalent privilege. |
| `sandbox_profile` | Yes | `internal`, `system-service`, or `restricted-service`. |
| `permission_profile` | Yes | `system-internal`, `service-runtime`, or `developer-control`. |
| `ipc_access` | Yes | Current value must be `local-private`. |
| `shutdown_timeout_ms` | Yes | Maximum graceful stop wait before forced termination. |

## Dependency Resolution

```mermaid
flowchart LR
    IPC["aether.ipc"] --> Logging["aether.logging"]
    Logging --> Config["aether.config"]
    Config --> Core["aether.core"]
```

The manager validates every dependency before startup. Missing dependencies and circular dependencies abort startup before any managed process is launched. Startup order is dependency-first. Shutdown order is the reverse.

## Supervision Rules

| Condition | Behaviour |
| --- | --- |
| Process exits successfully | Service moves to `STOPPED` and emits `service.exited`. |
| Process exits unsuccessfully with restart budget | Service moves to `FAILED`, then `RECOVERING`, waits backoff, increments restart count, and starts again. |
| Process exits unsuccessfully without restart budget | Service moves to `FAILED` and emits `service.failed`. |
| Health file reports degraded or unhealthy | Runtime health changes and lifecycle becomes `DEGRADED` while the process remains alive. |
| Stop request targets a dependency with running dependents | Request fails with a clear dependency error. |

## Restart Backoff

Backoff is calculated from the manifest base delay and restart count, capped at 30 seconds. Restart counts are bounded by `restart_limit`, preventing endless restart loops.

## Status Output

`aetherctl services` returns pipe-delimited rows:

```text
service|lifecycle|health|pid|restarts|failures|uptime_ms
aether.core|RUNNING|HEALTHY|123|0|0|5000
```

The format is intentionally stable for development tooling. A structured API can be added later without removing this CLI contract.

## Limitations

Current resource and security fields are declarative only. Enforcement through cgroups, namespaces, seccomp, Linux capabilities, and MAC policies belongs to a later hardening phase.
