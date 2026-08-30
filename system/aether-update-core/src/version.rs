// Version policy: decides whether a target version is
// acceptable given the currently installed version, the
// kind of update, and the operator's downgrade policy.
//
// Aether uses a strict, three-component semver-ish
// version ("MAJOR.MINOR.PATCH" with optional
// "-prerelease"). The policy enforces:
//
//   * Strict version shape: every component is a
//     non-negative integer; prereleases are a single
//     dash followed by one to 16 [a-z0-9-]+ characters.
//   * Monotonicity for normal updates: a downgrade is
//     rejected unless `allow_downgrade` is set.
//   * The "kind" of update can request an exception:
//     an `os-image` update with the same version as the
//     installed OS is a "reinstall" — it is allowed for
//     `os-image` and `service-bundle` but rejected for
//     `agent-model` (model downgrades always need an
//     explicit flag).
//   * Pre-release tolerance: a pre-release target with
//     the same MAJOR.MINOR.PATCH is rejected unless
//     `allow_prerelease` is set, even when the version
//     string is technically greater.
//
// The policy does not consult the network, the
// filesystem, or the daemon. It is pure logic; the
// future update-agent runs it before staging.

use serde::{Deserialize, Serialize};

use crate::plan::UpdateAction;

/// The kind of update a version policy is being applied
/// to. Mirrors `aether_security::signed_update::UpdateKind`
/// but kept as a separate type so the planning layer does
/// not depend on the security crate's wire form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VersionRequirement {
    /// The version is strictly greater than the
    /// installed version.
    Upgrade,
    /// The version is strictly less than the
    /// installed version. Requires an explicit
    /// downgrade flag.
    Downgrade,
    /// The version is the same as installed. A
    /// reinstall is allowed for os-image and
    /// service-bundle; the model runtime rejects
    /// reinstalls (model downgrades are downgrades).
    Same,
    /// Pre-release same-version: a pre-release suffix
    /// that differs from the installed version's
    /// pre-release. Treated as a separate category so
    /// the policy can apply a single flag.
    Prerelease,
}

impl VersionRequirement {
    /// Returns the canonical kebab-case name.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Upgrade => "upgrade",
            Self::Downgrade => "downgrade",
            Self::Same => "same",
            Self::Prerelease => "prerelease",
        }
    }
}

/// A parsed semver-ish version. The pre-release
/// component is preserved verbatim; ordering between
/// pre-releases is not defined by semver, so the
/// planning layer treats any change to the pre-release
/// suffix as a non-monotonic change (prerelease kind).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParsedVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub pre: Option<String>,
}

impl ParsedVersion {
    /// Parses a version string. Returns `None` for
    /// malformed input.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let (core, pre) = match raw.split_once('-') {
            Some((c, p)) => (c, Some(p.to_string())),
            None => (raw, None),
        };
        let mut parts = core.split('.');
        let major = parts.next()?.parse::<u32>().ok()?;
        let minor = parts.next()?.parse::<u32>().ok()?;
        let patch = parts.next()?.parse::<u32>().ok()?;
        if parts.next().is_some() {
            return None;
        }
        if let Some(ref p) = pre {
            if p.is_empty() || p.len() > 32 {
                return None;
            }
            if !p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.') {
                return None;
            }
        }
        Some(Self { major, minor, patch, pre })
    }

    /// The numeric ordering of two versions, ignoring
    /// the pre-release suffix.
    fn numeric_cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
    }
}

impl std::fmt::Display for ParsedVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.pre {
            write!(f, "-{pre}")?;
        }
        Ok(())
    }
}

/// The decision the version policy returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionPolicyDecision {
    /// The kind of version change the target
    /// represents.
    pub requirement: VersionRequirement,
    /// Whether the policy allows the update.
    pub allowed: bool,
    /// A short human-readable reason. Empty when
    /// `allowed` is `true`.
    pub reason: String,
}

impl VersionPolicyDecision {
    fn allow(req: VersionRequirement) -> Self {
        Self { requirement: req, allowed: true, reason: String::new() }
    }
    fn deny(req: VersionRequirement, reason: impl Into<String>) -> Self {
        Self { requirement: req, allowed: false, reason: reason.into() }
    }
}

/// The policy object. Constructed once at daemon
/// startup; held by the planning layer and queried on
/// every update.
#[derive(Debug, Clone, Default)]
pub struct VersionPolicy {
    /// Allow downgrades when set. Off by default; an
    /// operator must opt in.
    pub allow_downgrade: bool,
    /// Allow pre-release same-version updates when
    /// set. Off by default; an operator must opt in.
    pub allow_prerelease: bool,
}

impl VersionPolicy {
    /// Creates a policy with the given flags.
    #[must_use]
    pub fn new(allow_downgrade: bool, allow_prerelease: bool) -> Self {
        Self { allow_downgrade, allow_prerelease }
    }

    /// Returns the decision for an update targeting
    /// `target` when the currently installed version is
    /// `installed`, of the given `action`. Unparseable
    /// versions are denied.
    pub fn evaluate(
        &self,
        action: UpdateAction,
        installed: &str,
        target: &str,
    ) -> VersionPolicyDecision {
        let installed = match ParsedVersion::parse(installed) {
            Some(v) => v,
            None => {
                return VersionPolicyDecision::deny(
                    VersionRequirement::Same,
                    format!("installed version '{installed}' is not a valid Aether version"),
                );
            }
        };
        let target = match ParsedVersion::parse(target) {
            Some(v) => v,
            None => {
                return VersionPolicyDecision::deny(
                    VersionRequirement::Same,
                    format!("target version '{target}' is not a valid Aether version"),
                );
            }
        };
        let same_numeric = installed.numeric_cmp(&target) == std::cmp::Ordering::Equal;
        // Pre-release transition: same numeric, different
        // pre-release suffix. Reject unless explicitly
        // allowed.
        if same_numeric && installed.pre != target.pre {
            if self.allow_prerelease {
                return VersionPolicyDecision::allow(VersionRequirement::Prerelease);
            }
            return VersionPolicyDecision::deny(
                VersionRequirement::Prerelease,
                format!(
                    "pre-release target '{target}' is not allowed by the version policy"
                ),
            );
        }
        match installed.numeric_cmp(&target) {
            std::cmp::Ordering::Less => VersionPolicyDecision::allow(VersionRequirement::Upgrade),
            std::cmp::Ordering::Greater => {
                if self.allow_downgrade {
                    return VersionPolicyDecision::allow(VersionRequirement::Downgrade);
                }
                VersionPolicyDecision::deny(
                    VersionRequirement::Downgrade,
                    format!(
                        "downgrade from '{installed}' to '{target}' is not allowed by the version policy"
                    ),
                )
            }
            std::cmp::Ordering::Equal => {
                // Same version, same pre-release. This is
                // a reinstall. Allow for os-image and
                // service-bundle; deny for agent-model.
                match action {
                    UpdateAction::ReinstallOsImage | UpdateAction::ReinstallServiceBundle => {
                        VersionPolicyDecision::allow(VersionRequirement::Same)
                    }
                    UpdateAction::UpgradeOsImage
                    | UpdateAction::UpgradeServiceBundle
                    | UpdateAction::UpgradeAgentModel => {
                        VersionPolicyDecision::deny(
                            VersionRequirement::Same,
                            format!(
                                "target version '{target}' is identical to the installed version for an upgrade"
                            ),
                        )
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_three_part_version() {
        let v = ParsedVersion::parse("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.pre, None);
    }

    #[test]
    fn parses_pre_release() {
        let v = ParsedVersion::parse("1.2.3-rc.1").unwrap();
        assert_eq!(v.pre.as_deref(), Some("rc.1"));
    }

    #[test]
    fn rejects_malformed_version() {
        assert!(ParsedVersion::parse("1.2").is_none());
        assert!(ParsedVersion::parse("1.2.3.4").is_none());
        assert!(ParsedVersion::parse("v1.2.3").is_none());
        assert!(ParsedVersion::parse("1.-2.3").is_none());
        assert!(ParsedVersion::parse("").is_none());
        assert!(ParsedVersion::parse("1.2.3-").is_none());
        assert!(ParsedVersion::parse("1.2.3-!bad").is_none());
    }

    #[test]
    fn display_round_trip() {
        let v = ParsedVersion::parse("1.2.3-rc.1").unwrap();
        assert_eq!(v.to_string(), "1.2.3-rc.1");
    }

    #[test]
    fn upgrade_is_allowed_by_default() {
        let p = VersionPolicy::default();
        let d = p.evaluate(UpdateAction::UpgradeOsImage, "1.0.0", "1.0.1");
        assert!(d.allowed);
        assert_eq!(d.requirement, VersionRequirement::Upgrade);
    }

    #[test]
    fn downgrade_is_denied_by_default() {
        let p = VersionPolicy::default();
        let d = p.evaluate(UpdateAction::UpgradeOsImage, "1.0.1", "1.0.0");
        assert!(!d.allowed);
        assert_eq!(d.requirement, VersionRequirement::Downgrade);
    }

    #[test]
    fn downgrade_is_allowed_with_flag() {
        let p = VersionPolicy::new(true, false);
        let d = p.evaluate(UpdateAction::UpgradeOsImage, "1.0.1", "1.0.0");
        assert!(d.allowed);
        assert_eq!(d.requirement, VersionRequirement::Downgrade);
    }

    #[test]
    fn prerelease_transition_requires_flag() {
        let p = VersionPolicy::default();
        let d = p.evaluate(UpdateAction::UpgradeOsImage, "1.0.0-rc.1", "1.0.0-rc.2");
        assert!(!d.allowed, "prerelease transition should be denied by default: {d:?}");
        assert_eq!(d.requirement, VersionRequirement::Prerelease);
        let p = VersionPolicy::new(false, true);
        let d = p.evaluate(UpdateAction::UpgradeOsImage, "1.0.0-rc.1", "1.0.0-rc.2");
        assert!(d.allowed);
    }

    #[test]
    fn reinstall_os_image_is_allowed_at_same_version() {
        let p = VersionPolicy::default();
        let d = p.evaluate(UpdateAction::ReinstallOsImage, "1.2.3", "1.2.3");
        assert!(d.allowed);
        assert_eq!(d.requirement, VersionRequirement::Same);
    }

    #[test]
    fn reinstall_service_bundle_is_allowed_at_same_version() {
        let p = VersionPolicy::default();
        let d = p.evaluate(UpdateAction::ReinstallServiceBundle, "0.5.0", "0.5.0");
        assert!(d.allowed);
    }

    #[test]
    fn agent_model_same_version_is_denied() {
        // The model runtime never reinstalls; an
        // "upgrade" with the same version is a no-op
        // and should be rejected.
        let p = VersionPolicy::default();
        let d = p.evaluate(UpdateAction::UpgradeAgentModel, "0.1.0", "0.1.0");
        assert!(!d.allowed);
    }

    #[test]
    fn major_upgrade_is_allowed() {
        let p = VersionPolicy::default();
        let d = p.evaluate(UpdateAction::UpgradeServiceBundle, "0.99.99", "1.0.0");
        assert!(d.allowed);
        assert_eq!(d.requirement, VersionRequirement::Upgrade);
    }

    #[test]
    fn unknown_installed_version_is_rejected() {
        let p = VersionPolicy::default();
        let d = p.evaluate(UpdateAction::UpgradeOsImage, "garbage", "1.0.0");
        assert!(!d.allowed);
    }

    #[test]
    fn unknown_target_version_is_rejected() {
        let p = VersionPolicy::default();
        let d = p.evaluate(UpdateAction::UpgradeOsImage, "1.0.0", "garbage");
        assert!(!d.allowed);
    }

    #[test]
    fn requirement_as_str_is_stable() {
        assert_eq!(VersionRequirement::Upgrade.as_str(), "upgrade");
        assert_eq!(VersionRequirement::Downgrade.as_str(), "downgrade");
        assert_eq!(VersionRequirement::Same.as_str(), "same");
        assert_eq!(VersionRequirement::Prerelease.as_str(), "prerelease");
    }
}
