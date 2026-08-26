# Aether Storage Service

## Purpose

The storage portion of Phase 1.5 exposes mounted filesystem and capacity information without exposing unnecessary sensitive device identifiers. It gives the operating system a foundation for future removable media, user data scopes, storage pressure policy, and AI-safe file planning.

## Storage Model

| Scope | Meaning | Phase 1.5 Behavior |
| --- | --- | --- |
| Root filesystem | The base operating-system filesystem. | Readable through policy; sensitive paths remain restricted. |
| User data | Paths under `/home`. | Classified separately for future user-level policy. |
| Temporary filesystem | Paths under `/tmp` and `/run`. | Available for bounded runtime and benchmark operations. |
| Virtual filesystems | `proc`, `sysfs`, `devtmpfs`, `devpts`, cgroup, and tmpfs mounts. | Classified as virtual; write operations are denied where read-only or restricted. |
| Mounted storage | Paths under `/mnt` and `/media`. | Identified for future removable-storage workflows. |

## Storage Information

The service reads mounted filesystem information and reports:

| Field | Description |
| --- | --- |
| Mount point | Filesystem boundary visible to policy. |
| Filesystem type | Kernel-reported filesystem type. |
| Total capacity | Capacity in bytes when available. |
| Used capacity | Used bytes when available. |
| Available capacity | Available bytes when available. |
| Read-only state | Whether the mount is mounted read-only. |
| Device redaction | Indicates that raw device names are intentionally minimized. |
| Scope | Aether storage scope classification. |

## Mount Awareness

Phase 1.5 distinguishes mounted filesystem boundaries but does not mount, unmount, format, partition, encrypt, or repair storage devices. Those actions require additional policy, consent, and recovery workflows in later phases.

## Performance Baseline

The daemon includes a baseline benchmark mode for file write, file read, directory listing, stat, and search. The benchmark is used to establish measurable latency before optimization work begins.
