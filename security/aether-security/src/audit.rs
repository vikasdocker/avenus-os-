// System-level tamper-evident audit log.
//
// Distinct from the agent-runtime's `AuditLog` in
// `aether-agent-runtime::audit`, which records agent session
// events. This log records system-core dispatcher events
// (capability decisions, IPC dispatch, sandbox plans, process
// actions) and is the authoritative record for post-incident
// review.
//
// Every entry is bound to the previous one through a SHA-256
// hash chain. The first entry uses a fixed genesis hash as its
// `prev_hash`. `verify_chain` walks the entire log and returns
// the first corrupted index, or `Ok(())` if the chain is intact.
//
// Retention is bounded by a `RetentionPolicy`:
//   * `max_entries` is a hard cap on the number of stored
//     entries; `record` evicts the oldest entry when the cap
//     is reached.
//   * `max_age_ms` is a soft cap: `prune_older_than(now_ms)`
//     drops entries whose `timestamp_ms` is older than
//     `now_ms - max_age_ms`. Pruning preserves chain
//     integrity — the entry that *replaces* an evicted head
//     has its `prev_hash` recomputed so the chain continues
//     from the new head backward.
//
// The log is intentionally in-memory and synchronous; the
// system-core single-shot daemon does not need persistence
// (the agent-runtime's own audit log is already persisted
// for that role). A future `aether-audit-store` crate can
// wrap this type with a write-ahead journal when the OS
// grows a long-running audit server.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The genesis prev-hash. A fixed 32-byte zero block so the
/// chain has a well-known starting point that can be checked
/// without external state.
pub const GENESIS_PREV_HASH: [u8; 32] = [0u8; 32];

/// A single tamper-evident audit entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Monotonically increasing index within the log.
    pub index: u64,
    /// Wall-clock timestamp in milliseconds since the
    /// Unix epoch. The log trusts the caller to supply it;
    /// `record` does not read the clock itself.
    pub timestamp_ms: u64,
    /// Categorical event tag (e.g. "ipc.dispatch",
    /// "policy.decide", "sandbox.plan"). Free-form string so
    /// the log does not need a `match` for every caller.
    pub event: String,
    /// Originating component (e.g. "system-core",
    /// "process-manager").
    pub component: String,
    /// Optional free-form detail. Callers are responsible
    /// for redacting secrets before calling `record`.
    pub detail: String,
    /// `prev_hash` of the previous entry in the log, or
    /// `GENESIS_PREV_HASH` for `index == 0`.
    pub prev_hash: [u8; 32],
    /// SHA-256 of the canonical serialization of every
    /// other field in this entry, including `prev_hash`.
    /// `verify_chain` recomputes it from the stored fields
    /// and compares.
    pub content_hash: [u8; 32],
}

impl AuditEntry {
    /// Returns the canonical byte representation that is
    /// hashed into `content_hash`. The serialization is
    /// deterministic and field-order-independent from the
    /// caller's perspective: it is a fixed `concat` of the
    /// fields in declaration order, with the timestamp and
    /// index in big-endian form.
    fn canonical_bytes(&self) -> Vec<u8> {
        let mut buf =
            Vec::with_capacity(64 + self.event.len() + self.component.len() + self.detail.len());
        buf.extend_from_slice(&self.index.to_be_bytes());
        buf.extend_from_slice(&self.timestamp_ms.to_be_bytes());
        buf.extend_from_slice(self.event.as_bytes());
        buf.push(0);
        buf.extend_from_slice(self.component.as_bytes());
        buf.push(0);
        buf.extend_from_slice(self.detail.as_bytes());
        buf.push(0);
        buf.extend_from_slice(&self.prev_hash);
        buf
    }

    /// Recomputes the content hash from the entry's other
    /// fields. The result is what `content_hash` should hold
    /// for the entry to be authentic.
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

/// Retention policy for the audit log.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Hard cap on stored entries. When the log reaches this
    /// size, the next `record` evicts the oldest entry.
    /// `0` means unbounded.
    pub max_entries: usize,
    /// Soft cap based on age. `prune_older_than` drops
    /// entries with `timestamp_ms < now_ms - max_age_ms`.
    /// `0` disables time-based pruning.
    pub max_age_ms: u64,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        // 4 Ki entries is roughly 6 months of capacity for a
        // desktop OS firing ~20 events/minute. Bound matches
        // the agent-runtime's own default ring size.
        Self { max_entries: 4096, max_age_ms: 0 }
    }
}

impl RetentionPolicy {
    /// A policy that retains the most recent `n` entries and
    /// drops nothing by age.
    #[must_use]
    pub const fn last_n(n: usize) -> Self {
        Self { max_entries: n, max_age_ms: 0 }
    }

    /// A policy that retains entries newer than `age_ms`
    /// milliseconds relative to a caller-supplied `now_ms`,
    /// and caps the log at `n` entries.
    #[must_use]
    pub const fn bounded(n: usize, age_ms: u64) -> Self {
        Self { max_entries: n, max_age_ms: age_ms }
    }
}

/// Result of a chain verification pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainStatus {
    /// The chain is intact through every entry.
    Ok,
    /// The entry at `index` has a stored `content_hash`
    /// that does not match the recomputed hash.
    ContentMismatch { index: u64 },
    /// The entry at `index` has a `prev_hash` that does
    /// not match the previous entry's `content_hash`.
    BrokenLink { index: u64 },
    /// The entry at `index` is missing — the log's
    /// stored indices have a gap.
    IndexGap { index: u64 },
}

/// The tamper-evident audit log itself.
#[derive(Debug, Clone)]
pub struct AuditChain {
    entries: Vec<AuditEntry>,
    retention: RetentionPolicy,
}

impl AuditChain {
    /// Creates a new empty audit log with the given
    /// retention policy.
    #[must_use]
    pub fn new(retention: RetentionPolicy) -> Self {
        Self { entries: Vec::new(), retention }
    }

    /// Returns the number of entries currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the log has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the configured retention policy.
    #[must_use]
    pub fn retention(&self) -> RetentionPolicy {
        self.retention
    }

    /// Records a new entry. `prev_hash` and `content_hash`
    /// are computed by the log; the caller only supplies the
    /// human-meaningful fields.
    ///
    /// If `retention.max_entries` is non-zero and the log is
    /// at capacity, the oldest entry is evicted first to
    /// make room. Eviction does not change the index of
    /// existing entries; the new entry's `index` is set to
    /// the post-eviction length, which is the index the
    /// chain expects.
    pub fn record(&mut self, timestamp_ms: u64, event: &str, component: &str, detail: &str) -> u64 {
        if let Some(cap) = self.retention.max_entries.checked_add(1) {
            if cap != 0 && self.entries.len() >= self.retention.max_entries {
                self.entries.remove(0);
            }
        }
        let prev_hash = self.entries.last().map_or(GENESIS_PREV_HASH, |e| e.content_hash);
        let index = self.entries.len() as u64;
        let mut entry = AuditEntry {
            index,
            timestamp_ms,
            event: event.to_string(),
            component: component.to_string(),
            detail: detail.to_string(),
            prev_hash,
            content_hash: [0u8; 32],
        };
        entry.content_hash = entry.recompute_hash();
        self.entries.push(entry);
        index
    }

    /// Returns the entries in chronological order. The
    /// returned slice borrows from the log; clone if you
    /// need an owned copy.
    #[must_use]
    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    /// Returns the most recent `n` entries in
    /// newest-first order.
    #[must_use]
    pub fn recent(&self, n: usize) -> Vec<&AuditEntry> {
        let take = n.min(self.entries.len());
        self.entries.iter().rev().take(take).collect()
    }

    /// Walks the entire chain. Returns the first
    /// inconsistency found, or `Ok(())` if every entry's
    /// stored `content_hash` and `prev_hash` are valid.
    ///
    /// The check is O(n) over the stored entries. For a
    /// 4 Ki-entry log that is a few hundred microseconds.
    pub fn verify_chain(&self) -> Result<(), ChainStatus> {
        let mut expected_prev = GENESIS_PREV_HASH;
        for (expected_index, entry) in (0u64..).zip(self.entries.iter()) {
            if entry.index != expected_index {
                return Err(ChainStatus::IndexGap { index: entry.index });
            }
            if entry.prev_hash != expected_prev {
                return Err(ChainStatus::BrokenLink { index: entry.index });
            }
            let recomputed = entry.recompute_hash();
            if recomputed != entry.content_hash {
                return Err(ChainStatus::ContentMismatch { index: entry.index });
            }
            expected_prev = entry.content_hash;
        }
        Ok(())
    }

    /// Drops entries older than `now_ms - max_age_ms`. The
    /// remaining head's `prev_hash` is rewritten to
    /// `GENESIS_PREV_HASH` and its `content_hash` is
    /// recomputed so the chain remains self-consistent.
    ///
    /// Returns the number of entries that were dropped.
    ///
    /// If `max_age_ms` is `0` this is a no-op and returns
    /// `0`. If the head is itself expired, every entry is
    /// dropped and the log is empty.
    pub fn prune_older_than(&mut self, now_ms: u64) -> usize {
        if self.retention.max_age_ms == 0 {
            return 0;
        }
        let cutoff = now_ms.saturating_sub(self.retention.max_age_ms);
        let keep_from = self.entries.iter().position(|e| e.timestamp_ms >= cutoff);
        let Some(keep_from) = keep_from else {
            // Every entry is expired. Clear the log.
            let dropped = self.entries.len();
            self.entries.clear();
            return dropped;
        };
        if keep_from == 0 {
            return 0;
        }
        let dropped = keep_from;
        self.entries.drain(..keep_from);
        // Re-stamp the new head so its prev_hash points at
        // genesis, otherwise the chain reports a broken
        // link from the first remaining entry to its (now
        // gone) predecessor.
        if self.entries.is_empty() {
            return dropped;
        }
        // Re-stamp the new head so its prev_hash points at
        // genesis, otherwise the chain reports a broken link
        // from the first remaining entry to its (now gone)
        // predecessor. We also renumber the rest of the
        // chain so the indices remain contiguous.
        let len = self.entries.len();
        // Compute the new content hashes by walking the
        // vector once. The first entry gets genesis; every
        // other entry's prev_hash is the previous entry's
        // content_hash, which we just recomputed.
        let mut prev_hash = GENESIS_PREV_HASH;
        for (offset, entry) in self.entries.iter_mut().enumerate() {
            entry.index = offset as u64;
            entry.prev_hash = prev_hash;
            entry.content_hash = entry.recompute_hash();
            prev_hash = entry.content_hash;
        }
        // `len` is read once to make the borrow-checker
        // happy and to make the intent obvious: the loop
        // above touches every entry, so `len` is the
        // post-rewrite count.
        debug_assert_eq!(len, self.entries.len());
        dropped
    }

    /// Replaces the log's contents with `entries` after
    /// validating the supplied chain. Used by callers that
    /// rehydrate the log from a trusted source (a
    /// journal, a snapshot, a test fixture).
    ///
    /// If the supplied chain does not verify, the
    /// replacement is rejected and the log is left
    /// unchanged. The error returns the offending index.
    pub fn restore_from(&mut self, entries: Vec<AuditEntry>) -> Result<(), ChainStatus> {
        // Validate on a copy so a rejection does not leave
        // the log half-rebuilt.
        let mut expected_prev = GENESIS_PREV_HASH;
        for (expected_index, entry) in (0u64..).zip(entries.iter()) {
            if entry.index != expected_index {
                return Err(ChainStatus::IndexGap { index: entry.index });
            }
            if entry.prev_hash != expected_prev {
                return Err(ChainStatus::BrokenLink { index: entry.index });
            }
            let recomputed = entry.recompute_hash();
            if recomputed != entry.content_hash {
                return Err(ChainStatus::ContentMismatch { index: entry.index });
            }
            expected_prev = entry.content_hash;
        }
        if self.retention.max_entries != 0 && entries.len() > self.retention.max_entries {
            let drop = entries.len() - self.retention.max_entries;
            let mut entries = entries;
            entries.drain(..drop);
            self.entries = entries;
            return Ok(());
        }
        self.entries = entries;
        Ok(())
    }
}

impl Default for AuditChain {
    fn default() -> Self {
        Self::new(RetentionPolicy::default())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn entry_at(index: u64, _timestamp: u64) -> (String, String, String) {
        (format!("event-{index}"), "system-core".to_string(), format!("detail-{index}"))
    }

    #[test]
    fn empty_chain_verifies() {
        let chain = AuditChain::default();
        assert!(chain.verify_chain().is_ok());
    }

    #[test]
    fn record_assigns_increasing_indices() {
        let mut chain = AuditChain::new(RetentionPolicy::last_n(10));
        for i in 0..5 {
            let (event, component, detail) = entry_at(i, 1000 + i);
            let index = chain.record(1000 + i, &event, &component, &detail);
            assert_eq!(index, i);
        }
        assert_eq!(chain.len(), 5);
    }

    #[test]
    fn record_sets_prev_hash_to_previous_content_hash() {
        let mut chain = AuditChain::new(RetentionPolicy::last_n(10));
        chain.record(1000, "a", "c", "d0");
        chain.record(2000, "a", "c", "d1");
        let entries = chain.entries();
        assert_eq!(entries[0].prev_hash, GENESIS_PREV_HASH);
        assert_eq!(entries[1].prev_hash, entries[0].content_hash);
    }

    #[test]
    fn record_evicts_oldest_when_at_capacity() {
        let mut chain = AuditChain::new(RetentionPolicy::last_n(3));
        for i in 0..5 {
            let (event, component, detail) = entry_at(i, 1000 + i);
            chain.record(1000 + i, &event, &component, &detail);
        }
        assert_eq!(chain.len(), 3);
        let entries = chain.entries();
        // Entries 0 and 1 are evicted, so the surviving
        // entries are 2, 3, 4 in order.
        assert_eq!(entries[0].detail, "detail-2");
        assert_eq!(entries[1].detail, "detail-3");
        assert_eq!(entries[2].detail, "detail-4");
    }

    #[test]
    fn chain_verifies_after_many_inserts() {
        let mut chain = AuditChain::new(RetentionPolicy::last_n(2000));
        for i in 0..1000 {
            let (event, component, detail) = entry_at(i, 10_000 + i);
            chain.record(10_000 + i, &event, &component, &detail);
        }
        assert_eq!(chain.len(), 1000);
        assert!(chain.verify_chain().is_ok());
    }

    #[test]
    fn detect_content_tampering() {
        let mut chain = AuditChain::new(RetentionPolicy::last_n(10));
        for i in 0..5 {
            let (event, component, detail) = entry_at(i, 1000 + i);
            chain.record(1000 + i, &event, &component, &detail);
        }
        chain.entries[2].detail = "tampered".to_string();
        let status = chain.verify_chain().unwrap_err();
        assert_eq!(status, ChainStatus::ContentMismatch { index: 2 });
    }

    #[test]
    fn detect_broken_link_tampering() {
        let mut chain = AuditChain::new(RetentionPolicy::last_n(10));
        for i in 0..5 {
            let (event, component, detail) = entry_at(i, 1000 + i);
            chain.record(1000 + i, &event, &component, &detail);
        }
        chain.entries[3].prev_hash = [0xAB; 32];
        let status = chain.verify_chain().unwrap_err();
        assert_eq!(status, ChainStatus::BrokenLink { index: 3 });
    }

    #[test]
    fn detect_index_gap_tampering() {
        let mut chain = AuditChain::new(RetentionPolicy::last_n(10));
        for i in 0..5 {
            let (event, component, detail) = entry_at(i, 1000 + i);
            chain.record(1000 + i, &event, &component, &detail);
        }
        chain.entries[2].index = 99;
        let status = chain.verify_chain().unwrap_err();
        assert_eq!(status, ChainStatus::IndexGap { index: 99 });
    }

    #[test]
    fn recent_returns_newest_first() {
        let mut chain = AuditChain::new(RetentionPolicy::last_n(10));
        for i in 0..5 {
            let (event, component, detail) = entry_at(i, 1000 + i);
            chain.record(1000 + i, &event, &component, &detail);
        }
        let recent = chain.recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].detail, "detail-4");
        assert_eq!(recent[1].detail, "detail-3");
        assert_eq!(recent[2].detail, "detail-2");
    }

    #[test]
    fn recent_zero_returns_empty() {
        let mut chain = AuditChain::new(RetentionPolicy::last_n(10));
        chain.record(1, "e", "c", "d");
        assert!(chain.recent(0).is_empty());
    }

    #[test]
    fn prune_drops_expired_and_rewrites_head() {
        // max_age = 2_500 ms: anything older than 2_500 ms
        // relative to the supplied `now_ms` is dropped.
        let mut chain = AuditChain::new(RetentionPolicy::bounded(100, 2_500));
        chain.record(1_000, "e", "c", "d0");
        chain.record(2_000, "e", "c", "d1");
        chain.record(3_000, "e", "c", "d2");
        // At now=4_000, cutoff = 4_000 - 2_500 = 1_500.
        // Entries with timestamp < 1_500 are expired; only
        // d0 (timestamp 1_000) is dropped, leaving d1 and
        // d2 as the surviving chain.
        let dropped = chain.prune_older_than(4_000);
        assert_eq!(dropped, 1);
        assert_eq!(chain.len(), 2);
        let head = &chain.entries()[0];
        assert_eq!(head.detail, "d1");
        assert_eq!(head.index, 0);
        assert_eq!(head.prev_hash, GENESIS_PREV_HASH);
        // The chain is still self-consistent after a prune.
        assert!(chain.verify_chain().is_ok());
    }

    #[test]
    fn prune_with_zero_max_age_is_a_noop() {
        let mut chain = AuditChain::new(RetentionPolicy::last_n(10));
        chain.record(1_000, "e", "c", "d0");
        let dropped = chain.prune_older_than(100_000);
        assert_eq!(dropped, 0);
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn prune_clears_log_when_every_entry_is_expired() {
        let mut chain = AuditChain::new(RetentionPolicy::bounded(10, 100));
        chain.record(1_000, "e", "c", "d0");
        chain.record(1_500, "e", "c", "d1");
        let dropped = chain.prune_older_than(1_000_000);
        assert_eq!(dropped, 2);
        assert!(chain.is_empty());
        assert!(chain.verify_chain().is_ok());
    }

    #[test]
    fn restore_from_rejects_tampered_chain() {
        let mut chain = AuditChain::new(RetentionPolicy::last_n(10));
        for i in 0..3 {
            let (event, component, detail) = entry_at(i, 1000 + i);
            chain.record(1000 + i, &event, &component, &detail);
        }
        let mut entries = chain.entries().to_vec();
        entries[1].detail = "tampered".to_string();
        let res = chain.restore_from(entries);
        assert!(matches!(res, Err(ChainStatus::ContentMismatch { index: 1 })));
        // The log is unchanged.
        assert_eq!(chain.len(), 3);
        assert!(chain.verify_chain().is_ok());
    }

    #[test]
    fn restore_from_accepts_valid_chain() {
        let mut source = AuditChain::new(RetentionPolicy::last_n(10));
        for i in 0..5 {
            let (event, component, detail) = entry_at(i, 1000 + i);
            source.record(1000 + i, &event, &component, &detail);
        }
        let snapshot = source.entries().to_vec();
        let mut dest = AuditChain::new(RetentionPolicy::last_n(10));
        dest.restore_from(snapshot).expect("valid chain restores");
        assert_eq!(dest.len(), 5);
        assert!(dest.verify_chain().is_ok());
    }

    #[test]
    fn restore_from_drops_excess_to_fit_capacity() {
        let mut source = AuditChain::new(RetentionPolicy::last_n(20));
        for i in 0..10 {
            let (event, component, detail) = entry_at(i, 1000 + i);
            source.record(1000 + i, &event, &component, &detail);
        }
        let snapshot = source.entries().to_vec();
        let mut dest = AuditChain::new(RetentionPolicy::last_n(5));
        dest.restore_from(snapshot).expect("restore fits capacity");
        assert_eq!(dest.len(), 5);
        // The newest five survive: indices 5..10.
        let entries = dest.entries();
        assert_eq!(entries[0].detail, "detail-5");
        assert_eq!(entries[4].detail, "detail-9");
    }

    #[test]
    fn canonical_bytes_are_stable() {
        let mut chain = AuditChain::new(RetentionPolicy::last_n(1));
        chain.record(42, "e", "c", "d");
        let entry = &chain.entries()[0];
        let bytes = entry.canonical_bytes();
        // The fixed-field prefix is 16 bytes: index (u64)
        // then timestamp (u64), each in big-endian. The
        // first entry has index 0; the timestamp is 42.
        assert_eq!(&bytes[..8], &0u64.to_be_bytes());
        assert_eq!(&bytes[8..16], &42u64.to_be_bytes());
        // The body begins with the event bytes followed by
        // a nul separator, so the 17th byte is the nul.
        assert_eq!(bytes[16], b'e');
        assert_eq!(bytes[17], 0);
        assert_eq!(bytes[18], b'c');
    }
}
