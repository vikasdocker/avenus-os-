// Tamper-evident boot-measurement chain (Phase 11.9).
//
// A `BootMeasurementChain` is the root-of-trust companion to
// `AuditChain`. The audit chain records *runtime* IPC events
// after the system is up; the boot chain records *boot-time*
// artifacts in the order they are encountered, so a verifier
// reading the chain from the start can answer:
//
//   "Did the kernel command line, the loaded initramfs, the
//    active kernel modules, and the registered service
//    manifests match what the operator expected at the
//    last-known-good state?"
//
// Design rules:
//
//   1. The chain is content-addressed: every measurement is
//      a SHA-256 over a small canonical byte buffer. The
//      verifier recomputes the digest for each entry and
//      rejects on mismatch.
//
//   2. The chain is append-only with a fixed genesis
//      (`GENESIS_PREV_HASH`). Adding an entry with a
//      non-zero `prev_hash` is a programming error — the
//      chain is the only authority on ordering.
//
//   3. Stages are typed. A `BootStage` is one of:
//      * `KernelCommandLine` — the canonicalised kernel
//        cmdline string (after `init=` substitution).
//      * `InitramfsComponent` — the SHA-256 of one component
//        in the initramfs (a binary, a config file, a
//        manifest). The component name is recorded so the
//        operator can read the chain by name.
//      * `KernelModule` — the SHA-256 of one loaded kernel
//        module's `.ko` bytes plus the module name.
//      * `ServiceManifest` — the canonical JSON bytes of
//        one registered service manifest, plus the
//        service id.
//      * `BootComplete` — a marker entry recorded when the
//        init reaches the multi-user target. The marker
//        carries the audit-chain genesis hash so the boot
//        chain binds to the runtime chain.
//
//   4. The chain is bounded by `max_entries` and
//      `max_age_ms` (same model as the audit chain).
//      Pruning rewrites the new head's `prev_hash` to
//      `GENESIS_PREV_HASH` so the chain remains
//      self-consistent.
//
// Out of scope (lives in the init / supervisor binary):
//   * Reading /proc/cmdline, walking /sys/module, parsing
//     initramfs contents. This module is a pure data type.
//   * The actual hash of the running kernel image (a future
//     `MeasuredBoot` shim in `aether-init` can produce one).
//   * TPM / PCR sealing. The chain is software-only; a
//     future revision binds the genesis hash to a TPM PCR.
//
// Threading: the type is `Send + Sync` (it holds only
// owned `Vec`/`u64`/`u32` data). The chain is mutated
// through `&mut self`.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::audit::GENESIS_PREV_HASH;

/// A single boot-time measurement. The `payload` is the
/// SHA-256 of the measured artifact; the chain records the
/// digest, not the artifact itself. The `name` is a
/// human-readable label so the chain reads as a list of
/// "what was loaded when" rather than a list of hex
/// strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootMeasurement {
    /// Monotonically increasing index within the chain.
    pub index: u64,
    /// `prev_hash` of the previous entry, or
    /// `GENESIS_PREV_HASH` for `index == 0`.
    pub prev_hash: [u8; 32],
    /// SHA-256 of the measured artifact (kernel cmdline
    /// canonical bytes, initramfs component, kernel module
    /// bytes, or service manifest canonical bytes). 32
    /// bytes, hex-encoded in the JSON wire format.
    pub payload: [u8; 32],
    /// SHA-256 of every other field in this entry,
    /// including `prev_hash`. The verifier recomputes it
    /// from the stored fields and compares.
    pub content_hash: [u8; 32],
    /// Categorical stage tag.
    pub stage: BootStage,
    /// Human-readable name. For initramfs components this
    /// is the relative path; for kernel modules the module
    /// name; for service manifests the service id. For the
    /// kernel cmdline and the boot-complete marker, the
    /// name is empty.
    pub name: String,
    /// Optional free-form note (e.g. the human-readable
    /// kernel cmdline string after `init=` substitution).
    pub note: String,
    /// Wall-clock timestamp the measurement was recorded.
    /// The chain trusts the caller to supply it; `record`
    /// does not read the clock.
    pub timestamp_ms: u64,
}

/// The categorical stage of a measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BootStage {
    /// The kernel command line, canonicalised.
    KernelCommandLine,
    /// A single component inside the initramfs (binary,
    /// config, manifest, etc).
    InitramfsComponent,
    /// A single kernel module loaded by the init / supervisor.
    KernelModule,
    /// A single registered service manifest.
    ServiceManifest,
    /// A marker entry recorded at the end of the chain.
    /// The note carries the audit-chain genesis hash so
    /// the boot chain binds to the runtime chain.
    BootComplete,
}

impl BootStage {
    /// Canonical kebab-case wire name. Stable for the
    /// lifetime of the chain.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::KernelCommandLine => "kernel-command-line",
            Self::InitramfsComponent => "initramfs-component",
            Self::KernelModule => "kernel-module",
            Self::ServiceManifest => "service-manifest",
            Self::BootComplete => "boot-complete",
        }
    }
}

impl BootMeasurement {
    /// Returns the canonical byte representation that is
    /// hashed into `content_hash`. The serialization is
    /// deterministic and field-order-independent from the
    /// caller's perspective: it is a fixed `concat` of the
    /// fields in declaration order, with the index and
    /// timestamp in big-endian form.
    fn canonical_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(
            8 + 8 + 32 + 32 + self.stage.as_str().len() + self.name.len() + self.note.len() + 3,
        );
        buf.extend_from_slice(&self.index.to_be_bytes());
        buf.extend_from_slice(&self.timestamp_ms.to_be_bytes());
        buf.extend_from_slice(&self.prev_hash);
        buf.extend_from_slice(&self.payload);
        buf.extend_from_slice(self.stage.as_str().as_bytes());
        buf.push(0);
        buf.extend_from_slice(self.name.as_bytes());
        buf.push(0);
        buf.extend_from_slice(self.note.as_bytes());
        buf.push(0);
        buf
    }

    /// Recomputes the content hash from the entry's other
    /// fields. The result is what `content_hash` should
    /// hold for the entry to be authentic.
    #[must_use]
    pub fn recompute_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(self.canonical_bytes());
        let digest = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }
}

/// Retention policy for the boot chain. Same model as
/// `RetentionPolicy`; the type is separate so the boot
/// chain can be tuned independently of the runtime audit
/// chain.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct BootRetention {
    /// Hard cap on stored entries. `0` means unbounded.
    pub max_entries: usize,
    /// Soft cap based on age. Pruning drops entries with
    /// `timestamp_ms < now_ms - max_age_ms`. `0` disables
    /// time-based pruning.
    pub max_age_ms: u64,
}

impl Default for BootRetention {
    fn default() -> Self {
        // A desktop OS boots ~once per day; a server boots
        // ~once per month. 256 entries is roughly 8 months
        // of daily boots — well within the audit-log
        // retention budget.
        Self { max_entries: 256, max_age_ms: 0 }
    }
}

impl BootRetention {
    /// A policy that retains the most recent `n` entries
    /// and drops nothing by age.
    #[must_use]
    pub const fn last_n(n: usize) -> Self {
        Self { max_entries: n, max_age_ms: 0 }
    }
}

/// Result of a chain verification pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootChainStatus {
    /// The chain is intact through every entry.
    Ok,
    /// The entry at `index` has a stored `content_hash`
    /// that does not match the recomputed hash.
    ContentMismatch { index: u64 },
    /// The entry at `index` has a `prev_hash` that does
    /// not match the previous entry's `content_hash`.
    BrokenLink { index: u64 },
    /// The entry at `index` is missing — the chain's
    /// stored indices have a gap.
    IndexGap { index: u64 },
    /// The chain does not end in a `BootComplete` marker.
    /// This is informational, not a tamper: the chain is
    /// in-progress while the system is up.
    MissingBootComplete { last_index: u64 },
}

/// The tamper-evident boot chain itself.
#[derive(Debug, Clone)]
pub struct BootMeasurementChain {
    entries: Vec<BootMeasurement>,
    retention: BootRetention,
}

impl BootMeasurementChain {
    /// Creates a new empty boot chain with the given
    /// retention policy.
    #[must_use]
    pub fn new(retention: BootRetention) -> Self {
        Self { entries: Vec::new(), retention }
    }

    /// Returns the number of entries currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the chain has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the configured retention policy.
    #[must_use]
    pub fn retention(&self) -> BootRetention {
        self.retention
    }

    /// Records a new measurement. `prev_hash` and
    /// `content_hash` are computed by the chain; the caller
    /// only supplies the human-meaningful fields.
    ///
    /// If `retention.max_entries` is non-zero and the chain
    /// is at capacity, the oldest entry is evicted first to
    /// make room.
    pub fn record(
        &mut self,
        timestamp_ms: u64,
        stage: BootStage,
        name: impl Into<String>,
        note: impl Into<String>,
        payload: [u8; 32],
    ) -> u64 {
        if self.retention.max_entries != 0 && self.entries.len() >= self.retention.max_entries {
            self.entries.remove(0);
        }
        let prev_hash = self.entries.last().map_or(GENESIS_PREV_HASH, |e| e.content_hash);
        let index = self.entries.len() as u64;
        let mut entry = BootMeasurement {
            index,
            timestamp_ms,
            prev_hash,
            payload,
            content_hash: [0u8; 32],
            stage,
            name: name.into(),
            note: note.into(),
        };
        entry.content_hash = entry.recompute_hash();
        self.entries.push(entry);
        index
    }

    /// Returns the entries in chronological order.
    #[must_use]
    pub fn entries(&self) -> &[BootMeasurement] {
        &self.entries
    }

    /// Walks the entire chain. Returns the first
    /// inconsistency found, or `Ok(())` if every entry's
    /// stored `content_hash` and `prev_hash` are valid AND
    /// the chain ends in a `BootComplete` marker.
    ///
    /// To verify a chain that is still in-progress (no
    /// `BootComplete` marker yet), use
    /// `verify_chain_lenient` instead.
    pub fn verify_chain(&self) -> Result<(), BootChainStatus> {
        self.verify_chain_inner(true)
    }

    /// Lenient verifier: returns `Ok(())` if every
    /// entry's hash is intact; returns
    /// `MissingBootComplete` (informational, not an
    /// error) if the chain is in-progress.
    pub fn verify_chain_lenient(&self) -> Result<(), BootChainStatus> {
        self.verify_chain_inner(false)
    }

    fn verify_chain_inner(&self, require_complete: bool) -> Result<(), BootChainStatus> {
        let mut expected_prev = GENESIS_PREV_HASH;
        for (expected_index, entry) in (0u64..).zip(self.entries.iter()) {
            if entry.index != expected_index {
                return Err(BootChainStatus::IndexGap { index: entry.index });
            }
            if entry.prev_hash != expected_prev {
                return Err(BootChainStatus::BrokenLink { index: entry.index });
            }
            let recomputed = entry.recompute_hash();
            if recomputed != entry.content_hash {
                return Err(BootChainStatus::ContentMismatch { index: entry.index });
            }
            expected_prev = entry.content_hash;
        }
        if require_complete
            && !self.entries.last().is_some_and(|e| e.stage == BootStage::BootComplete)
        {
            return Err(BootChainStatus::MissingBootComplete {
                last_index: self.entries.len().saturating_sub(1) as u64,
            });
        }
        Ok(())
    }

    /// Drops entries older than `now_ms - max_age_ms`. The
    /// remaining head's `prev_hash` is rewritten to
    /// `GENESIS_PREV_HASH` and its `content_hash` is
    /// recomputed so the chain remains self-consistent.
    ///
    /// Returns the number of entries that were dropped.
    pub fn prune_older_than(&mut self, now_ms: u64) -> usize {
        if self.retention.max_age_ms == 0 {
            return 0;
        }
        let cutoff = now_ms.saturating_sub(self.retention.max_age_ms);
        let keep_from = self.entries.iter().position(|e| e.timestamp_ms >= cutoff);
        let Some(keep_from) = keep_from else {
            let dropped = self.entries.len();
            self.entries.clear();
            return dropped;
        };
        if keep_from == 0 {
            return 0;
        }
        let dropped = keep_from;
        self.entries.drain(..keep_from);
        // Re-stamp the new head so its prev_hash points
        // at genesis.
        let mut prev_hash = GENESIS_PREV_HASH;
        for (offset, entry) in self.entries.iter_mut().enumerate() {
            entry.index = offset as u64;
            entry.prev_hash = prev_hash;
            entry.content_hash = entry.recompute_hash();
            prev_hash = entry.content_hash;
        }
        dropped
    }
}

impl Default for BootMeasurementChain {
    fn default() -> Self {
        Self::new(BootRetention::default())
    }
}

/// Hash the canonical kernel command-line bytes. The
/// kernel parses the cmdline into a list of `key=value`
/// pairs; the helper canonicalises the pair list so two
/// cmdlines that differ only in argument order hash to the
/// same digest.
#[must_use]
pub fn kernel_cmdline_digest(cmdline: &str) -> [u8; 32] {
    let mut parts: Vec<&str> = cmdline.split_whitespace().collect();
    parts.sort_unstable();
    let mut hasher = Sha256::new();
    for p in parts {
        hasher.update(p.as_bytes());
        hasher.update([0u8]);
    }
    hasher.finalize().into()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn sha256(bytes: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hasher.finalize().into()
    }

    #[test]
    fn empty_chain_verifies() {
        let chain = BootMeasurementChain::default();
        assert!(chain.verify_chain_lenient().is_ok());
    }

    #[test]
    fn empty_chain_is_not_complete() {
        let chain = BootMeasurementChain::default();
        let status = chain.verify_chain().unwrap_err();
        assert!(matches!(status, BootChainStatus::MissingBootComplete { last_index: 0 }));
    }

    #[test]
    fn record_assigns_increasing_indices() {
        let mut chain = BootMeasurementChain::new(BootRetention::last_n(10));
        let cmdline = sha256(b"init=/sbin/aether-init");
        let i0 = chain.record(1000, BootStage::KernelCommandLine, "", "", cmdline);
        let module = sha256(b"aether_supervisor.ko");
        let i1 = chain.record(2000, BootStage::KernelModule, "aether_supervisor", "", module);
        assert_eq!(i0, 0);
        assert_eq!(i1, 1);
        assert_eq!(chain.len(), 2);
    }

    #[test]
    fn record_sets_prev_hash_to_previous_content_hash() {
        let mut chain = BootMeasurementChain::new(BootRetention::last_n(10));
        chain.record(1000, BootStage::KernelCommandLine, "", "", [1u8; 32]);
        chain.record(2000, BootStage::KernelModule, "a", "", [2u8; 32]);
        let entries = chain.entries();
        assert_eq!(entries[0].prev_hash, GENESIS_PREV_HASH);
        assert_eq!(entries[1].prev_hash, entries[0].content_hash);
    }

    #[test]
    fn chain_verifies_after_many_inserts() {
        let mut chain = BootMeasurementChain::new(BootRetention::last_n(200));
        for i in 0..100u64 {
            chain.record(10_000 + i, BootStage::InitramfsComponent, format!("comp-{i}"), "", [i as u8; 32]);
        }
        assert_eq!(chain.len(), 100);
        assert!(chain.verify_chain_lenient().is_ok());
    }

    #[test]
    fn detect_content_tampering() {
        let mut chain = BootMeasurementChain::new(BootRetention::last_n(10));
        for i in 0..5u64 {
            chain.record(1000 + i, BootStage::InitramfsComponent, format!("c-{i}"), "", [i as u8; 32]);
        }
        chain.entries[2].payload = [0xAB; 32];
        let status = chain.verify_chain_lenient().unwrap_err();
        assert_eq!(status, BootChainStatus::ContentMismatch { index: 2 });
    }

    #[test]
    fn detect_broken_link_tampering() {
        let mut chain = BootMeasurementChain::new(BootRetention::last_n(10));
        for i in 0..5u64 {
            chain.record(1000 + i, BootStage::InitramfsComponent, format!("c-{i}"), "", [i as u8; 32]);
        }
        chain.entries[3].prev_hash = [0xCD; 32];
        let status = chain.verify_chain_lenient().unwrap_err();
        assert_eq!(status, BootChainStatus::BrokenLink { index: 3 });
    }

    #[test]
    fn detect_index_gap_tampering() {
        let mut chain = BootMeasurementChain::new(BootRetention::last_n(10));
        for i in 0..5u64 {
            chain.record(1000 + i, BootStage::InitramfsComponent, format!("c-{i}"), "", [i as u8; 32]);
        }
        chain.entries[2].index = 99;
        let status = chain.verify_chain_lenient().unwrap_err();
        assert_eq!(status, BootChainStatus::IndexGap { index: 99 });
    }

    #[test]
    fn chain_verifies_when_complete_marker_present() {
        let mut chain = BootMeasurementChain::new(BootRetention::last_n(10));
        chain.record(1, BootStage::KernelCommandLine, "", "", [1; 32]);
        chain.record(2, BootStage::InitramfsComponent, "aether-init", "", [2; 32]);
        chain.record(3, BootStage::ServiceManifest, "aether-system-core", "", [3; 32]);
        // The audit-chain genesis hash binds the boot
        // chain to the runtime chain.
        chain.record(4, BootStage::BootComplete, "", "audit-genesis=deadbeef", [4; 32]);
        chain.verify_chain().expect("complete chain verifies");
    }

    #[test]
    fn chain_is_not_complete_without_marker() {
        let mut chain = BootMeasurementChain::new(BootRetention::last_n(10));
        chain.record(1, BootStage::KernelCommandLine, "", "", [1; 32]);
        let status = chain.verify_chain().unwrap_err();
        assert!(matches!(status, BootChainStatus::MissingBootComplete { .. }));
    }

    #[test]
    fn record_evicts_oldest_when_at_capacity() {
        let mut chain = BootMeasurementChain::new(BootRetention::last_n(3));
        for i in 0..5u64 {
            chain.record(1000 + i, BootStage::InitramfsComponent, format!("c-{i}"), "", [i as u8; 32]);
        }
        assert_eq!(chain.len(), 3);
        let entries = chain.entries();
        assert_eq!(entries[0].name, "c-2");
        assert_eq!(entries[2].name, "c-4");
    }

    #[test]
    fn prune_drops_expired_and_rewrites_head() {
        let mut chain = BootMeasurementChain::new(BootRetention { max_entries: 100, max_age_ms: 2_500 });
        chain.record(1_000, BootStage::InitramfsComponent, "c-0", "", [0; 32]);
        chain.record(2_000, BootStage::InitramfsComponent, "c-1", "", [1; 32]);
        chain.record(3_000, BootStage::InitramfsComponent, "c-2", "", [2; 32]);
        let dropped = chain.prune_older_than(4_000);
        assert_eq!(dropped, 1);
        assert_eq!(chain.len(), 2);
        let head = &chain.entries()[0];
        assert_eq!(head.name, "c-1");
        assert_eq!(head.index, 0);
        assert_eq!(head.prev_hash, GENESIS_PREV_HASH);
        assert!(chain.verify_chain_lenient().is_ok());
    }

    #[test]
    fn prune_with_zero_max_age_is_a_noop() {
        let mut chain = BootMeasurementChain::new(BootRetention::last_n(10));
        chain.record(1_000, BootStage::InitramfsComponent, "c-0", "", [0; 32]);
        let dropped = chain.prune_older_than(100_000);
        assert_eq!(dropped, 0);
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn kernel_cmdline_digest_is_canonical() {
        // Two cmdlines that differ only in argument order
        // hash to the same digest.
        let a = kernel_cmdline_digest("root=/dev/sda1 ro quiet");
        let b = kernel_cmdline_digest("quiet ro root=/dev/sda1");
        assert_eq!(a, b);
    }

    #[test]
    fn kernel_cmdline_digest_differs_for_different_args() {
        let a = kernel_cmdline_digest("root=/dev/sda1 ro");
        let b = kernel_cmdline_digest("root=/dev/sda2 ro");
        assert_ne!(a, b);
    }

    #[test]
    fn boot_stage_as_str_is_stable() {
        assert_eq!(BootStage::KernelCommandLine.as_str(), "kernel-command-line");
        assert_eq!(BootStage::InitramfsComponent.as_str(), "initramfs-component");
        assert_eq!(BootStage::KernelModule.as_str(), "kernel-module");
        assert_eq!(BootStage::ServiceManifest.as_str(), "service-manifest");
        assert_eq!(BootStage::BootComplete.as_str(), "boot-complete");
    }

    #[test]
    fn entry_round_trips_through_serde_json() {
        let mut chain = BootMeasurementChain::new(BootRetention::last_n(10));
        chain.record(1000, BootStage::KernelCommandLine, "", "root=/dev/sda1", [7; 32]);
        let entry = &chain.entries()[0];
        let text = serde_json::to_string(entry).expect("encode");
        let back: BootMeasurement = serde_json::from_str(&text).expect("decode");
        assert_eq!(back, *entry);
    }

    #[test]
    fn canonical_bytes_contain_stage_marker() {
        let mut chain = BootMeasurementChain::new(BootRetention::last_n(1));
        chain.record(42, BootStage::KernelCommandLine, "k", "n", [1; 32]);
        let bytes = chain.entries()[0].canonical_bytes();
        // The fixed-field prefix is 8 (index) + 8 (timestamp)
        // + 32 (prev_hash) + 32 (payload) = 80 bytes. The
        // stage marker ("kernel-command-line", 19 chars)
        // begins at byte 80 and is followed by a NUL, then
        // the name, then a NUL, then the note, then a NUL.
        let stage_start = 80;
        let stage_len = BootStage::KernelCommandLine.as_str().len();
        let stage = std::str::from_utf8(&bytes[stage_start..stage_start + stage_len])
            .expect("stage is utf8");
        assert_eq!(stage, BootStage::KernelCommandLine.as_str());
        assert_eq!(bytes[stage_start + stage_len], 0u8);
        // The name "k" follows the NUL.
        assert_eq!(bytes[stage_start + stage_len + 1], b'k');
    }
}
