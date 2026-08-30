// Device identity.
//
// `DeviceId` is the stable string identity a
// device carries across reboots, network
// reconnections, and pairing changes. It is
// generated once on first boot and stored in
// the device's sealed credential store; the
// future runtime never re-generates it.
//
// `DeviceClass` is a typed enum so a caller can
// ask "is this a phone?" without a string
// compare. New classes are added only via a
// ROADMAP change; we want the device taxonomy
// to be a stable, reviewable contract.

use serde::{Deserialize, Serialize};

/// The maximum length of a `DeviceId`. Picked
/// to be long enough for human-meaningful
/// names ("vikas-laptop-2026") but short
/// enough to keep the IPC payload small.
pub const DEVICE_ID_MAX_LEN: usize = 64;

/// A unique identifier for a single Aether
/// device. The shell accepts any non-empty
/// string up to `DEVICE_ID_MAX_LEN`; the
/// future device runtime generates a
/// `dev-<uuidv7>`-style identifier on first
/// boot.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeviceId(String);

impl DeviceId {
    /// Creates a new `DeviceId` from a non-empty
    /// string of at most `DEVICE_ID_MAX_LEN`
    /// characters.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let s: String = value.into();
        if s.is_empty() || s.len() > DEVICE_ID_MAX_LEN {
            return None;
        }
        Some(Self(s))
    }

    /// Returns the inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The class of an Aether device. The taxonomy
/// is stable: new classes are added only via a
/// ROADMAP change so a caller can match on a
/// specific variant without fear of breaking
/// the next phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceClass {
    /// A handheld phone-class device. Carries a
    /// touch UI, a microphone, and a battery.
    Phone,
    /// A larger touch-first device (e.g. an
    /// iPad-class tablet). May or may not have
    /// a microphone; battery powered.
    Tablet,
    /// A clamshell laptop. Has a keyboard, a
    /// trackpad, a battery, and a primary
    /// desktop-style UI when docked.
    Laptop,
    /// A desk-bound machine. Mains powered; no
    /// battery; primary UI is a windowed shell.
    Desktop,
    /// A small headless device (smart speaker,
    /// sensor hub, light controller). The
    /// "agent" runs without a user-facing UI;
    /// the future device runtime talks to it
    /// over a low-bandwidth channel.
    Iot,
    /// A server-class machine. Mains powered;
    /// runs services but no user UI.
    Server,
    /// An external display (TV, monitor) that
    /// is paired with a primary device.
    External,
    /// A device whose class is not in the
    /// canonical taxonomy. Reserved for
    /// forward compatibility — the future
    /// runtime may decide to log a warning
    /// rather than reject the device.
    Other,
}

impl DeviceClass {
    /// Returns the canonical kebab-case name.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Phone => "phone",
            Self::Tablet => "tablet",
            Self::Laptop => "laptop",
            Self::Desktop => "desktop",
            Self::Iot => "iot",
            Self::Server => "server",
            Self::External => "external",
            Self::Other => "other",
        }
    }

    /// Returns `true` for classes that are
    /// typically battery-powered.
    #[must_use]
    pub fn is_battery_powered(&self) -> bool {
        matches!(self, Self::Phone | Self::Tablet | Self::Laptop)
    }

    /// Returns `true` for classes that have a
    /// primary user-facing UI.
    #[must_use]
    pub fn has_user_ui(&self) -> bool {
        matches!(self, Self::Phone | Self::Tablet | Self::Laptop | Self::Desktop)
    }
}

impl std::fmt::Display for DeviceClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_device_id_only_for_non_empty_and_bounded() {
        assert!(DeviceId::new("").is_none());
        assert!(DeviceId::new("vikas-laptop").is_some());
        let too_long = "a".repeat(DEVICE_ID_MAX_LEN + 1);
        assert!(DeviceId::new(too_long).is_none());
        let just_right = "a".repeat(DEVICE_ID_MAX_LEN);
        assert!(DeviceId::new(just_right).is_some());
    }

    #[test]
    fn device_class_as_str_is_stable() {
        assert_eq!(DeviceClass::Phone.as_str(), "phone");
        assert_eq!(DeviceClass::Tablet.as_str(), "tablet");
        assert_eq!(DeviceClass::Laptop.as_str(), "laptop");
        assert_eq!(DeviceClass::Desktop.as_str(), "desktop");
        assert_eq!(DeviceClass::Iot.as_str(), "iot");
        assert_eq!(DeviceClass::Server.as_str(), "server");
        assert_eq!(DeviceClass::External.as_str(), "external");
        assert_eq!(DeviceClass::Other.as_str(), "other");
    }

    #[test]
    fn is_battery_powered_matches_handhelds_and_laptops() {
        assert!(DeviceClass::Phone.is_battery_powered());
        assert!(DeviceClass::Tablet.is_battery_powered());
        assert!(DeviceClass::Laptop.is_battery_powered());
        assert!(!DeviceClass::Desktop.is_battery_powered());
        assert!(!DeviceClass::Iot.is_battery_powered());
        assert!(!DeviceClass::Server.is_battery_powered());
    }

    #[test]
    fn has_user_ui_matches_primary_devices() {
        assert!(DeviceClass::Phone.has_user_ui());
        assert!(DeviceClass::Tablet.has_user_ui());
        assert!(DeviceClass::Laptop.has_user_ui());
        assert!(DeviceClass::Desktop.has_user_ui());
        assert!(!DeviceClass::Iot.has_user_ui());
        assert!(!DeviceClass::Server.has_user_ui());
        assert!(!DeviceClass::External.has_user_ui());
    }
}
