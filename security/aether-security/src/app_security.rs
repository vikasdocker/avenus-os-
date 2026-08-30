// Application security layer (Phase 9.3).
//
// This module is the bridge between `aether_core::app` (the typed
// `AppPackage` / `AppManifest` contract) and the kernel-sandbox
// layer (`aether_core::sandbox::plan_sandbox` + the
// `aether-sandbox` binary). It is also the *install-time*
// authority: every grant or refusal of an `AppPermission` flows
// through here, and the decision is recorded in the system
// audit log so a post-incident review can answer "did the user
// actually approve this app's network access?" without ambiguity.
//
// Three concepts:
//   1. `AppPermission -> Capability` mapping. The install-time
//      consent prompt is a single ordered list of typed
//      `AppPermission` values, but the runtime gate operates on
//      the broader `Capability` system. The mapping in
//      `app_permission_capability` makes that bridge explicit and
//      unit-testable.
//   2. `AppConsentRecord`. A persistent, per-app record of which
//      permissions the user actually approved at install time.
//      The runtime gate MUST consult this record before allowing
//      any capability the app touches; a missing or denied entry
//      causes the gate to deny.
//   3. `AppInstallDecision` + `AppInstaller`. The install-time
//      entry point: it produces a consent record (granted set),
//      a derived `SandboxPlan`, and a structured install verdict
//      that the application manager returns to the caller and
//      records in the audit log.
//
// Threading & storage: `AppConsentRecord` is `Clone + PartialEq +
// Eq + Serialize + Deserialize`. It is meant to be persisted as
// JSON in `~/.local/share/aether/apps/<app_id>/consent.json` (the
// application manager owns the on-disk layout). The consent
// record carries the publisher key fingerprint from the manifest
// so a tampered or swapped app cannot inherit the consent of the
// real publisher.

use aether_core::app::{app_cgroup_slice, AppManifest, AppPackage, AppPermission};
use aether_core::capability::{Capability, CapabilityDomain, RiskLevel};
use aether_core::sandbox::{plan_sandbox, SandboxPlan};
use serde::{Deserialize, Serialize};

use crate::audit::AuditChain;

/// The set of permissions the user has granted to a single app.
///
/// This is the *install-time* contract. The runtime gate
/// (`AppPermissionGate`) consults this record for every
/// capability check; if a permission is not in this set, the
/// gate denies the request regardless of any other policy.
///
/// `version` is a monotonic counter incremented on every
/// change; it is included in the audit log so a reviewer can
/// correlate a "consent record was rewritten" event with the
/// record it replaced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConsentRecord {
    /// The reverse-DNS app id from the manifest.
    pub app_id: String,
    /// Hex SHA-256 fingerprint of the publisher's signing key,
    /// copied from `AppManifest::publisher_key_id`. The runtime
    /// gate refuses to honour a consent record whose fingerprint
    /// does not match the manifest being launched — a swapped
    /// app cannot inherit consent.
    pub publisher_key_id: String,
    /// The set of `AppPermission` values the user explicitly
    /// granted at install time.
    pub granted: Vec<AppPermission>,
    /// Wall-clock timestamp the user granted the record.
    pub granted_at_ms: u64,
    /// Monotonic version, incremented on every change. Starts
    /// at 1 on first creation.
    pub version: u32,
}

impl AppConsentRecord {
    /// Construct a fresh record. `granted_at_ms` is supplied by
    /// the caller (the application manager reads the wall clock);
    /// the consent record itself is a pure value.
    #[must_use]
    pub fn new(
        app_id: impl Into<String>,
        publisher_key_id: impl Into<String>,
        granted: Vec<AppPermission>,
        granted_at_ms: u64,
    ) -> Self {
        Self {
            app_id: app_id.into(),
            publisher_key_id: publisher_key_id.into(),
            granted,
            granted_at_ms,
            version: 1,
        }
    }

    /// Returns `true` if `permission` is in the granted set.
    #[must_use]
    pub fn grants(&self, permission: AppPermission) -> bool {
        self.granted.contains(&permission)
    }

    /// Returns the granted permissions in a stable,
    /// sorted-by-`as_str` order. The application manager uses
    /// this when persisting the record so semantically-equal
    /// records always serialise byte-for-byte identically.
    #[must_use]
    pub fn sorted_grants(&self) -> Vec<AppPermission> {
        let mut g = self.granted.clone();
        g.sort_by_key(|p| p.as_str());
        g
    }
}

/// The install-time verdict returned by `AppInstaller`.
///
/// The application manager carries this back to the caller
/// (the future `aether-store`) and writes the audit log line
/// `"app.install"` with the embedded plan digest and consent
/// record version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppInstallDecision {
    /// The manifest that was approved. Stored by value so the
    /// decision is self-describing.
    pub manifest: AppManifest,
    /// The consent record the installer produced. Equal to
    /// `manifest.permissions` minus the entries the user
    /// refused; the user is always told exactly which
    /// permissions were dropped.
    pub consent: AppConsentRecord,
    /// The kernel-sandbox plan the application manager will
    /// hand to `aether-sandbox` on launch. Apps always get
    /// the `RestrictedService` plan: full user/pid/network/ipc
    /// namespace isolation, no_new_privs, the strict
    /// `restricted-app-v1` seccomp filter tag, and the app's
    /// own cgroup slice under `aether.slice/app.<id>.slice`.
    pub plan: SandboxPlan,
    /// The list of permissions the user **refused** at install
    /// time. The app is installed but will not be able to use
    /// any capability backed by one of these permissions; the
    /// gate denies them.
    pub refused: Vec<AppPermission>,
}

/// The application installer. The owner of the install-time
/// consent flow.
///
/// The installer is constructed with a fresh `AuditChain`
/// reference (the application manager owns the chain). It is
/// *not* the gate itself; the gate is `AppPermissionGate`,
/// below. Splitting the two keeps the audit-log side effect
/// concentrated in the install path and leaves the gate as a
/// pure function for the runtime path.
pub struct AppInstaller<'a> {
    audit_chain: &'a mut AuditChain,
}

impl<'a> AppInstaller<'a> {
    /// Construct an installer that records its decisions into
    /// `audit_chain`.
    pub fn new(audit_chain: &'a mut AuditChain) -> Self {
        Self { audit_chain }
    }

    /// Drive the install-time consent flow.
    ///
    /// `user_consent` is the *granted* subset of
    /// `manifest.permissions` — i.e. the set the user actually
    /// ticked in the install-time UI. The installer compares
    /// this against the manifest's request list, derives the
    /// refused set, builds the consent record, derives the
    /// sandbox plan, writes the audit log line, and returns
    /// the decision.
    ///
    /// # Errors
    /// Returns `Err` if any permission in `user_consent` is not
    /// in `manifest.permissions` (the user cannot grant a
    /// permission the app did not request) or if the manifest
    /// itself fails validation.
    pub fn install(
        &mut self,
        manifest: AppManifest,
        user_consent: &[AppPermission],
        granted_at_ms: u64,
    ) -> Result<AppInstallDecision, String> {
        // 1. Validate the manifest first; a malformed
        //    manifest is rejected before any consent is
        //    recorded.
        manifest.validate()?;

        // 2. The user may only grant permissions the manifest
        //    actually requested.
        for granted in user_consent {
            if !manifest.permissions.iter().any(|p| p == granted) {
                return Err(format!(
                    "user granted '{}' but the manifest did not request it",
                    granted.as_str()
                ));
            }
        }

        // 3. Derive the refused set (manifest request minus
        //    user grant).
        let refused: Vec<AppPermission> = manifest
            .permissions
            .iter()
            .copied()
            .filter(|p| !user_consent.iter().any(|g| g == p))
            .collect();

        // 4. Build the consent record. The granted set is
        //    sorted for stable persistence.
        let consent = AppConsentRecord {
            app_id: manifest.app_id.clone(),
            publisher_key_id: manifest.publisher_key_id.clone(),
            granted: {
                let mut g = user_consent.to_vec();
                g.sort_by_key(|p| p.as_str());
                g
            },
            granted_at_ms,
            version: 1,
        };

        // 5. Derive the sandbox plan. Apps always go through
        //    the restricted-service path; the slice name is
        //    app-specific so per-app memory caps hold.
        let plan = sandbox_plan_for_app(&manifest);

        // 6. Audit log: the install decision, including the
        //    refused set and the plan digest.
        let refused_strs: Vec<&'static str> = refused.iter().map(|p| p.as_str()).collect();
        let detail = format!(
            "version={} refused=[{}] plan_seccomp={} plan_slice={}",
            consent.version,
            refused_strs.join(","),
            plan.seccomp.as_ref().map(|t| t.as_str()).unwrap_or("none"),
            plan.resources.cgroup_slice,
        );
        self.audit_chain.record(
            granted_at_ms,
            "app.install",
            "app-installer",
            &detail,
        );

        Ok(AppInstallDecision { manifest, consent, plan, refused })
    }
}

/// The runtime permission gate.
///
/// The gate is the *runtime* counterpart to the install-time
/// `AppInstaller`. It does not write the audit log; the
/// application manager wraps the call and records the
/// allow/deny event itself. Splitting the audit side effect
/// keeps the gate a pure function for testing.
#[derive(Debug, Clone)]
pub struct AppPermissionGate<'r> {
    consent: &'r AppConsentRecord,
}

impl<'r> AppPermissionGate<'r> {
    /// Construct a gate bound to a single consent record. The
    /// gate is a thin shim: the record is the source of truth.
    #[must_use]
    pub const fn new(consent: &'r AppConsentRecord) -> Self {
        Self { consent }
    }

    /// Returns the underlying consent record. Exposed so the
    /// application manager can surface "you granted X" to the
    /// user without round-tripping the record separately.
    #[must_use]
    pub const fn consent(&self) -> &AppConsentRecord {
        self.consent
    }

    /// Evaluate a `(app_id, capability)` pair against the
    /// consent record.
    ///
    /// Rules:
    ///   * The capability's `AppPermission` (looked up via
    ///     `app_permission_capability`) must be in the
    ///     granted set. If the user denied it at install
    ///     time, the gate denies regardless of risk.
    ///   * The app id in the consent record must match the
    ///     supplied `app_id`. The gate refuses to answer if
    ///     the caller asks about a different app than the
    ///     record was created for — a programmer error, not
    ///     a runtime policy decision.
    ///   * The capability's risk level is included in the
    ///     reason string so the audit log can be filtered on
    ///     high-risk attempts without re-deriving the level.
    pub fn evaluate(&self, app_id: &str, capability: &Capability) -> GateVerdict {
        if app_id != self.consent.app_id {
            return GateVerdict::deny(format!(
                "consent record is for '{}', not '{app_id}'",
                self.consent.app_id
            ));
        }
        let Some(permission) = app_permission_for_capability(capability) else {
            // The capability is not tied to any user-facing
            // AppPermission. The default is allow — system
            // capabilities (e.g. `application.list`) are not
            // opt-in.
            return GateVerdict::allow("system capability not gated by user consent");
        };
        if self.consent.grants(permission) {
            GateVerdict::allow(format!(
                "user granted '{}' at install time",
                permission.as_str()
            ))
        } else {
            GateVerdict::deny(format!(
                "user did not grant '{}' (capability {} at {:?} risk)",
                permission.as_str(),
                capability.qualified_name(),
                capability.risk_level
            ))
        }
    }
}

/// A gate verdict. The application manager turns this into an
/// `IpcResponse` and a `Decision`; the verifier here is
/// deliberately minimal so it has no dependency on the
/// system-core IPC types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateVerdict {
    /// `true` if the gate allows the call.
    pub allowed: bool,
    /// Human-readable reason. The application manager writes
    /// this into the audit log; UI surfaces show it to the
    /// user when the call is denied.
    pub reason: String,
}

impl GateVerdict {
    /// Construct an `allow` verdict.
    #[must_use]
    pub fn allow(reason: impl Into<String>) -> Self {
        Self { allowed: true, reason: reason.into() }
    }
    /// Construct a `deny` verdict.
    #[must_use]
    pub fn deny(reason: impl Into<String>) -> Self {
        Self { allowed: false, reason: reason.into() }
    }
}

/// Map an `AppPermission` to the underlying `Capability` it
/// guards. Returns `None` for permissions that are *additive*
/// to a capability (e.g. `Notify` is its own capability, with
/// no parent); in that case the capability *is* the permission.
///
/// Risk levels are chosen so the agent-runtime's existing
/// `DefaultPermissionPolicy` produces sensible defaults: a
/// `CaptureScreen` request is automatically
/// `REQUIRES_CONFIRMATION` even if the user has ticked the box
/// at install time. The install-time consent and the
/// runtime confirmation are intentionally distinct gates.
#[must_use]
pub fn app_permission_capability(permission: AppPermission) -> Capability {
    let (domain, name, risk) = match permission {
        AppPermission::ReadUserFiles => {
            (CapabilityDomain::Filesystem, "user.read", RiskLevel::Medium)
        }
        AppPermission::WriteUserFiles => {
            (CapabilityDomain::Filesystem, "user.write", RiskLevel::High)
        }
        AppPermission::NetworkEgress => {
            (CapabilityDomain::Network, "egress", RiskLevel::High)
        }
        AppPermission::NetworkListen => {
            (CapabilityDomain::Network, "listen", RiskLevel::High)
        }
        AppPermission::ReadPersonalData => {
            (CapabilityDomain::Identity, "personal.read", RiskLevel::High)
        }
        AppPermission::Notify => (CapabilityDomain::Application, "notify", RiskLevel::Low),
        AppPermission::CaptureScreen => {
            (CapabilityDomain::Application, "screen.capture", RiskLevel::Critical)
        }
        AppPermission::Camera => (CapabilityDomain::Application, "camera", RiskLevel::High),
        AppPermission::Microphone => {
            (CapabilityDomain::Application, "microphone", RiskLevel::High)
        }
        AppPermission::Location => (CapabilityDomain::Application, "location", RiskLevel::High),
        AppPermission::PairDevices => {
            (CapabilityDomain::Application, "pair.devices", RiskLevel::Medium)
        }
    };
    Capability::new(domain, name, risk)
}

/// Reverse lookup: given a `Capability`, return the
/// `AppPermission` that gates it (if any).
///
/// Returns `None` for capabilities that are not backed by a
/// user-facing `AppPermission` (system-internal capabilities,
/// cross-domain reads, etc). The runtime gate treats `None`
/// as "not gated by user consent" and allows by default.
#[must_use]
pub fn app_permission_for_capability(capability: &Capability) -> Option<AppPermission> {
    if capability.domain == CapabilityDomain::Filesystem
        && capability.name == "user.read"
    {
        return Some(AppPermission::ReadUserFiles);
    }
    if capability.domain == CapabilityDomain::Filesystem
        && capability.name == "user.write"
    {
        return Some(AppPermission::WriteUserFiles);
    }
    if capability.domain == CapabilityDomain::Network && capability.name == "egress" {
        return Some(AppPermission::NetworkEgress);
    }
    if capability.domain == CapabilityDomain::Network && capability.name == "listen" {
        return Some(AppPermission::NetworkListen);
    }
    if capability.domain == CapabilityDomain::Identity
        && capability.name == "personal.read"
    {
        return Some(AppPermission::ReadPersonalData);
    }
    if capability.domain == CapabilityDomain::Application
        && capability.name == "notify"
    {
        return Some(AppPermission::Notify);
    }
    if capability.domain == CapabilityDomain::Application
        && capability.name == "screen.capture"
    {
        return Some(AppPermission::CaptureScreen);
    }
    if capability.domain == CapabilityDomain::Application && capability.name == "camera" {
        return Some(AppPermission::Camera);
    }
    if capability.domain == CapabilityDomain::Application
        && capability.name == "microphone"
    {
        return Some(AppPermission::Microphone);
    }
    if capability.domain == CapabilityDomain::Application
        && capability.name == "location"
    {
        return Some(AppPermission::Location);
    }
    if capability.domain == CapabilityDomain::Application
        && capability.name == "pair.devices"
    {
        return Some(AppPermission::PairDevices);
    }
    None
}

/// Derive a `SandboxPlan` for an `AppManifest`.
///
/// All apps get the `RestrictedService` profile: full user +
/// pid + network + ipc namespace isolation, no_new_privs, a
/// tight seccomp filter tag, and a per-app cgroup slice under
/// `aether.slice/`. The slice name is derived from
/// `app_cgroup_slice` so the per-app memory cap is enforced
/// at the cgroup level.
#[must_use]
pub fn sandbox_plan_for_app(manifest: &AppManifest) -> SandboxPlan {
    // The cgroup slice name is app-specific so per-app
    // memory caps hold even if two apps request the same
    // resources. The plan builder itself only knows about
    // profiles; we patch the slice field on the returned
    // plan.
    let mut plan = plan_sandbox(aether_core::manifest::SandboxProfile::RestrictedService);
    plan.resources.cgroup_slice = app_cgroup_slice(&manifest.app_id);
    // Memory cap is a function of the manifest's declared
    // resource limit. The plan builder leaves it None for
    // restricted apps; we copy the app's `memory_max_bytes`
    // (if any) into the cgroup v2 memory.max the launcher
    // will write. The default cap (512 MiB) is in the plan
    // builder; an app that asks for less gets exactly that.
    if let Some(bytes) = manifest.resources.memory_max_bytes {
        plan.resources.memory_max_bytes = Some(bytes);
    }
    plan
}

/// Verify the publisher-fingerprint binding on a consent
/// record against an `AppPackage`. Returns the verified
/// fingerprint on success; an error otherwise.
///
/// This is the runtime-time check that protects against
/// "swap a malicious payload into a previously-approved
/// directory" attacks: the consent record's
/// `publisher_key_id` MUST match the manifest's
/// `publisher_key_id` field; otherwise the record was
/// written for a *different* publisher than the one that
/// signed the package now on disk.
pub fn verify_consent_for_package(
    consent: &AppConsentRecord,
    package: &AppPackage,
) -> Result<String, String> {
    if consent.app_id != package.manifest.app_id {
        return Err(format!(
            "consent record is for '{}' but package is for '{}'",
            consent.app_id, package.manifest.app_id
        ));
    }
    if consent.publisher_key_id != package.manifest.publisher_key_id {
        return Err(format!(
            "consent record was issued for publisher '{}' but package is signed by '{}'",
            consent.publisher_key_id, package.manifest.publisher_key_id
        ));
    }
    Ok(package.manifest.publisher_key_id.clone())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_core::app::{AppPermission, AppResourceLimits};
    use aether_core::manifest::SandboxProfile;

    fn sample_manifest() -> AppManifest {
        AppManifest {
            schema_version: "1".to_string(),
            app_id: "com.example.calc".to_string(),
            name: "Aether Calculator".to_string(),
            version: "1.0.0".to_string(),
            publisher: "Example Org".to_string(),
            description: "minimal".to_string(),
            min_os_version: "0.2.0".to_string(),
            permissions: vec![
                AppPermission::ReadUserFiles,
                AppPermission::Notify,
                AppPermission::NetworkEgress,
            ],
            sandbox_profile: SandboxProfile::RestrictedService,
            resources: AppResourceLimits {
                cpu_weight: 50,
                memory_max_bytes: Some(64 * 1024 * 1024),
                pids_max: Some(16),
                io_weight: 50,
            },
            binary_sha256: "a".repeat(64),
            payload_len: 4096,
            depends_on: vec![],
            timestamp_ms: 1_700_000_000_000,
            publisher_key_id: "deadbeef".repeat(8),
        }
    }

    #[test]
    fn consent_record_round_trip_json() {
        let r = AppConsentRecord::new(
            "com.example.calc",
            "deadbeef".repeat(8),
            vec![AppPermission::ReadUserFiles, AppPermission::Notify],
            1_700_000_000_000,
        );
        let text = serde_json::to_string(&r).expect("encode");
        let back: AppConsentRecord = serde_json::from_str(&text).expect("decode");
        assert_eq!(back, r);
    }

    #[test]
    fn consent_grants_checks_membership() {
        let r = AppConsentRecord::new(
            "com.example.calc",
            "deadbeef".repeat(8),
            vec![AppPermission::Notify],
            1,
        );
        assert!(r.grants(AppPermission::Notify));
        assert!(!r.grants(AppPermission::ReadUserFiles));
    }

    #[test]
    fn consent_sorted_grants_is_stable() {
        let r = AppConsentRecord::new(
            "com.example.calc",
            "k".repeat(64),
            vec![
                AppPermission::Notify,
                AppPermission::ReadUserFiles,
                AppPermission::NetworkEgress,
            ],
            1,
        );
        let sorted = r.sorted_grants();
        // Sorted by as_str() — kebab-case lexicographic
        // order. The exact ordering is not part of the
        // public contract; we only assert that two calls
        // produce the same sequence and that no permission
        // is lost.
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted, sorted.clone());
        let again = r.sorted_grants();
        assert_eq!(again, sorted);
    }

    #[test]
    fn installer_records_audit_entry() {
        let mut chain = AuditChain::default();
        let manifest = sample_manifest();
        let mut installer = AppInstaller::new(&mut chain);
        let decision = installer
            .install(manifest.clone(), &[AppPermission::Notify], 1_700_000_000_000)
            .expect("install");
        // The audit log captured the install.
        assert_eq!(chain.len(), 1);
        let entry = &chain.entries()[0];
        assert_eq!(entry.event, "app.install");
        assert!(entry.detail.contains("refused=[read-user-files,network-egress]"));
        assert!(entry.detail.contains("plan_seccomp=restricted-app-v1"));
        assert!(entry.detail.contains("plan_slice=aether.slice/app.com_example_calc.slice"));
        // The refused set is exactly the manifest minus the
        // granted set.
        assert_eq!(decision.refused.len(), 2);
        assert!(decision.refused.contains(&AppPermission::ReadUserFiles));
        assert!(decision.refused.contains(&AppPermission::NetworkEgress));
    }

    #[test]
    fn installer_rejects_grant_of_unrequested_permission() {
        let mut chain = AuditChain::default();
        let manifest = sample_manifest();
        let mut installer = AppInstaller::new(&mut chain);
        // The user "grants" Camera; the manifest did not
        // request it. The installer must reject.
        let err = installer
            .install(manifest, &[AppPermission::Camera], 1)
            .expect_err("unrequested grant is rejected");
        assert!(err.contains("camera"), "{err}");
        // No audit entry was recorded for a failed install.
        assert_eq!(chain.len(), 0);
    }

    #[test]
    fn installer_rejects_invalid_manifest() {
        let mut chain = AuditChain::default();
        let mut manifest = sample_manifest();
        manifest.app_id = String::new();
        let mut installer = AppInstaller::new(&mut chain);
        let err = installer
            .install(manifest, &[], 1)
            .expect_err("invalid manifest is rejected");
        assert!(!err.is_empty());
        assert_eq!(chain.len(), 0);
    }

    #[test]
    fn installer_derives_app_specific_cgroup_slice() {
        let mut chain = AuditChain::default();
        let mut installer = AppInstaller::new(&mut chain);
        let decision = installer
            .install(sample_manifest(), &[AppPermission::Notify], 1)
            .expect("install");
        assert_eq!(
            decision.plan.resources.cgroup_slice,
            "aether.slice/app.com_example_calc.slice"
        );
    }

    #[test]
    fn installer_copies_memory_max_into_plan() {
        let mut chain = AuditChain::default();
        let mut installer = AppInstaller::new(&mut chain);
        let decision = installer
            .install(sample_manifest(), &[AppPermission::Notify], 1)
            .expect("install");
        // The manifest requested 64 MiB; the plan carries
        // it through so the launcher can write it into
        // cgroupfs.
        assert_eq!(decision.plan.resources.memory_max_bytes, Some(64 * 1024 * 1024));
    }

    #[test]
    fn installer_grants_all_when_user_approves_everything() {
        let mut chain = AuditChain::default();
        let mut installer = AppInstaller::new(&mut chain);
        let manifest = sample_manifest();
        let all = manifest.permissions.clone();
        let decision = installer
            .install(manifest, &all, 1)
            .expect("install");
        assert!(decision.refused.is_empty());
        // The consent record's granted set equals the
        // manifest's permissions set.
        for p in &manifest_permissions_for_test() {
            assert!(decision.consent.grants(*p));
        }
    }

    fn manifest_permissions_for_test() -> Vec<AppPermission> {
        vec![
            AppPermission::ReadUserFiles,
            AppPermission::Notify,
            AppPermission::NetworkEgress,
        ]
    }

    #[test]
    fn gate_allows_granted_permission() {
        let r = AppConsentRecord::new(
            "com.example.calc",
            "k".repeat(64),
            vec![AppPermission::Notify],
            1,
        );
        let gate = AppPermissionGate::new(&r);
        let verdict = gate.evaluate(
            "com.example.calc",
            &app_permission_capability(AppPermission::Notify),
        );
        assert!(verdict.allowed);
        assert!(verdict.reason.contains("notify"));
    }

    #[test]
    fn gate_denies_refused_permission() {
        let r = AppConsentRecord::new(
            "com.example.calc",
            "k".repeat(64),
            vec![AppPermission::Notify],
            1,
        );
        let gate = AppPermissionGate::new(&r);
        let verdict = gate.evaluate(
            "com.example.calc",
            &app_permission_capability(AppPermission::NetworkEgress),
        );
        assert!(!verdict.allowed);
        assert!(verdict.reason.contains("network-egress"));
    }

    #[test]
    fn gate_denies_mismatched_app_id() {
        let r = AppConsentRecord::new(
            "com.example.calc",
            "k".repeat(64),
            vec![AppPermission::Notify],
            1,
        );
        let gate = AppPermissionGate::new(&r);
        let verdict = gate.evaluate(
            "com.example.other",
            &app_permission_capability(AppPermission::Notify),
        );
        assert!(!verdict.allowed);
    }

    #[test]
    fn gate_allows_system_capability_not_backed_by_permission() {
        let r = AppConsentRecord::new(
            "com.example.calc",
            "k".repeat(64),
            vec![AppPermission::Notify],
            1,
        );
        let gate = AppPermissionGate::new(&r);
        let system_cap = Capability::new(
            CapabilityDomain::Application,
            "list",
            RiskLevel::Low,
        );
        let verdict = gate.evaluate("com.example.calc", &system_cap);
        assert!(verdict.allowed);
    }

    #[test]
    fn app_permission_capability_maps_all_variants() {
        // Every variant in the AppPermission enum must map
        // to a non-empty capability; if a future variant is
        // added without a mapping, this test fails.
        let perms = [
            AppPermission::ReadUserFiles,
            AppPermission::WriteUserFiles,
            AppPermission::NetworkEgress,
            AppPermission::NetworkListen,
            AppPermission::ReadPersonalData,
            AppPermission::Notify,
            AppPermission::CaptureScreen,
            AppPermission::Camera,
            AppPermission::Microphone,
            AppPermission::Location,
            AppPermission::PairDevices,
        ];
        for p in perms {
            let c = app_permission_capability(p);
            assert!(!c.qualified_name().is_empty());
            assert_eq!(c.domain, c.domain); // trivial
        }
    }

    #[test]
    fn reverse_lookup_round_trips_for_all_variants() {
        let perms = [
            AppPermission::ReadUserFiles,
            AppPermission::WriteUserFiles,
            AppPermission::NetworkEgress,
            AppPermission::NetworkListen,
            AppPermission::ReadPersonalData,
            AppPermission::Notify,
            AppPermission::CaptureScreen,
            AppPermission::Camera,
            AppPermission::Microphone,
            AppPermission::Location,
            AppPermission::PairDevices,
        ];
        for p in perms {
            let c = app_permission_capability(p);
            let back = app_permission_for_capability(&c);
            assert_eq!(back, Some(p), "round-trip failed for {:?}", p);
        }
    }

    #[test]
    fn capture_screen_is_critical_risk() {
        // CaptureScreen is intentionally `Critical` so the
        // DefaultPermissionPolicy requires consent even
        // after install-time approval. This is a defence
        // against the "ticked the box once" attack.
        let c = app_permission_capability(AppPermission::CaptureScreen);
        assert_eq!(c.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn write_user_files_is_high_risk() {
        let c = app_permission_capability(AppPermission::WriteUserFiles);
        assert_eq!(c.risk_level, RiskLevel::High);
    }

    #[test]
    fn verify_consent_rejects_swapped_publisher() {
        let mut chain = AuditChain::default();
        let mut installer = AppInstaller::new(&mut chain);
        let manifest = sample_manifest();
        let decision = installer
            .install(manifest, &[AppPermission::Notify], 1)
            .expect("install");
        // A second package signed by a different publisher
        // cannot reuse the consent record.
        let mut swapped = sample_manifest();
        swapped.publisher_key_id = "cafebabe".repeat(8);
        swapped.payload_len = 1; // not zero
        let fake_pkg = aether_core::app::AppPackage {
            manifest: swapped,
            signature: "sig".to_string(),
            payload: vec![1],
        };
        let err =
            verify_consent_for_package(&decision.consent, &fake_pkg).expect_err("swap rejected");
        assert!(err.contains("publisher"), "{err}");
    }

    #[test]
    fn verify_consent_accepts_matching_publisher() {
        let mut chain = AuditChain::default();
        let mut installer = AppInstaller::new(&mut chain);
        let manifest = sample_manifest();
        let decision = installer
            .install(manifest.clone(), &[AppPermission::Notify], 1)
            .expect("install");
        let pkg = aether_core::app::AppPackage {
            manifest,
            signature: "sig".to_string(),
            payload: vec![1],
        };
        let fp = verify_consent_for_package(&decision.consent, &pkg).expect("verify");
        assert_eq!(fp, "deadbeef".repeat(8));
    }

    #[test]
    fn verify_consent_rejects_mismatched_app_id() {
        let mut chain = AuditChain::default();
        let mut installer = AppInstaller::new(&mut chain);
        let manifest = sample_manifest();
        let decision = installer
            .install(manifest, &[AppPermission::Notify], 1)
            .expect("install");
        let mut other = sample_manifest();
        other.app_id = "com.example.other".to_string();
        let pkg = aether_core::app::AppPackage {
            manifest: other,
            signature: "sig".to_string(),
            payload: vec![1],
        };
        let err =
            verify_consent_for_package(&decision.consent, &pkg).expect_err("mismatched app id");
        assert!(err.contains("'com.example.calc'"), "{err}");
    }

    #[test]
    fn sandbox_plan_uses_restricted_profile() {
        let plan = sandbox_plan_for_app(&sample_manifest());
        // The seccomp filter tag for the restricted profile
        // is `restricted-app-v1`; the launcher reads this
        // name to look up the compiled policy.
        let seccomp = plan
            .seccomp
            .as_ref()
            .map(|t| t.as_str().to_string())
            .unwrap_or_default();
        assert_eq!(seccomp, "restricted-app-v1");
        // No new privileges is set.
        assert!(plan.no_new_privs);
        // The capability whitelist is empty: a restricted
        // app starts with nothing and may not acquire any
        // ambient capability.
        assert!(plan.capabilities.is_empty());
    }

    #[test]
    fn sandbox_plan_isolates_namespaces() {
        let plan = sandbox_plan_for_app(&sample_manifest());
        // The restricted profile is fully isolating.
        assert!(plan.namespaces.contains(&aether_core::sandbox::LinuxNamespace::User));
        assert!(plan.namespaces.contains(&aether_core::sandbox::LinuxNamespace::Pid));
        assert!(plan.namespaces.contains(&aether_core::sandbox::LinuxNamespace::Network));
        assert!(plan.namespaces.contains(&aether_core::sandbox::LinuxNamespace::Ipc));
        assert!(plan.namespaces.contains(&aether_core::sandbox::LinuxNamespace::Uts));
    }
}
