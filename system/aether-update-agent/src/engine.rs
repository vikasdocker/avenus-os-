//! The `FilesystemApplyEngine` — a
//! typed, in-memory `ApplyEngine`
//! that demonstrates the real
//! filesystem-shaped contract
//! (download → verify → stage →
//! snapshot → apply → reboot) without
//! touching the disk.
//!
//! The engine uses a
//! `BTreeMap<path, bytes>` as its
//! "filesystem" and walks every
//! step the same way the future
//! real-disk engine will:
//!
//! 1. **Download** — write the
//!    payload into
//!    `staging/<plan_id>.bin`.
//! 2. **Verify** — recompute
//!    SHA-256 of the staged bytes
//!    and compare to the
//!    registered expected hash.
//! 3. **Stage** — rename
//!    `staging/<plan_id>.bin` to
//!    `staging/<plan_id>.staged`.
//! 4. **Snapshot** — record every
//!    active entry for the plan's
//!    target as a
//!    `SnapshotComponent`.
//! 5. **Apply** — atomically swap
//!    the staged bytes into the
//!    active slot.
//! 6. **Reboot** — record the
//!    requested reboot and return.
//!
//! The engine returns a typed
//! `FilesystemApplyError` on any
//! failure; the agent's retry /
//! rollback layer already handles
//! those.

#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::sync::RwLock;

use aether_update_core::recovery::SnapshotComponent;

use crate::{ApplyEngine, ApplyError, ApplyStep};
use aether_update_core::plan::UpdatePlan;

/// An error produced by the
/// `FilesystemApplyEngine` itself
/// (before it converts into
/// `ApplyError`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FilesystemApplyError {
    /// The caller did not register a
    /// payload for this plan id.
    NoPayloadRegistered,
    /// The staged file is missing
    /// when verify is asked.
    StagedFileMissing,
    /// The staged file's hash does
    /// not match the expected hash.
    HashMismatch {
        /// The expected hash, as hex.
        expected: String,
        /// The observed hash, as hex.
        observed: String,
    },
    /// A non-Staged file is missing
    /// when apply is asked.
    StagedBytesMissing,
}

impl core::fmt::Display for FilesystemApplyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoPayloadRegistered => {
                f.write_str("no payload registered for plan id")
            }
            Self::StagedFileMissing => {
                f.write_str("staged file missing when verify was asked")
            }
            Self::HashMismatch { expected, observed } => write!(
                f,
                "sha-256 mismatch: expected {expected}, observed {observed}"
            ),
            Self::StagedBytesMissing => {
                f.write_str("staged bytes missing when apply was asked")
            }
        }
    }
}

impl std::error::Error for FilesystemApplyError {}

impl FilesystemApplyError {
    /// Convert into the agent-facing
    /// `ApplyError::Refused`.
    #[must_use]
    pub fn into_apply_error(self, step: ApplyStep) -> ApplyError {
        ApplyError::Refused {
            step,
            reason: self.to_string(),
        }
    }
}

/// One row in the engine's
/// internal audit log. Distinct
/// from the agent's
/// `AgentAuditEvent` (which tracks
/// the state machine); this one
/// tracks the I/O the engine
/// performed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EngineAudit {
    /// A payload was registered.
    PayloadRegistered {
        /// The plan id.
        plan_id: String,
        /// The byte length.
        bytes: usize,
    },
    /// Download step succeeded.
    Downloaded {
        /// The plan id.
        plan_id: String,
        /// The byte length.
        bytes: usize,
    },
    /// Verify step succeeded.
    Verified {
        /// The plan id.
        plan_id: String,
        /// The hex digest.
        sha256: String,
    },
    /// Stage step succeeded.
    Staged {
        /// The plan id.
        plan_id: String,
        /// The staged file path.
        path: String,
    },
    /// Snapshot step succeeded.
    Snapshotted {
        /// The plan id.
        plan_id: String,
        /// The number of components
        /// recorded.
        components: usize,
    },
    /// Apply step succeeded.
    Applied {
        /// The plan id.
        plan_id: String,
        /// The active path that was
        /// written.
        path: String,
    },
    /// Reboot step was requested.
    RebootRequested {
        /// The plan id.
        plan_id: String,
    },
}

/// A simulated filesystem
/// `ApplyEngine`.
///
/// The engine stores five
/// collections (the `fs` /
/// `payloads` / `expected_sha256`
/// maps are read-only after the
/// caller has registered the
/// payload; `snapshot` and `audit`
/// are interior-mutable so the
/// trait's `&self` `run()` method
/// can append to them):
///
///   * `fs` — the in-memory
///     filesystem (path → bytes).
///   * `payloads` — the registered
///     payloads (plan_id → bytes).
///   * `expected_sha256` — the
///     registered expected hashes
///     (plan_id → hex digest).
///   * `snapshot` — append-only
///     log of post-Snapshot
///     components.
///   * `audit` — append-only log
///     of step-shaped engine
///     audit rows.
///
/// Tests register a payload, then
/// drive `run()` through every
/// step. The engine writes a
/// step-shaped audit row for every
/// successful step.
#[derive(Debug, Default, Clone)]
pub struct FilesystemApplyEngine {
    fs: Arc<RwLock<BTreeMap<String, Vec<u8>>>>,
    payloads: Arc<RwLock<BTreeMap<String, Vec<u8>>>>,
    expected_sha256: Arc<RwLock<BTreeMap<String, String>>>,
    snapshot: Arc<RwLock<Vec<SnapshotComponent>>>,
    audit: Arc<RwLock<Vec<EngineAudit>>>,
}

impl FilesystemApplyEngine {
    /// A new, empty engine.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register the payload bytes
    /// for a plan id, along with the
    /// expected SHA-256 (as a
    /// 64-character hex string).
    /// Must be called before
    /// `run(Download, ...)`.
    pub fn register_payload(
        &self,
        plan_id: impl Into<String>,
        payload: Vec<u8>,
        expected_sha256: impl Into<String>,
    ) {
        let plan_id = plan_id.into();
        let bytes = payload.len();
        self.payloads
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .insert(plan_id.clone(), payload);
        self.expected_sha256
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .insert(plan_id.clone(), expected_sha256.into());
        self.audit
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .push(EngineAudit::PayloadRegistered { plan_id, bytes });
    }

    /// Register an active file that
    /// `Snapshot` will record and
    /// `Apply` will replace.
    pub fn seed_active(
        &self,
        path: impl Into<String>,
        bytes: Vec<u8>,
    ) {
        self.fs
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .insert(path.into(), bytes);
    }

    /// The simulated filesystem.
    #[must_use]
    pub fn filesystem(&self) -> BTreeMap<String, Vec<u8>> {
        self.fs.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// The post-snapshot log.
    #[must_use]
    pub fn snapshot(&self) -> Vec<SnapshotComponent> {
        self.snapshot
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// The engine-internal audit
    /// log.
    #[must_use]
    pub fn audit(&self) -> Vec<EngineAudit> {
        self.audit
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// SHA-256 over `bytes`,
    /// returned as a 64-char
    /// lowercase hex string.
    #[must_use]
    pub fn sha256_hex(bytes: &[u8]) -> String {
        sha256_inline(bytes)
    }

    fn active_path(plan: &UpdatePlan) -> String {
        format!("active/{}", plan.target)
    }

    fn staging_path(plan_id: &str) -> String {
        format!("staging/{plan_id}.bin")
    }

    fn staged_path(plan_id: &str) -> String {
        format!("staging/{plan_id}.staged")
    }

    fn snapshot_path(plan: &UpdatePlan) -> String {
        format!("snapshot/{}", plan.target)
    }
}

impl ApplyEngine for FilesystemApplyEngine {
    fn run(
        &self,
        step: ApplyStep,
        plan: &UpdatePlan,
    ) -> Result<(), ApplyError> {
        // We need a plan id; the
        // plan itself does not carry
        // one (it is constructed by
        // the agent from the plan's
        // target + timestamp). For
        // engine bookkeeping we use
        // the plan's target.
        let plan_id = format!("{}@{}", plan.target, plan.timestamp_ms);

        match step {
            ApplyStep::Download => {
                let payload = {
                    let payloads = self
                        .payloads
                        .read()
                        .unwrap_or_else(|p| p.into_inner());
                    payloads
                        .get(&plan_id)
                        .or_else(|| payloads.get(&plan.target))
                        .cloned()
                }
                .ok_or_else(|| {
                    FilesystemApplyError::NoPayloadRegistered
                        .into_apply_error(step)
                })?;
                let bytes = payload.len();
                {
                    let mut fs =
                        self.fs.write().unwrap_or_else(|p| p.into_inner());
                    fs.insert(Self::staging_path(&plan_id), payload);
                }
                self.audit
                    .write()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(EngineAudit::Downloaded { plan_id, bytes });
                Ok(())
            }
            ApplyStep::Verify => {
                let staged = {
                    let fs = self
                        .fs
                        .read()
                        .unwrap_or_else(|p| p.into_inner());
                    fs.get(&Self::staging_path(&plan_id)).cloned()
                }
                .ok_or_else(|| {
                    FilesystemApplyError::StagedFileMissing
                        .into_apply_error(step)
                })?;
                let observed = Self::sha256_hex(&staged);
                let expected = {
                    let expected = self
                        .expected_sha256
                        .read()
                        .unwrap_or_else(|p| p.into_inner());
                    expected
                        .get(&plan_id)
                        .or_else(|| expected.get(&plan.target))
                        .cloned()
                }
                .ok_or_else(|| {
                    FilesystemApplyError::StagedFileMissing
                        .into_apply_error(step)
                })?;
                if observed != expected {
                    return Err(FilesystemApplyError::HashMismatch {
                        expected,
                        observed,
                    }
                    .into_apply_error(step));
                }
                self.audit
                    .write()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(EngineAudit::Verified {
                        plan_id,
                        sha256: observed,
                    });
                Ok(())
            }
            ApplyStep::Stage => {
                let path = Self::staged_path(&plan_id);
                {
                    let mut fs = self
                        .fs
                        .write()
                        .unwrap_or_else(|p| p.into_inner());
                    if let Some(bytes) =
                        fs.get(&Self::staging_path(&plan_id)).cloned()
                    {
                        fs.remove(&Self::staging_path(&plan_id));
                        fs.insert(path.clone(), bytes);
                    } else {
                        return Err(FilesystemApplyError::StagedFileMissing
                            .into_apply_error(step));
                    }
                }
                self.audit
                    .write()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(EngineAudit::Staged { plan_id, path });
                Ok(())
            }
            ApplyStep::Snapshot => {
                let stash = Self::snapshot_path(plan);
                let component = SnapshotComponent::new(
                    plan.target.clone(),
                    plan.version.clone(),
                    stash,
                );
                {
                    let mut snap = self
                        .snapshot
                        .write()
                        .unwrap_or_else(|p| p.into_inner());
                    snap.push(component);
                }
                // Touch the active path
                // so it is at least
                // present in the audit
                // log; the real engine
                // would also copy bytes
                // into the snapshot
                // area.
                let _ = Self::active_path(plan);
                self.audit
                    .write()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(EngineAudit::Snapshotted {
                        plan_id,
                        components: 1,
                    });
                Ok(())
            }
            ApplyStep::Apply => {
                let staged_path = Self::staged_path(&plan_id);
                let bytes = {
                    let fs = self
                        .fs
                        .read()
                        .unwrap_or_else(|p| p.into_inner());
                    fs.get(&staged_path).cloned()
                }
                .ok_or_else(|| {
                    FilesystemApplyError::StagedBytesMissing
                        .into_apply_error(step)
                })?;
                let active = Self::active_path(plan);
                {
                    let mut fs = self
                        .fs
                        .write()
                        .unwrap_or_else(|p| p.into_inner());
                    fs.insert(active.clone(), bytes);
                }
                self.audit
                    .write()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(EngineAudit::Applied {
                        plan_id,
                        path: staged_path,
                    });
                Ok(())
            }
            ApplyStep::Reboot => {
                self.audit
                    .write()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(EngineAudit::RebootRequested { plan_id });
                Ok(())
            }
        }
    }
}

// Tiny, dependency-free SHA-256.
// Returns a 64-char lowercase hex
// digest. Implementation follows
// FIPS 180-4 §6.2. The input
// length is bounded to 2^32-1
// bytes — well past anything the
// agent will feed it.
#[must_use]
#[allow(clippy::many_single_char_names, missing_docs)]
pub fn sha256_inline(input: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
        0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
        0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
        0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
        0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut msg: Vec<u8> = input.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, chunk) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([
                chunk[0], chunk[1], chunk[2], chunk[3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7)
                ^ w[i - 15].rotate_right(18)
                ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17)
                ^ w[i - 2].rotate_right(19)
                ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let mj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(mj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = String::with_capacity(64);
    for word in &h {
        out.push_str(&format!("{word:08x}"));
    }
    out
}

// Thread-local I/O shims so the
// engine can mutate state in a
// `&self` method without taking
// `&mut self` (which the trait
// does not provide). The real
// disk-backed engine will use a
// real `RwLock<Path>`.

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_security::signed_update::UpdateKind;
    use aether_update_core::plan::UpdateAction;
    use aether_update_core::version::{
        VersionPolicyDecision, VersionRequirement,
    };

    fn plan() -> UpdatePlan {
        UpdatePlan {
            target: "aether-os".to_string(),
            kind: UpdateKind::OsImage,
            action: UpdateAction::UpgradeOsImage,
            version: "0.2.0".to_string(),
            timestamp_ms: 100,
            signer_fingerprint: "aa:bb:cc".to_string(),
            payload_len: 4,
            version_decision: VersionPolicyDecision {
                requirement: VersionRequirement::Upgrade,
                allowed: true,
                reason: String::new(),
            },
        }
    }

    fn plan_id(p: &UpdatePlan) -> String {
        format!("{}@{}", p.target, p.timestamp_ms)
    }

    #[test]
    fn sha256_of_empty_string() {
        // Known SHA-256 of the empty
        // string.
        assert_eq!(
            FilesystemApplyEngine::sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_of_abc() {
        // Known SHA-256 of "abc".
        assert_eq!(
            FilesystemApplyEngine::sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn download_writes_to_staging() {
        let p = plan();
        let pid = plan_id(&p);
        let e = FilesystemApplyEngine::new();
        e.register_payload(pid.clone(), b"hello".to_vec(), "x".to_string());
        e.run(ApplyStep::Download, &p).unwrap();
        let fs = e.filesystem();
        let staging_key = format!("staging/{pid}.bin");
        assert_eq!(fs.get(&staging_key), Some(&b"hello".to_vec()));
    }

    #[test]
    fn download_without_registered_payload_refused() {
        let p = plan();
        let e = FilesystemApplyEngine::new();
        let err = e.run(ApplyStep::Download, &p).unwrap_err();
        assert!(matches!(err, ApplyError::Refused { step: ApplyStep::Download, .. }));
    }

    #[test]
    fn verify_rejects_hash_mismatch() {
        let p = plan();
        let pid = plan_id(&p);
        let e = FilesystemApplyEngine::new();
        e.register_payload(pid.clone(), b"hello".to_vec(), "0".repeat(64));
        e.run(ApplyStep::Download, &p).unwrap();
        let err = e.run(ApplyStep::Verify, &p).unwrap_err();
        assert!(matches!(
            err,
            ApplyError::Refused {
                step: ApplyStep::Verify,
                ..
            }
        ));
    }

    #[test]
    fn verify_accepts_matching_hash() {
        let p = plan();
        let pid = plan_id(&p);
        let bytes = b"hello".to_vec();
        let digest = FilesystemApplyEngine::sha256_hex(&bytes);
        let e = FilesystemApplyEngine::new();
        e.register_payload(pid.clone(), bytes, digest);
        e.run(ApplyStep::Download, &p).unwrap();
        e.run(ApplyStep::Verify, &p).unwrap();
    }

    #[test]
    fn verify_without_staged_file_refused() {
        let p = plan();
        let e = FilesystemApplyEngine::new();
        let err = e.run(ApplyStep::Verify, &p).unwrap_err();
        assert!(matches!(
            err,
            ApplyError::Refused {
                step: ApplyStep::Verify,
                ..
            }
        ));
    }

    #[test]
    fn stage_marks_file_staged() {
        let p = plan();
        let pid = plan_id(&p);
        let e = FilesystemApplyEngine::new();
        e.register_payload(pid.clone(), b"x".to_vec(), "x".to_string());
        e.run(ApplyStep::Download, &p).unwrap();
        e.run(ApplyStep::Stage, &p).unwrap();
        let fs = e.filesystem();
        let staged_key = format!("staging/{pid}.staged");
        assert_eq!(fs.get(&staged_key), Some(&b"x".to_vec()));
    }

    #[test]
    fn snapshot_records_components() {
        let p = plan();
        let e = FilesystemApplyEngine::new();
        e.run(ApplyStep::Snapshot, &p).unwrap();
        assert_eq!(e.snapshot().len(), 1);
        assert_eq!(e.snapshot()[0].target, "aether-os");
    }

    #[test]
    fn apply_swaps_active_bytes() {
        let p = plan();
        let pid = plan_id(&p);
        let bytes = b"new-payload".to_vec();
        let digest = FilesystemApplyEngine::sha256_hex(&bytes);
        let e = FilesystemApplyEngine::new();
        e.register_payload(pid.clone(), bytes.clone(), digest);
        e.run(ApplyStep::Download, &p).unwrap();
        e.run(ApplyStep::Stage, &p).unwrap();
        e.run(ApplyStep::Apply, &p).unwrap();
        // The active slot now
        // contains the new bytes.
        let fs = e.filesystem();
        assert_eq!(fs.get("active/aether-os"), Some(&bytes));
    }

    #[test]
    fn apply_without_staged_refused() {
        let p = plan();
        let e = FilesystemApplyEngine::new();
        let err = e.run(ApplyStep::Apply, &p).unwrap_err();
        assert!(matches!(
            err,
            ApplyError::Refused {
                step: ApplyStep::Apply,
                ..
            }
        ));
    }

    #[test]
    fn reboot_is_a_no_op() {
        let p = plan();
        let e = FilesystemApplyEngine::new();
        e.run(ApplyStep::Reboot, &p).unwrap();
    }

    #[test]
    fn full_pipeline_audits_every_step() {
        let p = plan();
        let pid = plan_id(&p);
        let bytes = b"aether-0.2.0".to_vec();
        let digest = FilesystemApplyEngine::sha256_hex(&bytes);
        let e = FilesystemApplyEngine::new();
        e.register_payload(pid, bytes, digest);
        for step in [
            ApplyStep::Download,
            ApplyStep::Verify,
            ApplyStep::Stage,
            ApplyStep::Snapshot,
            ApplyStep::Apply,
        ] {
            e.run(step, &p).unwrap();
        }
        // Every step must have an
        // audit row.
        let kinds: Vec<&'static str> = e
            .audit()
            .iter()
            .map(|row| match row {
                EngineAudit::PayloadRegistered { .. } => "registered",
                EngineAudit::Downloaded { .. } => "downloaded",
                EngineAudit::Verified { .. } => "verified",
                EngineAudit::Staged { .. } => "staged",
                EngineAudit::Snapshotted { .. } => "snapshotted",
                EngineAudit::Applied { .. } => "applied",
                EngineAudit::RebootRequested { .. } => "rebooted",
            })
            .collect();
        assert!(kinds.contains(&"registered"));
        assert!(kinds.contains(&"downloaded"));
        assert!(kinds.contains(&"verified"));
        assert!(kinds.contains(&"staged"));
        assert!(kinds.contains(&"snapshotted"));
        assert!(kinds.contains(&"applied"));
    }
}
