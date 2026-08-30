// Trusted-publisher registry for the Aether Store.
//
// A publisher's *identity* in the Aether Store is the SHA-256
// fingerprint of their Ed25519 public key, hex-encoded. The
// application manager looks the fingerprint up in the
// trusted-publisher registry to decide whether a given package
// may even be considered for install. Without a matching entry,
// the manifest is rejected before any consent prompt appears.
//
// The registry is a small JSON file on disk. In production the
// file is rooted under `/var/lib/aether/store/trust.json` (or the
// configured `root`) and is updated by the OS update channel when
// the OS vendor wants to add a new publisher. The store's unit
// tests construct the registry inline.
//
// Each `PublisherTrust` entry carries the fingerprint plus
// optional metadata: a human-readable display name and a free-form
// notes string the registry maintainer may use to record the
// reason the publisher was added.

use serde::{Deserialize, Serialize};

use crate::fs::StoreFs;

/// The on-disk filename for the trust registry. The Store reads
/// this on construction if it exists; missing files are treated as
/// "no trusted publishers", which causes every install to be
/// rejected — the safe default.
pub const TRUST_FILE: &str = "trust.json";

/// A single trusted-publisher record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublisherTrust {
    /// The hex SHA-256 fingerprint of the publisher's Ed25519
    /// public key. Matches `AppManifest::publisher_key_id`.
    pub fingerprint: String,
    /// Human-readable display name (e.g. "Example Org").
    /// Optional; the registry may carry fingerprints for keys
    /// whose owner name has not yet been resolved.
    pub display_name: Option<String>,
    /// Free-form notes. Optional. The application manager writes
    /// this verbatim into the audit log entry that records the
    /// registry change.
    pub notes: Option<String>,
}

impl PublisherTrust {
    /// Construct a minimal record with just the fingerprint.
    /// The display name and notes are unset.
    #[must_use]
    pub fn new(fingerprint: impl Into<String>) -> Self {
        Self { fingerprint: fingerprint.into(), display_name: None, notes: None }
    }

    /// Builder: set the display name.
    #[must_use]
    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    /// Builder: set the notes.
    #[must_use]
    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }
}

/// The trust registry itself. Persists to `TRUST_FILE` via
/// `StoreFs`; tests construct one inline.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedPublisherRegistry {
    publishers: Vec<PublisherTrust>,
}

impl TrustedPublisherRegistry {
    /// An empty registry (no trusted publishers). Every install
    /// is rejected — this is the safe default for a freshly
    /// provisioned system.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Build a registry from a list of `PublisherTrust` records.
    /// The list is deduplicated by fingerprint and sorted
    /// lexicographically so the on-disk representation is stable.
    #[must_use]
    pub fn from_publishers(mut publishers: Vec<PublisherTrust>) -> Self {
        publishers.sort_by(|a, b| a.fingerprint.cmp(&b.fingerprint));
        publishers.dedup_by(|a, b| a.fingerprint == b.fingerprint);
        Self { publishers }
    }

    /// Load the registry from `TRUST_FILE` via `fs`. If the file
    /// is missing, returns an empty registry (and does NOT
    /// create the file). If the file is present but malformed,
    /// returns an error; the Store refuses to start with a
    /// tampered trust file.
    ///
    /// # Errors
    /// Returns `Err` if the trust file exists but cannot be
    /// parsed as JSON.
    pub fn load(fs: &dyn StoreFs) -> Result<Self, String> {
        if !fs.exists(TRUST_FILE) {
            return Ok(Self::empty());
        }
        let bytes = fs.read(TRUST_FILE)?;
        serde_json::from_slice(&bytes).map_err(|e| format!("parse {TRUST_FILE}: {e}"))
    }

    /// Persist the registry to `TRUST_FILE` via `fs`. The
    /// directory containing the file is created if needed.
    ///
    /// # Errors
    /// Returns `Err` on I/O failure.
    pub fn save(&self, fs: &mut dyn StoreFs) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(self).map_err(|e| format!("encode trust: {e}"))?;
        fs.write(TRUST_FILE, &json)
    }

    /// Returns `true` if `fingerprint` is in the registry.
    #[must_use]
    pub fn contains(&self, fingerprint: &str) -> bool {
        self.publishers.iter().any(|p| p.fingerprint == fingerprint)
    }

    /// Returns the publisher record for `fingerprint`, or `None`.
    #[must_use]
    pub fn get(&self, fingerprint: &str) -> Option<&PublisherTrust> {
        self.publishers.iter().find(|p| p.fingerprint == fingerprint)
    }

    /// Adds a publisher. The registry is kept sorted and
    /// deduplicated.
    pub fn add(&mut self, publisher: PublisherTrust) {
        self.publishers.push(publisher);
        self.publishers.sort_by(|a, b| a.fingerprint.cmp(&b.fingerprint));
        self.publishers.dedup_by(|a, b| a.fingerprint == b.fingerprint);
    }

    /// Removes a publisher by fingerprint. Returns `true` if a
    /// publisher was removed.
    pub fn remove(&mut self, fingerprint: &str) -> bool {
        let before = self.publishers.len();
        self.publishers.retain(|p| p.fingerprint != fingerprint);
        self.publishers.len() != before
    }

    /// Returns the publishers in lexicographic fingerprint order.
    #[must_use]
    pub fn publishers(&self) -> &[PublisherTrust] {
        &self.publishers
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::fs::MemoryFs;

    #[test]
    fn empty_registry_rejects_anything() {
        let r = TrustedPublisherRegistry::empty();
        assert!(!r.contains("anything"));
    }

    #[test]
    fn add_and_contains() {
        let mut r = TrustedPublisherRegistry::empty();
        r.add(PublisherTrust::new("a".repeat(64)));
        assert!(r.contains(&"a".repeat(64)));
        assert!(!r.contains(&"b".repeat(64)));
    }

    #[test]
    fn add_dedupes_and_sorts() {
        let mut r = TrustedPublisherRegistry::empty();
        r.add(PublisherTrust::new("b".repeat(64)));
        r.add(PublisherTrust::new("a".repeat(64)));
        r.add(PublisherTrust::new("a".repeat(64))); // dup
        let fps: Vec<String> = r.publishers().iter().map(|p| p.fingerprint.clone()).collect();
        assert_eq!(fps, vec!["a".repeat(64), "b".repeat(64)]);
    }

    #[test]
    fn from_publishers_dedupes_and_sorts() {
        let pubs = vec![
            PublisherTrust::new("z".repeat(64)).with_display_name("Zeta"),
            PublisherTrust::new("a".repeat(64)),
            PublisherTrust::new("a".repeat(64)),
        ];
        let r = TrustedPublisherRegistry::from_publishers(pubs);
        let fps: Vec<String> = r.publishers().iter().map(|p| p.fingerprint.clone()).collect();
        assert_eq!(fps, vec!["a".repeat(64), "z".repeat(64)]);
    }

    #[test]
    fn remove_returns_true_when_present() {
        let mut r = TrustedPublisherRegistry::empty();
        r.add(PublisherTrust::new("a".repeat(64)));
        assert!(r.remove(&"a".repeat(64)));
        assert!(!r.contains(&"a".repeat(64)));
        assert!(!r.remove(&"a".repeat(64)));
    }

    #[test]
    fn save_and_load_round_trip() {
        let mut r = TrustedPublisherRegistry::empty();
        r.add(PublisherTrust::new("a".repeat(64)).with_display_name("Acme"));
        let mut fs = MemoryFs::new();
        r.save(&mut fs).expect("save");
        let loaded = TrustedPublisherRegistry::load(&fs).expect("load");
        assert_eq!(loaded, r);
    }

    #[test]
    fn load_missing_file_returns_empty() {
        let fs = MemoryFs::new();
        let r = TrustedPublisherRegistry::load(&fs).expect("missing file = empty");
        assert!(r.publishers().is_empty());
    }

    #[test]
    fn load_malformed_file_errors() {
        let mut fs = MemoryFs::new();
        fs.write(TRUST_FILE, b"not-json").expect("write");
        let err = TrustedPublisherRegistry::load(&fs).expect_err("malformed must error");
        assert!(err.contains("parse"), "{err}");
    }

    #[test]
    fn display_name_and_notes_are_preserved() {
        let p = PublisherTrust::new("a".repeat(64))
            .with_display_name("Acme")
            .with_notes("mainstream publisher");
        assert_eq!(p.display_name.as_deref(), Some("Acme"));
        assert_eq!(p.notes.as_deref(), Some("mainstream publisher"));
    }
}
