//! Aether sandbox policy — the typed
//! audit and runtime-check layer for
//! the declarative `SandboxPlan`.
//!
//! Phase 1.4 of the ROADMAP. The
//! underlying plan lives in
//! `aether_core::sandbox`; this crate
//! adds:
//!
//!   * a per-launch **audit record**
//!     of what was actually applied
//!     (`SandboxAudit`),
//!   * a `diff_plan_vs_audit` that
//!     flags any plan field that was
//!     requested but not satisfied
//!     (or vice versa),
//!   * a `SandboxEnforcer` trait the
//!     runtime uses to plug in a real
//!     backend (the existing
//!     `aether-sandbox` binary, or a
//!     future BPF-based enforcer).
//!
//! The model has five pieces:
//!
//! 1. **`PrimitiveStatus`** — was a
//!    primitive applied, skipped, or
//!    failed?
//! 2. **`SandboxAudit`** — a record
//!    of every primitive the
//!    enforcer touched.
//! 3. **`SandboxDiff`** — the
//!    requested vs applied view.
//! 4. **`SandboxEnforcer`** — the
//!    trait.
//! 5. **`NullEnforcer`** — the no-op
//!    fallback used in tests and on
//!    non-Linux hosts.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use aether_core::manifest::SandboxProfile;
use aether_core::sandbox::SandboxPlan;

/// A single, trackable primitive the
/// enforcer can apply.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SandboxPrimitive {
    /// `prctl(PR_SET_NO_NEW_PRIVS, 1)`.
    NoNewPrivs,
    /// Enter a user namespace.
    UserNamespace,
    /// Enter a mount namespace.
    MountNamespace,
    /// Enter a network namespace.
    NetworkNamespace,
    /// Enter a PID namespace.
    PidNamespace,
    /// Enter an IPC namespace.
    IpcNamespace,
    /// Drop every capability not in
    /// the plan's whitelist.
    DropCapabilities,
    /// Install a seccomp filter.
    InstallSeccomp,
    /// Create the cgroup slice.
    CreateCgroupSlice,
    /// Write the cgroup controllers'
    /// limits (cpu, memory, io).
    WriteCgroupLimits,
    /// Final `execvp` of the child.
    ExecChild,
}

impl SandboxPrimitive {
    /// The kebab-case name.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::NoNewPrivs => "no-new-privs",
            Self::UserNamespace => "user-namespace",
            Self::MountNamespace => "mount-namespace",
            Self::NetworkNamespace => "network-namespace",
            Self::PidNamespace => "pid-namespace",
            Self::IpcNamespace => "ipc-namespace",
            Self::DropCapabilities => "drop-capabilities",
            Self::InstallSeccomp => "install-seccomp",
            Self::CreateCgroupSlice => "create-cgroup-slice",
            Self::WriteCgroupLimits => "write-cgroup-limits",
            Self::ExecChild => "exec-child",
        }
    }
}

/// The status of a single primitive
/// during enforcement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveStatus {
    /// The primitive was applied
    /// successfully.
    Applied,
    /// The enforcer chose to skip
    /// the primitive (e.g. not
    /// supported on this kernel).
    Skipped,
    /// The primitive failed to
    /// apply.
    Failed,
}

impl PrimitiveStatus {
    /// The kebab-case name.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
        }
    }
}

/// A single audit row: one primitive
/// and its observed status.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PrimitiveAudit {
    /// The primitive.
    pub primitive: SandboxPrimitive,
    /// What happened.
    pub status: PrimitiveStatus,
    /// The kernel return code (or 0
    /// for success / no return).
    pub return_code: i32,
    /// A free-form reason (failure
    /// detail, skip reason, etc.).
    pub reason: String,
}

impl PrimitiveAudit {
    /// A new audit row.
    #[must_use]
    pub fn new(
        primitive: SandboxPrimitive,
        status: PrimitiveStatus,
        return_code: i32,
    ) -> Self {
        Self {
            primitive,
            status,
            return_code,
            reason: String::new(),
        }
    }

    /// Set the reason.
    #[must_use]
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = reason.into();
        self
    }

    /// `true` if the primitive was
    /// applied.
    #[must_use]
    pub const fn is_applied(&self) -> bool {
        matches!(self.status, PrimitiveStatus::Applied)
    }

    /// `true` if the primitive was
    /// skipped.
    #[must_use]
    pub const fn is_skipped(&self) -> bool {
        matches!(self.status, PrimitiveStatus::Skipped)
    }

    /// `true` if the primitive
    /// failed.
    #[must_use]
    pub const fn is_failed(&self) -> bool {
        matches!(self.status, PrimitiveStatus::Failed)
    }
}

/// The list of primitives a plan
/// requests.
#[must_use]
pub fn primitives_for(plan: &SandboxPlan) -> Vec<SandboxPrimitive> {
    let mut out = Vec::new();
    if plan.no_new_privs {
        out.push(SandboxPrimitive::NoNewPrivs);
    }
    use aether_core::sandbox::LinuxNamespace;
    for ns in &plan.namespaces {
        match ns {
            LinuxNamespace::User => out.push(SandboxPrimitive::UserNamespace),
            LinuxNamespace::Mount => out.push(SandboxPrimitive::MountNamespace),
            LinuxNamespace::Network => out.push(SandboxPrimitive::NetworkNamespace),
            LinuxNamespace::Pid => out.push(SandboxPrimitive::PidNamespace),
            LinuxNamespace::Ipc => out.push(SandboxPrimitive::IpcNamespace),
            _ => {}
        }
    }
    if !plan.capabilities.is_empty() {
        out.push(SandboxPrimitive::DropCapabilities);
    }
    if plan.seccomp.is_some() {
        out.push(SandboxPrimitive::InstallSeccomp);
    }
    out.push(SandboxPrimitive::CreateCgroupSlice);
    out.push(SandboxPrimitive::WriteCgroupLimits);
    out.push(SandboxPrimitive::ExecChild);
    out
}

/// A complete audit for one launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxAudit {
    /// The plan that was supposed to
    /// be applied.
    pub plan: SandboxPlan,
    /// The audit rows, one per
    /// primitive.
    pub rows: Vec<PrimitiveAudit>,
    /// The launch timestamp (ms
    /// since epoch; the caller
    /// supplies the clock).
    pub timestamp_ms: u64,
    /// The service id that was
    /// launched.
    pub service: String,
}

impl SandboxAudit {
    /// A new audit.
    #[must_use]
    pub fn new(plan: SandboxPlan, service: impl Into<String>, timestamp_ms: u64) -> Self {
        Self {
            plan,
            rows: Vec::new(),
            timestamp_ms,
            service: service.into(),
        }
    }

    /// Add a row.
    pub fn record(&mut self, row: PrimitiveAudit) {
        self.rows.push(row);
    }

    /// The number of applied rows.
    #[must_use]
    pub fn applied_count(&self) -> usize {
        self.rows.iter().filter(|r| r.is_applied()).count()
    }

    /// The number of skipped rows.
    #[must_use]
    pub fn skipped_count(&self) -> usize {
        self.rows.iter().filter(|r| r.is_skipped()).count()
    }

    /// The number of failed rows.
    #[must_use]
    pub fn failed_count(&self) -> usize {
        self.rows.iter().filter(|r| r.is_failed()).count()
    }

    /// `true` if every primitive in
    /// the plan was applied.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.failed_count() == 0 && self.skipped_count() == 0
    }
}

/// A diff between a requested plan
/// and the observed audit.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SandboxDiff {
    /// Primitives that were
    /// requested but not applied.
    pub missing: Vec<SandboxPrimitive>,
    /// Primitives that were applied
    /// but not requested.
    pub unexpected: Vec<SandboxPrimitive>,
    /// Primitives that failed to
    /// apply.
    pub failed: Vec<SandboxPrimitive>,
}

impl SandboxDiff {
    /// A new, empty diff.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` if the plan and the
    /// audit agree.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.missing.is_empty() && self.unexpected.is_empty() && self.failed.is_empty()
    }

    /// The total number of
    /// discrepancies.
    #[must_use]
    pub fn discrepancy_count(&self) -> usize {
        self.missing.len() + self.unexpected.len() + self.failed.len()
    }
}

/// Compare a plan and an audit,
/// producing a `SandboxDiff`.
#[must_use]
pub fn diff_plan_vs_audit(plan: &SandboxPlan, audit: &SandboxAudit) -> SandboxDiff {
    let requested = primitives_for(plan);
    let mut diff = SandboxDiff::new();
    for p in &requested {
        let row = audit.rows.iter().find(|r| &r.primitive == p);
        match row {
            None => diff.missing.push(p.clone()),
            Some(r) if r.is_failed() => diff.failed.push(p.clone()),
            Some(_) => {}
        }
    }
    for r in &audit.rows {
        if !requested.contains(&r.primitive) {
            diff.unexpected.push(r.primitive.clone());
        }
    }
    diff
}

/// The sandbox enforcer trait. The
/// runtime plugs in a real backend.
pub trait SandboxEnforcer {
    /// Apply the plan for the named
    /// service, recording an audit.
    fn apply(&self, plan: &SandboxPlan, service: &str, now_ms: u64) -> SandboxAudit;
}

/// A null enforcer. Records every
/// primitive as skipped. Used for
/// tests and on non-Linux hosts.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NullEnforcer;

impl SandboxEnforcer for NullEnforcer {
    fn apply(&self, plan: &SandboxPlan, service: &str, now_ms: u64) -> SandboxAudit {
        let mut audit = SandboxAudit::new(plan.clone(), service, now_ms);
        for p in primitives_for(plan) {
            audit.record(PrimitiveAudit::new(
                p,
                PrimitiveStatus::Skipped,
                0,
            ).with_reason("null enforcer"));
        }
        audit
    }
}

/// A policy that says which
/// profiles may run on this host.
/// Used by the supervisor to refuse
/// to launch a service whose profile
/// is not in the allow-list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxHostPolicy {
    /// The profiles allowed on this
    /// host.
    pub allowed_profiles: Vec<SandboxProfile>,
    /// Whether to refuse launches
    /// when the audit shows
    /// discrepancies.
    pub refuse_on_discrepancy: bool,
}

impl SandboxHostPolicy {
    /// A permissive default: all
    /// profiles allowed, but the
    /// supervisor will still flag
    /// discrepancies.
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            allowed_profiles: alloc::vec![
                SandboxProfile::Internal,
                SandboxProfile::SystemService,
                SandboxProfile::RestrictedService,
            ],
            refuse_on_discrepancy: false,
        }
    }

    /// A strict policy: only
    /// `RestrictedService` is
    /// allowed, and any audit
    /// discrepancy is fatal.
    #[must_use]
    pub fn strict() -> Self {
        Self {
            allowed_profiles: alloc::vec![SandboxProfile::RestrictedService],
            refuse_on_discrepancy: true,
        }
    }

    /// `true` if the profile is
    /// allowed.
    #[must_use]
    pub fn allows(&self, profile: SandboxProfile) -> bool {
        self.allowed_profiles.contains(&profile)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_core::sandbox::plan_sandbox;
    use aether_core::manifest::SandboxProfile;

    #[test]
    fn primitive_as_str() {
        assert_eq!(SandboxPrimitive::NoNewPrivs.as_str(), "no-new-privs");
        assert_eq!(SandboxPrimitive::ExecChild.as_str(), "exec-child");
    }

    #[test]
    fn primitive_status_as_str() {
        assert_eq!(PrimitiveStatus::Applied.as_str(), "applied");
        assert_eq!(PrimitiveStatus::Skipped.as_str(), "skipped");
        assert_eq!(PrimitiveStatus::Failed.as_str(), "failed");
    }

    #[test]
    fn row_with_reason() {
        let r = PrimitiveAudit::new(
            SandboxPrimitive::NoNewPrivs,
            PrimitiveStatus::Skipped,
            0,
        )
        .with_reason("not supported");
        assert_eq!(r.reason, "not supported");
        assert!(r.is_skipped());
    }

    #[test]
    fn row_status_predicates() {
        let applied = PrimitiveAudit::new(SandboxPrimitive::NoNewPrivs, PrimitiveStatus::Applied, 0);
        let skipped = PrimitiveAudit::new(SandboxPrimitive::NoNewPrivs, PrimitiveStatus::Skipped, 0);
        let failed = PrimitiveAudit::new(SandboxPrimitive::NoNewPrivs, PrimitiveStatus::Failed, -1);
        assert!(applied.is_applied());
        assert!(skipped.is_skipped());
        assert!(failed.is_failed());
    }

    #[test]
    fn primitives_for_internal_profile_is_short() {
        let plan = plan_sandbox(SandboxProfile::Internal);
        let p = primitives_for(&plan);
        // Internal only requests the
        // always-on primitives (cgroup
        // slice + limits + exec).
        assert!(!p.contains(&SandboxPrimitive::NoNewPrivs));
    }

    #[test]
    fn primitives_for_system_service_includes_no_new_privs() {
        let plan = plan_sandbox(SandboxProfile::SystemService);
        let p = primitives_for(&plan);
        assert!(p.contains(&SandboxPrimitive::NoNewPrivs));
    }

    #[test]
    fn primitives_for_restricted_includes_seccomp() {
        let plan = plan_sandbox(SandboxProfile::RestrictedService);
        let p = primitives_for(&plan);
        assert!(p.contains(&SandboxPrimitive::InstallSeccomp));
    }

    #[test]
    fn audit_record_and_counts() {
        let plan = plan_sandbox(SandboxProfile::SystemService);
        let mut audit = SandboxAudit::new(plan, "aether-agentd", 100);
        for p in primitives_for(&audit.plan) {
            audit.record(PrimitiveAudit::new(
                p,
                PrimitiveStatus::Applied,
                0,
            ));
        }
        assert_eq!(audit.applied_count(), audit.rows.len());
        assert_eq!(audit.skipped_count(), 0);
        assert_eq!(audit.failed_count(), 0);
        assert!(audit.is_complete());
    }

    #[test]
    fn audit_incomplete_when_failed() {
        let plan = plan_sandbox(SandboxProfile::SystemService);
        let mut audit = SandboxAudit::new(plan, "aether-agentd", 100);
        audit.record(PrimitiveAudit::new(
            SandboxPrimitive::NoNewPrivs,
            PrimitiveStatus::Failed,
            -1,
        ));
        assert!(!audit.is_complete());
        assert_eq!(audit.failed_count(), 1);
    }

    #[test]
    fn diff_clean() {
        let plan = plan_sandbox(SandboxProfile::SystemService);
        let mut audit = SandboxAudit::new(plan.clone(), "s", 0);
        for p in primitives_for(&plan) {
            audit.record(PrimitiveAudit::new(
                p,
                PrimitiveStatus::Applied,
                0,
            ));
        }
        let d = diff_plan_vs_audit(&plan, &audit);
        assert!(d.is_clean());
        assert_eq!(d.discrepancy_count(), 0);
    }

    #[test]
    fn diff_detects_missing() {
        let plan = plan_sandbox(SandboxProfile::SystemService);
        let audit = SandboxAudit::new(plan.clone(), "s", 0);
        let d = diff_plan_vs_audit(&plan, &audit);
        let expected = primitives_for(&plan).len();
        assert_eq!(d.missing.len(), expected);
    }

    #[test]
    fn diff_detects_unexpected() {
        let plan = plan_sandbox(SandboxProfile::Internal);
        let mut audit = SandboxAudit::new(plan, "s", 0);
        audit.record(PrimitiveAudit::new(
            SandboxPrimitive::NoNewPrivs,
            PrimitiveStatus::Applied,
            0,
        ));
        let d = diff_plan_vs_audit(&audit.plan, &audit);
        assert_eq!(d.unexpected.len(), 1);
    }

    #[test]
    fn diff_detects_failed() {
        let plan = plan_sandbox(SandboxProfile::SystemService);
        let mut audit = SandboxAudit::new(plan.clone(), "s", 0);
        audit.record(PrimitiveAudit::new(
            SandboxPrimitive::NoNewPrivs,
            PrimitiveStatus::Failed,
            -1,
        ));
        let d = diff_plan_vs_audit(&plan, &audit);
        assert_eq!(d.failed.len(), 1);
    }

    #[test]
    fn null_enforcer_records_skipped() {
        let plan = plan_sandbox(SandboxProfile::SystemService);
        let audit = NullEnforcer.apply(&plan, "aether-agentd", 0);
        assert_eq!(audit.service, "aether-agentd");
        let expected = primitives_for(&plan).len();
        assert_eq!(audit.rows.len(), expected);
        for r in &audit.rows {
            assert!(r.is_skipped());
            assert_eq!(r.reason, "null enforcer");
        }
    }

    #[test]
    fn host_policy_permissive_allows_all() {
        let p = SandboxHostPolicy::permissive();
        assert!(p.allows(SandboxProfile::Internal));
        assert!(p.allows(SandboxProfile::SystemService));
        assert!(p.allows(SandboxProfile::RestrictedService));
        assert!(!p.refuse_on_discrepancy);
    }

    #[test]
    fn host_policy_strict_rejects_internal() {
        let p = SandboxHostPolicy::strict();
        assert!(!p.allows(SandboxProfile::Internal));
        assert!(p.allows(SandboxProfile::RestrictedService));
        assert!(p.refuse_on_discrepancy);
    }
}
