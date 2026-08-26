# Filesystem Security

## Security Goal

The filesystem service prevents AI agents and tools from gaining unrestricted filesystem access. Every public request must enter through System Core IPC, pass capability evaluation, be audited, and then pass daemon-side path validation before touching the Linux filesystem.

## Threats Addressed

| Threat | Phase 1.5 Control |
| --- | --- |
| Path traversal | Relative paths containing parent directory traversal are rejected. |
| Absolute-path abuse | Public filesystem IPC accepts only scope-relative paths. |
| Symlink escape | Existing paths are resolved and must remain inside the configured scope. |
| Null-byte injection | Paths containing null bytes are rejected. |
| Restricted system paths | Sensitive paths such as Aether IPC sockets, audit logs, root home, and credential files are policy-denied. |
| Unauthorized mount access | Mounts are classified and sensitive device fields are minimized. |
| Unbounded recursion | Recursive directory operations have depth and file-count limits. |
| Destructive operation abuse | Delete capability is critical risk and recursive deletion is explicit. |
| Sensitive logging | Audit records contain actor, operation, target, decision, reason, and correlation ID; file contents are not logged. |

## Authorization

System Core maps filesystem requests to named capabilities. Filesystem operations require a local private IPC peer. The daemon is supervised as `aether.filesystem` and has a private socket at `/run/aether/ipc/aether-filesystemd.sock`.

## Defense In Depth

Authorization is centralized in System Core, while `aether-filesystemd` independently enforces path safety and resource bounds. This prevents direct daemon misuse from bypassing path security and prevents callers from relying on daemon privileges as a substitute for capability policy.

## Audit Behavior

System Core records all filesystem IPC requests. High-risk operations such as write, rename, move, and delete are auditable because they pass through the same request handler as service and system control commands. The service does not log file contents.

## Current Limits

Phase 1.5 does not provide per-user discretionary access-control policy, content classification, cryptographic file labeling, semantic search, or race-free kernel-mediated path handles. Those controls require future phases that extend the identity, sandbox, and storage policy layers.
