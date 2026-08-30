// Permission helpers for third-party Aether apps.
//
// The 11 `AppPermission` variants in `aether-core` are the
// developer-facing surface ("my app needs to send
// notifications, read user files, and use the camera").
// Each one maps to a `Capability` with a `RiskLevel` via
// `app_permission_capability`. Apps that want to render a
// pre-install preview of what they're asking for can use
// `permissions_to_capabilities` to get the canonical
// `(permission, capability)` pairs side by side.
//
// This module is intentionally thin — it re-exports the
// types and the security crate's mapping so the SDK has a
// single import path for everything a third-party app
// would need to reason about its own permission set.

use aether_core::app::AppPermission;
use aether_security::app_security::app_permission_capability;

/// A `(permission, capability)` pair. Apps use this to
/// render a human-readable consent prompt at install time
/// (the system renders its own; the SDK exposes this so
/// apps can preview the same list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    /// The high-level permission the app is asking for.
    pub permission: AppPermission,
    /// The capability the system records internally,
    /// including the risk level the dispatcher will use
    /// to decide allow / require-consent / deny.
    pub capability: aether_core::Capability,
}

impl PermissionRequest {
    /// Risk level the dispatcher applies to this
    /// permission. Convenience accessor.
    #[must_use]
    pub fn risk_level(&self) -> aether_core::RiskLevel {
        self.capability.risk_level
    }
}

/// Map a slice of `AppPermission` to the corresponding
/// `PermissionRequest` list. The order matches the input;
/// duplicates are preserved as-is (callers can dedupe
/// before calling if they care about the UI).
#[must_use]
pub fn permissions_to_capabilities(perms: &[AppPermission]) -> Vec<PermissionRequest> {
    perms
        .iter()
        .map(|p| PermissionRequest { permission: *p, capability: app_permission_capability(*p) })
        .collect()
}

/// Convenience: number of permissions the app is asking
/// for. Just `permissions.len()`, but the name is clearer
/// at call sites that render a "X permissions requested"
/// header.
#[must_use]
pub fn count_permissions(perms: &[AppPermission]) -> usize {
    perms.len()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_core::{CapabilityDomain, RiskLevel};

    #[test]
    fn notify_is_low_risk() {
        let reqs = permissions_to_capabilities(&[AppPermission::Notify]);
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].permission, AppPermission::Notify);
        assert_eq!(reqs[0].capability.domain, CapabilityDomain::Application);
        assert_eq!(reqs[0].capability.risk_level, RiskLevel::Low);
    }

    #[test]
    fn camera_is_high_risk() {
        let reqs = permissions_to_capabilities(&[AppPermission::Camera]);
        assert_eq!(reqs[0].capability.risk_level, RiskLevel::High);
    }

    #[test]
    fn capture_screen_is_critical() {
        let reqs = permissions_to_capabilities(&[AppPermission::CaptureScreen]);
        assert_eq!(reqs[0].capability.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn all_eleven_permissions_have_a_capability() {
        // Sanity: every variant maps to something. If
        // a new variant is added to `AppPermission` and
        // the security crate forgets to handle it, this
        // will surface a fallback capability rather
        // than a panic.
        let all = [
            AppPermission::Notify,
            AppPermission::ReadUserFiles,
            AppPermission::WriteUserFiles,
            AppPermission::NetworkEgress,
            AppPermission::NetworkListen,
            AppPermission::ReadPersonalData,
            AppPermission::PairDevices,
            AppPermission::Camera,
            AppPermission::Microphone,
            AppPermission::Location,
            AppPermission::CaptureScreen,
        ];
        assert_eq!(all.len(), 11);
        let reqs = permissions_to_capabilities(&all);
        assert_eq!(reqs.len(), 11);
    }

    #[test]
    fn count_permissions_matches_len() {
        let perms = [AppPermission::Notify, AppPermission::NetworkEgress];
        assert_eq!(count_permissions(&perms), 2);
    }

    #[test]
    fn duplicates_preserved() {
        // We do NOT dedupe here — the caller asked for
        // a list, we give them a list.
        let perms = [AppPermission::Notify, AppPermission::Notify];
        let reqs = permissions_to_capabilities(&perms);
        assert_eq!(reqs.len(), 2);
    }
}
