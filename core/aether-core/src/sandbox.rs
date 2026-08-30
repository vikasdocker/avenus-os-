// Aether Core - kernel sandboxing plan.
//
// Phase 11.4: defence-in-depth beyond the capability/policy gate.
//
// Every service declares a `SandboxProfile` in its manifest. The
// `plan_sandbox` function turns that profile into a *declarative*
// `SandboxPlan`: a typed description of the Linux kernel primitives
// the launcher is expected to apply before exec().
//
// This module is **declarative** on purpose:
//
//   * It does NOT call prctl(2), unshare(2), seccomp(2), or write
//     to cgroupfs. Those operations require CAP_SYS_ADMIN and must
//     run on the Aether OS image, not in this Rust crate. Tests
//     cannot exercise them from a normal user shell.
//
//   * The output `SandboxPlan` is a structured value the launcher
//     (a future `aether-sandbox` binary) consumes and enforces.
//     The plan is deterministic and unit-testable from any
//     platform.
//
//   * The plan is part of the service's audit record: every launch
//     must log the exact primitives applied. A real exploit chain
//     would have to bypass *both* the capability gate (Phase 11.3)
//     and the kernel sandbox (this phase).
//
// Profiles:
//
//   * `Internal`         - same-process services. No kernel sandbox
//                          is necessary; the runtime guarantees
//                          address isolation. (We still emit a
//                          zero-primitive plan for symmetry.)
//   * `SystemService`    - long-running OS daemons. A user namespace,
//                          a private cgroup slice, the ambient
//                          Linux capabilities the service actually
//                          needs, and a seccomp filter that allows
//                          only the syscall set a systemd-style
//                          service is expected to use.
//   * `RestrictedService`- user-facing apps. A user namespace, a
//                          dedicated cgroup slice with a tight
//                          memory cap, no_new_privs, a strict
//                          seccomp allow-list, and a capability
//                          whitelist (typically: nothing).

use crate::manifest::SandboxProfile;
use serde::{Deserialize, Serialize};

/// Linux capability whitelist. Mirrors the names in
/// `<linux/capability.h>` (no leading `CAP_`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LinuxCapability {
    Chown,
    DacOverride,
    DacReadSearch,
    Fowner,
    Fsetid,
    Kill,
    Setgid,
    Setuid,
    Setpcap,
    LinuxImmutable,
    NetBindService,
    NetBroadcast,
    NetAdmin,
    NetRaw,
    IpcLock,
    IpcOwner,
    SysModule,
    SysRawio,
    SysChroot,
    SysPtrace,
    SysPacct,
    SysAdmin,
    SysBoot,
    SysNice,
    SysResource,
    SysTime,
    SysTtyConfig,
    Mknod,
    Lease,
    AuditWrite,
    AuditControl,
    Setfcap,
    MacOverride,
    MacAdmin,
    Syslog,
    WakeAlarm,
    BlockSuspend,
    AuditRead,
    Perfmon,
    Bpf,
    CheckpointRestore,
}

impl LinuxCapability {
    /// Canonical wire name (no `CAP_` prefix).
    pub fn name(self) -> &'static str {
        match self {
            Self::Chown => "chown",
            Self::DacOverride => "dac_override",
            Self::DacReadSearch => "dac_read_search",
            Self::Fowner => "fowner",
            Self::Fsetid => "fsetid",
            Self::Kill => "kill",
            Self::Setgid => "setgid",
            Self::Setuid => "setuid",
            Self::Setpcap => "setpcap",
            Self::LinuxImmutable => "linux_immutable",
            Self::NetBindService => "net_bind_service",
            Self::NetBroadcast => "net_broadcast",
            Self::NetAdmin => "net_admin",
            Self::NetRaw => "net_raw",
            Self::IpcLock => "ipc_lock",
            Self::IpcOwner => "ipc_owner",
            Self::SysModule => "sys_module",
            Self::SysRawio => "sys_rawio",
            Self::SysChroot => "sys_chroot",
            Self::SysPtrace => "sys_ptrace",
            Self::SysPacct => "sys_pacct",
            Self::SysAdmin => "sys_admin",
            Self::SysBoot => "sys_boot",
            Self::SysNice => "sys_nice",
            Self::SysResource => "sys_resource",
            Self::SysTime => "sys_time",
            Self::SysTtyConfig => "sys_tty_config",
            Self::Mknod => "mknod",
            Self::Lease => "lease",
            Self::AuditWrite => "audit_write",
            Self::AuditControl => "audit_control",
            Self::Setfcap => "setfcap",
            Self::MacOverride => "mac_override",
            Self::MacAdmin => "mac_admin",
            Self::Syslog => "syslog",
            Self::WakeAlarm => "wake_alarm",
            Self::BlockSuspend => "block_suspend",
            Self::AuditRead => "audit_read",
            Self::Perfmon => "perfmon",
            Self::Bpf => "bpf",
            Self::CheckpointRestore => "checkpoint_restore",
        }
    }
}

/// Linux namespaces the sandbox can request. Mirrors the `CLONE_*`
/// flags (without the `CLONE_` prefix).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LinuxNamespace {
    Mount,
    Pid,
    Network,
    Ipc,
    Uts,
    User,
    Cgroup,
}

impl LinuxNamespace {
    pub fn name(self) -> &'static str {
        match self {
            Self::Mount => "mount",
            Self::Pid => "pid",
            Self::Network => "network",
            Self::Ipc => "ipc",
            Self::Uts => "uts",
            Self::User => "user",
            Self::Cgroup => "cgroup",
        }
    }
}

/// Seccomp filter identifier. The launcher maps each tag to a
/// real BPF / libseccomp filter. The tag is opaque from this
/// crate's perspective; it just needs to be unique per filter.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SeccompFilterTag(pub String);

impl SeccompFilterTag {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SeccompFilterTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Resource limits that translate directly into a cgroup v2 write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// cgroup v2 slice name. Always rooted under `aether.slice/`.
    pub cgroup_slice: String,
    /// `cpu.weight`; 1..10000. 100 = default.
    pub cpu_weight: u32,
    /// `memory.max` in bytes (None = inherit parent).
    pub memory_max_bytes: Option<u64>,
    /// `pids.max`; None = inherit parent.
    pub pids_max: Option<u32>,
    /// `io.weight`; 1..10000. 100 = default.
    pub io_weight: u32,
}

impl ResourceLimits {
    /// Minimal but realistic defaults for a system service.
    pub fn system_default() -> Self {
        Self {
            cgroup_slice: "aether.slice/system.service.slice".to_string(),
            cpu_weight: 100,
            memory_max_bytes: None,
            pids_max: Some(256),
            io_weight: 100,
        }
    }
    /// Tighter defaults for a user-facing restricted app.
    pub fn restricted_default() -> Self {
        Self {
            cgroup_slice: "aether.slice/restricted.app.slice".to_string(),
            cpu_weight: 50,
            memory_max_bytes: Some(512 * 1024 * 1024),
            pids_max: Some(64),
            io_weight: 50,
        }
    }
}

/// Declarative kernel sandbox plan. The launcher applies these
/// primitives in the order they're listed; every successful apply
/// is logged. The plan is deterministic and snapshot-friendly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxPlan {
    pub profile: SandboxProfile,
    pub namespaces: Vec<LinuxNamespace>,
    /// Linux capabilities to KEEP in the ambient set. Every other
    /// capability is dropped before exec().
    pub capabilities: Vec<LinuxCapability>,
    /// True if `PR_SET_NO_NEW_PRIVS` must be set. Strongly
    /// recommended for all sandboxed services.
    pub no_new_privs: bool,
    /// Seccomp filter to install (if any). The launcher is
    /// responsible for translating the tag into a real BPF
    /// program.
    pub seccomp: Option<SeccompFilterTag>,
    /// Resource limits. Always present (the cgroup slice exists
    /// even when the limits are otherwise inherited).
    pub resources: ResourceLimits,
    /// Human-readable description of the rationale, surfaced in
    /// audit logs.
    pub rationale: String,
}

/// Build the plan for a given profile. The function is total
/// (every profile maps to a valid plan) and pure (no I/O, no
/// system calls).
pub fn plan_sandbox(profile: SandboxProfile) -> SandboxPlan {
    match profile {
        SandboxProfile::Internal => SandboxPlan {
            profile,
            namespaces: Vec::new(),
            capabilities: Vec::new(),
            no_new_privs: false,
            seccomp: None,
            resources: ResourceLimits {
                cgroup_slice: "aether.slice/internal.slice".to_string(),
                cpu_weight: 100,
                memory_max_bytes: None,
                pids_max: None,
                io_weight: 100,
            },
            rationale:
                "internal services run inside the system-core address space; no kernel sandbox is applied"
                    .to_string(),
        },
        SandboxProfile::SystemService => SandboxPlan {
            profile,
            namespaces: vec![LinuxNamespace::User, LinuxNamespace::Mount, LinuxNamespace::Uts],
            capabilities: vec![
                LinuxCapability::Chown,
                LinuxCapability::Fowner,
                LinuxCapability::Fsetid,
                LinuxCapability::Setgid,
                LinuxCapability::Setuid,
                LinuxCapability::Setpcap,
                LinuxCapability::NetBindService,
                LinuxCapability::Kill,
                LinuxCapability::SysTime,
                LinuxCapability::SysResource,
                LinuxCapability::AuditWrite,
            ],
            no_new_privs: true,
            seccomp: Some(SeccompFilterTag::new("system-service-v1")),
            resources: ResourceLimits::system_default(),
            rationale: "system services get a user namespace, the Linux capabilities they actually need, a seccomp allow-list, and no_new_privs so setuid binaries cannot escalate"
                .to_string(),
        },
        SandboxProfile::RestrictedService => SandboxPlan {
            profile,
            namespaces: vec![
                LinuxNamespace::User,
                LinuxNamespace::Mount,
                LinuxNamespace::Pid,
                LinuxNamespace::Network,
                LinuxNamespace::Ipc,
                LinuxNamespace::Uts,
            ],
            capabilities: Vec::new(),
            no_new_privs: true,
            seccomp: Some(SeccompFilterTag::new("restricted-app-v1")),
            resources: ResourceLimits::restricted_default(),
            rationale: "user-facing apps run in their own user+pid+network namespace, drop every ambient capability, and are constrained to a strict seccomp allow-list"
                .to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_profile_has_no_kernel_primitives() {
        let p = plan_sandbox(SandboxProfile::Internal);
        assert!(p.namespaces.is_empty());
        assert!(p.capabilities.is_empty());
        assert_eq!(p.seccomp, None);
        assert!(!p.no_new_privs);
        assert!(p.rationale.contains("internal"));
    }

    #[test]
    fn system_service_has_namespaces_and_minimal_caps() {
        let p = plan_sandbox(SandboxProfile::SystemService);
        assert!(p.namespaces.contains(&LinuxNamespace::User));
        assert!(p.namespaces.contains(&LinuxNamespace::Mount));
        // MUST NOT carry sys_admin / sys_module / sys_rawio.
        assert!(!p.capabilities.contains(&LinuxCapability::SysAdmin));
        assert!(!p.capabilities.contains(&LinuxCapability::SysModule));
        assert!(!p.capabilities.contains(&LinuxCapability::SysRawio));
        assert!(p.no_new_privs);
        assert!(p.seccomp.is_some());
    }

    #[test]
    fn restricted_app_drops_every_capability() {
        let p = plan_sandbox(SandboxProfile::RestrictedService);
        assert!(p.capabilities.is_empty(), "restricted app must drop every cap");
        assert!(p.namespaces.contains(&LinuxNamespace::Pid));
        assert!(p.namespaces.contains(&LinuxNamespace::Network));
        assert!(p.no_new_privs);
        // Memory cap is bounded so a runaway app cannot exhaust the system.
        assert!(p.resources.memory_max_bytes.is_some());
    }

    #[test]
    fn resource_limits_default_to_different_slices() {
        let s = plan_sandbox(SandboxProfile::SystemService).resources;
        let r = plan_sandbox(SandboxProfile::RestrictedService).resources;
        assert!(s.cgroup_slice.starts_with("aether.slice/"));
        assert!(r.cgroup_slice.starts_with("aether.slice/"));
        assert_ne!(s.cgroup_slice, r.cgroup_slice, "must not share a cgroup slice");
    }

    #[test]
    fn seccomp_tags_are_distinct_per_profile() {
        let s = plan_sandbox(SandboxProfile::SystemService)
            .seccomp
            .unwrap_or_else(|| panic!("system service should have a seccomp tag"));
        let r = plan_sandbox(SandboxProfile::RestrictedService)
            .seccomp
            .unwrap_or_else(|| panic!("restricted service should have a seccomp tag"));
        assert_ne!(s, r, "system and restricted profiles must use different seccomp filters");
    }

    #[test]
    fn plan_is_deterministic() {
        let a = plan_sandbox(SandboxProfile::SystemService);
        let b = plan_sandbox(SandboxProfile::SystemService);
        assert_eq!(a, b);
    }

    #[test]
    fn plan_round_trips_through_serde_json() {
        let original = plan_sandbox(SandboxProfile::SystemService);
        let text = serde_json::to_string(&original).unwrap_or_else(|e| panic!("{e}"));
        let back: SandboxPlan = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(original, back);
    }

    #[test]
    fn capability_names_are_canonical() {
        // Stable wire names; the launcher parses them as CAP_<NAME>.
        assert_eq!(LinuxCapability::NetBindService.name(), "net_bind_service");
        assert_eq!(LinuxCapability::SysAdmin.name(), "sys_admin");
        assert_eq!(LinuxCapability::AuditWrite.name(), "audit_write");
    }

    #[test]
    fn namespace_names_match_clone_flag_names() {
        // Names match the suffix of CLONE_NEW<NAME> in the kernel.
        assert_eq!(LinuxNamespace::User.name(), "user");
        assert_eq!(LinuxNamespace::Mount.name(), "mount");
        assert_eq!(LinuxNamespace::Pid.name(), "pid");
        assert_eq!(LinuxNamespace::Network.name(), "network");
        assert_eq!(LinuxNamespace::Ipc.name(), "ipc");
        assert_eq!(LinuxNamespace::Uts.name(), "uts");
        assert_eq!(LinuxNamespace::Cgroup.name(), "cgroup");
    }
}
