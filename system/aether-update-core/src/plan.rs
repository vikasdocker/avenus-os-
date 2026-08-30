// Update plan: the declarative description of "what
// should happen" for a single update, plus the bridge
// from a verified `SignedUpdate` to that plan.
//
// The plan is pure data; the future update-agent daemon
// reads it, stages the payload, applies it, and records
// the result. The planning layer does not perform I/O
// and does not own the state machine (see `state`).
//
// An `UpdatePlan` is constructed either directly by a
// caller (operator-initiated dry-run) or by
// `plan_from_signed_update`, which is the integration
// point with the security crate's verifier.

use serde::{Deserialize, Serialize};

use aether_security::signed_update::{SignedUpdate, UpdateKind};

use crate::version::{VersionPolicy, VersionPolicyDecision};

/// The action an update represents. Distinct from the
/// payload kind: a payload of kind `os-image` can be
/// either an upgrade or a reinstall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateAction {
    /// The payload is an OS image that upgrades the
    /// installed OS to a strictly greater version.
    UpgradeOsImage,
    /// The payload is an OS image that reinstalls the
    /// same version of the OS.
    ReinstallOsImage,
    /// The payload is a service bundle that upgrades
    /// one or more services to a strictly greater
    /// version.
    UpgradeServiceBundle,
    /// The payload is a service bundle that reinstalls
    /// the same version of one or more services.
    ReinstallServiceBundle,
    /// The payload is a new agent model.
    UpgradeAgentModel,
}

impl UpdateAction {
    /// Returns the canonical kebab-case name.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UpgradeOsImage => "upgrade-os-image",
            Self::ReinstallOsImage => "reinstall-os-image",
            Self::UpgradeServiceBundle => "upgrade-service-bundle",
            Self::ReinstallServiceBundle => "reinstall-service-bundle",
            Self::UpgradeAgentModel => "upgrade-agent-model",
        }
    }
}

/// The declarative description of an update.
///
/// `version` is the version the target will be at once
/// the update applies successfully. The currently
/// installed version lives outside the plan (in the
/// caller's `VersionPolicy` context).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatePlan {
    /// The component the update targets. For
    /// `os-image` this is typically `"aether-os"`; for
    /// `service-bundle` it is the service id; for
    /// `agent-model` it is the model id.
    pub target: String,
    /// The payload kind (matches the SignedUpdate
    /// header). Kept here so the IPC layer can render
    /// it without re-decoding the header.
    pub kind: UpdateKind,
    /// The action (upgrade vs reinstall). Derived
    /// from the kind + the version comparison.
    pub action: UpdateAction,
    /// The version the target will be at after a
    /// successful apply.
    pub version: String,
    /// The wall-clock timestamp carried by the
    /// SignedUpdate header. The plan does not enforce
    /// a "newer-than" check; the caller may apply one
    /// on top.
    pub timestamp_ms: u64,
    /// The signer fingerprint, copied from the
    /// SignedUpdate header.
    pub signer_fingerprint: String,
    /// The size of the payload in bytes. The plan
    /// itself does not carry the payload; the
    /// future update-agent retrieves it from the
    /// staging area.
    pub payload_len: usize,
    /// The decision returned by the version policy.
    /// The plan is constructed only when this is
    /// `allowed = true`; the field is preserved so
    /// the IPC layer can report it for audit.
    pub version_decision: VersionPolicyDecision,
}

/// Reasons an update can fail to turn into a plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdatePlanError {
    /// The version policy denied the update.
    PolicyDenied(String),
    /// The SignedUpdate's `target` is empty.
    EmptyTarget,
    /// The SignedUpdate's `version` is empty.
    EmptyVersion,
    /// The SignedUpdate's `payload_len` is zero.
    EmptyPayload,
}

impl std::fmt::Display for UpdatePlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PolicyDenied(s) => write!(f, "version policy denied: {s}"),
            Self::EmptyTarget => f.write_str("signed update has empty target"),
            Self::EmptyVersion => f.write_str("signed update has empty version"),
            Self::EmptyPayload => f.write_str("signed update has zero-length payload"),
        }
    }
}

impl std::error::Error for UpdatePlanError {}

/// Turns a verified `SignedUpdate` into an `UpdatePlan`.
///
/// The caller is responsible for signature verification
/// (via `aether_security::signed_update::verify_signed_update_*`)
/// *before* calling this. We do not re-verify here; the
/// plan is the next step in the pipeline.
///
/// `installed_version` is the version of `target` that
/// is currently installed (or `None` if `target` is not
/// yet installed — the policy must then allow the
/// update). When `None`, the policy is consulted with
/// the literal version `"0.0.0"` as the installed
/// version, so an upgrade to any non-`0.0.0` version
/// is allowed; a downgrade to `0.0.0` is rejected.
pub fn plan_from_signed_update(
    update: &SignedUpdate,
    installed_version: Option<&str>,
    policy: &VersionPolicy,
) -> Result<UpdatePlan, UpdatePlanError> {
    if update.header.target.is_empty() {
        return Err(UpdatePlanError::EmptyTarget);
    }
    if update.header.version.is_empty() {
        return Err(UpdatePlanError::EmptyVersion);
    }
    if update.header.payload_len == 0 {
        return Err(UpdatePlanError::EmptyPayload);
    }
    let installed = installed_version.unwrap_or("0.0.0");
    // We don't know the action yet, so we evaluate the
    // policy twice: first to learn the requirement, then
    // again to confirm the action lines up. Both calls
    // are cheap (no I/O, no allocation), so the
    // duplication is fine.
    let pre =
        policy.evaluate(action_placeholder(&update.header.kind), installed, &update.header.version);
    let action = derive_action(&update.header.kind, &pre.requirement);
    let decision = policy.evaluate(action, installed, &update.header.version);
    if !decision.allowed {
        return Err(UpdatePlanError::PolicyDenied(decision.reason));
    }
    Ok(UpdatePlan {
        target: update.header.target.clone(),
        kind: update.header.kind,
        action,
        version: update.header.version.clone(),
        timestamp_ms: update.header.timestamp_ms,
        signer_fingerprint: update.header.signer_key_id.clone(),
        payload_len: update.header.payload_len,
        version_decision: decision,
    })
}

/// A conservative placeholder used to query the version
/// policy *before* we know which action the update
/// represents. We pick the action that produces the
/// most-permissive `requirement` (so a downgrade is not
/// mis-classified as a same-version reinstall just
/// because the policy would allow reinstalls).
fn action_placeholder(kind: &UpdateKind) -> UpdateAction {
    match kind {
        UpdateKind::OsImage => UpdateAction::UpgradeOsImage,
        UpdateKind::ServiceBundle => UpdateAction::UpgradeServiceBundle,
        UpdateKind::AgentModel => UpdateAction::UpgradeAgentModel,
    }
}

/// Derives the action from the payload kind and the
/// version-policy requirement.
fn derive_action(kind: &UpdateKind, req: &crate::version::VersionRequirement) -> UpdateAction {
    use crate::version::VersionRequirement;
    match (kind, req) {
        (UpdateKind::OsImage, VersionRequirement::Same) => UpdateAction::ReinstallOsImage,
        (UpdateKind::ServiceBundle, VersionRequirement::Same) => {
            UpdateAction::ReinstallServiceBundle
        }
        (UpdateKind::OsImage, _) => UpdateAction::UpgradeOsImage,
        (UpdateKind::ServiceBundle, _) => UpdateAction::UpgradeServiceBundle,
        (UpdateKind::AgentModel, _) => UpdateAction::UpgradeAgentModel,
    }
}

/// A bundle of a signed update, its verification key,
/// and the caller's installed-version context. Used by
/// the IPC layer to chain verify + plan in a single
/// call.
pub struct PlanRequest<'a> {
    pub update: &'a SignedUpdate,
    pub public_key_bytes: &'a [u8; 32],
    pub installed_version: Option<&'a str>,
    pub policy: &'a VersionPolicy,
}

/// Convenience: signs an update with a fresh random
/// signer, used by the integration tests so they don't
/// each hand-roll a key.
#[cfg(test)]
pub fn sign_for_test(
    kind: UpdateKind,
    target: &str,
    version: &str,
    timestamp_ms: u64,
    payload: &[u8],
) -> (SignedUpdate, aether_security::signed_update::UpdateSigner) {
    let signer = aether_security::signed_update::UpdateSigner::generate();
    let update = signer.sign(kind, target, version, timestamp_ms, payload);
    (update, signer)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn policy_default() -> VersionPolicy {
        VersionPolicy::default()
    }

    fn policy_with_downgrade() -> VersionPolicy {
        VersionPolicy::new(true, true)
    }

    #[test]
    fn plan_from_upgrade_os_image() {
        let (update, _signer) = sign_for_test(
            UpdateKind::OsImage,
            "aether-os",
            "1.2.0",
            1_700_000_000_000,
            b"image-bytes",
        );
        let plan = plan_from_signed_update(&update, Some("1.1.0"), &policy_default()).unwrap();
        assert_eq!(plan.target, "aether-os");
        assert_eq!(plan.action, UpdateAction::UpgradeOsImage);
        assert_eq!(plan.version, "1.2.0");
        assert_eq!(plan.kind, UpdateKind::OsImage);
        assert_eq!(plan.version_decision.requirement, crate::version::VersionRequirement::Upgrade);
    }

    #[test]
    fn plan_from_reinstall_service_bundle() {
        let (update, _) = sign_for_test(
            UpdateKind::ServiceBundle,
            "aether-agentd",
            "0.5.0",
            1_700_000_000_000,
            b"bundle",
        );
        let plan = plan_from_signed_update(&update, Some("0.5.0"), &policy_default()).unwrap();
        assert_eq!(plan.action, UpdateAction::ReinstallServiceBundle);
    }

    #[test]
    fn plan_from_agent_model_with_no_installed_version() {
        let (update, _) = sign_for_test(
            UpdateKind::AgentModel,
            "model-base",
            "0.1.0",
            1_700_000_000_000,
            b"weights",
        );
        let plan = plan_from_signed_update(&update, None, &policy_default()).unwrap();
        assert_eq!(plan.action, UpdateAction::UpgradeAgentModel);
    }

    #[test]
    fn plan_rejects_empty_target() {
        let (update, _) = sign_for_test(UpdateKind::OsImage, "", "1.0.0", 1, b"image");
        let err = plan_from_signed_update(&update, Some("0.9.0"), &policy_default()).unwrap_err();
        assert_eq!(err, UpdatePlanError::EmptyTarget);
    }

    #[test]
    fn plan_rejects_empty_version() {
        let (update, _) = sign_for_test(UpdateKind::OsImage, "aether-os", "", 1, b"image");
        let err = plan_from_signed_update(&update, Some("0.9.0"), &policy_default()).unwrap_err();
        assert_eq!(err, UpdatePlanError::EmptyVersion);
    }

    #[test]
    fn plan_rejects_empty_payload() {
        // Sign with a non-empty payload then empty
        // it on the wire — the plan rejects zero
        // length.
        let (mut update, _) = sign_for_test(UpdateKind::OsImage, "aether-os", "1.0.0", 1, b"image");
        update.payload.clear();
        update.header.payload_len = 0;
        let err = plan_from_signed_update(&update, Some("0.9.0"), &policy_default()).unwrap_err();
        assert_eq!(err, UpdatePlanError::EmptyPayload);
    }

    #[test]
    fn plan_rejects_downgrade_by_default() {
        let (update, _) = sign_for_test(UpdateKind::OsImage, "aether-os", "0.9.0", 1, b"image");
        let err = plan_from_signed_update(&update, Some("1.0.0"), &policy_default()).unwrap_err();
        match err {
            UpdatePlanError::PolicyDenied(s) => assert!(s.contains("downgrade")),
            other => panic!("expected PolicyDenied, got {other:?}"),
        }
    }

    #[test]
    fn plan_accepts_downgrade_with_flag() {
        let (update, _) = sign_for_test(UpdateKind::OsImage, "aether-os", "0.9.0", 1, b"image");
        let plan =
            plan_from_signed_update(&update, Some("1.0.0"), &policy_with_downgrade()).unwrap();
        assert_eq!(plan.action, UpdateAction::UpgradeOsImage);
    }

    #[test]
    fn action_as_str_is_stable() {
        assert_eq!(UpdateAction::UpgradeOsImage.as_str(), "upgrade-os-image");
        assert_eq!(UpdateAction::ReinstallOsImage.as_str(), "reinstall-os-image");
        assert_eq!(UpdateAction::UpgradeServiceBundle.as_str(), "upgrade-service-bundle");
        assert_eq!(UpdateAction::ReinstallServiceBundle.as_str(), "reinstall-service-bundle");
        assert_eq!(UpdateAction::UpgradeAgentModel.as_str(), "upgrade-agent-model");
    }
}
