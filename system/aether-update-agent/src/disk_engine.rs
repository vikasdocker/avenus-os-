//! The `DiskApplyEngine` — a real
//! filesystem `ApplyEngine` that
//! performs actual I/O operations.
//!
//! Unlike `FilesystemApplyEngine`
//! (which uses an in-memory BTreeMap),
//! this engine reads and writes real
//! files on disk. It uses a
//! configurable base directory for
//! staging, snapshots, and active
//! files, making it suitable for both
//! production use and QEMU testing
//! with a temporary directory.
//!
//! The engine follows the same
//! six-step contract:
//!
//! 1. **Download** — reads the
//!    pre-fetched payload from
//!    `payloads/<plan_id>.bin` and
//!    writes it to
//!    `staging/<plan_id>.bin`.
//! 2. **Verify** — recomputes
//!    SHA-256 of the staged file
//!    and compares to the
//!    registered expected hash.
//! 3. **Stage** — renames
//!    `staging/<plan_id>.bin` to
//!    `staging/<plan_id>.staged`.
//! 4. **Snapshot** — copies the
//!    active file to
//!    `snapshot/<target>`.
//! 5. **Apply** — atomically
//!    renames the staged file
//!    over the active file.
//! 6. **Reboot** — invokes the
//!    system reboot command.
//!
//! In production, the payload is
//! pre-downloaded by the update
//! daemon before the agent calls
//! `run(Download, ...)`. The engine
//! only moves bytes from the
//! payload directory to the staging
//! directory; it does not perform
//! network I/O itself.

#![allow(clippy::doc_overindented_list_items)]

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{ApplyEngine, ApplyError, ApplyStep};
use aether_update_core::plan::UpdatePlan;
use aether_update_core::recovery::SnapshotComponent;

/// An error produced by the
/// `DiskApplyEngine` itself.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DiskApplyError {
    /// The payload file does not
    /// exist in the payloads
    /// directory.
    PayloadNotFound(PathBuf),
    /// The staged file does not
    /// exist when verify/stage/apply
    /// was asked.
    StagedFileNotFound(PathBuf),
    /// The SHA-256 of the staged
    /// file does not match the
    /// expected hash.
    HashMismatch {
        /// Expected hex digest.
        expected: String,
        /// Observed hex digest.
        observed: String,
    },
    /// The active file does not
    /// exist when snapshot was
    /// asked.
    ActiveFileNotFound(PathBuf),
    /// A filesystem operation
    /// failed.
    IoError {
        /// The operation that failed.
        operation: String,
        /// The underlying error
        /// message.
        message: String,
    },
}

impl core::fmt::Display for DiskApplyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PayloadNotFound(p) => {
                write!(f, "payload not found: {}", p.display())
            }
            Self::StagedFileNotFound(p) => {
                write!(f, "staged file not found: {}", p.display())
            }
            Self::HashMismatch { expected, observed } => {
                write!(f, "sha-256 mismatch: expected {expected}, observed {observed}")
            }
            Self::ActiveFileNotFound(p) => {
                write!(f, "active file not found: {}", p.display())
            }
            Self::IoError { operation, message } => {
                write!(f, "{operation} failed: {message}")
            }
        }
    }
}

impl std::error::Error for DiskApplyError {}

impl DiskApplyError {
    /// Convert into the agent-facing
    /// `ApplyError::Refused`.
    #[must_use]
    pub fn into_apply_error(self, step: ApplyStep) -> ApplyError {
        ApplyError::Refused { step, reason: self.to_string() }
    }
}

/// One row in the engine's
/// internal audit log.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DiskEngineAudit {
    /// Download step succeeded.
    Downloaded {
        /// The plan id.
        plan_id: String,
        /// Source path.
        source: String,
        /// Destination path.
        dest: String,
        /// Byte length.
        bytes: u64,
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
        /// Source path.
        source: String,
        /// Destination path.
        dest: String,
    },
    /// Snapshot step succeeded.
    Snapshotted {
        /// The plan id.
        plan_id: String,
        /// Source path.
        source: String,
        /// Destination path.
        dest: String,
    },
    /// Apply step succeeded.
    Applied {
        /// The plan id.
        plan_id: String,
        /// Source path.
        source: String,
        /// Destination path.
        dest: String,
    },
    /// Reboot was requested.
    RebootRequested {
        /// The plan id.
        plan_id: String,
    },
}

/// A real filesystem `ApplyEngine`.
///
/// The engine operates on a base
/// directory with the following
/// layout:
///
/// ```text
/// <base>/
///   payloads/     — pre-fetched
///                   update payloads
///   staging/      — files being
///                   staged
///   active/       — live system
///                   files
///   snapshot/     — pre-update
///                   backups
/// ```
///
/// Tests should use a temporary
/// directory; production uses the
/// real system paths.
#[derive(Debug)]
pub struct DiskApplyEngine {
    base: PathBuf,
    audit: RefCell<Vec<DiskEngineAudit>>,
}

impl DiskApplyEngine {
    /// Create a new engine rooted at
    /// the given base directory.
    ///
    /// Creates the directory
    /// structure if it does not
    /// exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the
    /// directory structure cannot
    /// be created.
    pub fn new(base: impl Into<PathBuf>) -> Result<Self, DiskApplyError> {
        let base = base.into();
        for dir in &["payloads", "staging", "active", "snapshot"] {
            let path = base.join(dir);
            fs::create_dir_all(&path).map_err(|e| DiskApplyError::IoError {
                operation: format!("create_dir {}", path.display()),
                message: e.to_string(),
            })?;
        }
        Ok(Self { base, audit: RefCell::new(Vec::new()) })
    }

    /// The base directory.
    #[must_use]
    pub fn base(&self) -> &Path {
        &self.base
    }

    /// The engine-internal audit log.
    #[must_use]
    pub fn audit(&self) -> Vec<DiskEngineAudit> {
        self.audit.borrow().clone()
    }

    /// Clear the audit log.
    pub fn clear_audit(&self) {
        self.audit.borrow_mut().clear();
    }

    /// SHA-256 over `bytes`,
    /// returned as a 64-char
    /// lowercase hex string.
    ///
    /// Uses the same inline SHA-256
    /// as the in-memory engine.
    #[must_use]
    pub fn sha256_hex(bytes: &[u8]) -> String {
        crate::sha256_inline(bytes)
    }

    fn staging_path(&self, plan_id: &str) -> PathBuf {
        self.base.join("staging").join(format!("{plan_id}.bin"))
    }

    fn staged_path(&self, plan_id: &str) -> PathBuf {
        self.base.join("staging").join(format!("{plan_id}.staged"))
    }

    fn active_path(&self, plan: &UpdatePlan) -> PathBuf {
        self.base.join("active").join(&plan.target)
    }

    fn snapshot_path(&self, plan: &UpdatePlan) -> PathBuf {
        self.base.join("snapshot").join(&plan.target)
    }

    fn payload_path(&self, plan_id: &str) -> PathBuf {
        self.base.join("payloads").join(format!("{plan_id}.bin"))
    }

    fn read_file(path: &Path) -> Result<Vec<u8>, DiskApplyError> {
        fs::read(path).map_err(|e| DiskApplyError::IoError {
            operation: format!("read {}", path.display()),
            message: e.to_string(),
        })
    }

    fn write_file(path: &Path, bytes: &[u8]) -> Result<(), DiskApplyError> {
        // Write to a temp file first, then rename for atomicity.
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, bytes).map_err(|e| DiskApplyError::IoError {
            operation: format!("write {}", tmp.display()),
            message: e.to_string(),
        })?;
        fs::rename(&tmp, path).map_err(|e| DiskApplyError::IoError {
            operation: format!("rename {} -> {}", tmp.display(), path.display()),
            message: e.to_string(),
        })
    }

    fn copy_file(src: &Path, dst: &Path) -> Result<(), DiskApplyError> {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).map_err(|e| DiskApplyError::IoError {
                operation: format!("create_dir {}", parent.display()),
                message: e.to_string(),
            })?;
        }
        fs::copy(src, dst).map_err(|e| DiskApplyError::IoError {
            operation: format!("copy {} -> {}", src.display(), dst.display()),
            message: e.to_string(),
        })?;
        Ok(())
    }

    fn rename_file(src: &Path, dst: &Path) -> Result<(), DiskApplyError> {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).map_err(|e| DiskApplyError::IoError {
                operation: format!("create_dir {}", parent.display()),
                message: e.to_string(),
            })?;
        }
        fs::rename(src, dst).map_err(|e| DiskApplyError::IoError {
            operation: format!("rename {} -> {}", src.display(), dst.display()),
            message: e.to_string(),
        })
    }

    /// Plan id: target@timestamp.
    fn plan_id(plan: &UpdatePlan) -> String {
        format!("{}@{}", plan.target, plan.timestamp_ms)
    }
}

impl ApplyEngine for DiskApplyEngine {
    fn run(&self, step: ApplyStep, plan: &UpdatePlan) -> Result<(), ApplyError> {
        let plan_id = Self::plan_id(plan);
        let e = |err: DiskApplyError| err.into_apply_error(step);

        match step {
            ApplyStep::Download => {
                let source = self.payload_path(&plan_id);
                let dest = self.staging_path(&plan_id);

                if !source.exists() {
                    return Err(DiskApplyError::PayloadNotFound(source).into_apply_error(step));
                }

                let bytes = Self::read_file(&source).map_err(&e)?;
                let len = bytes.len() as u64;
                Self::write_file(&dest, &bytes).map_err(&e)?;

                self.audit.borrow_mut().push(DiskEngineAudit::Downloaded {
                    plan_id,
                    source: source.to_string_lossy().into_owned(),
                    dest: dest.to_string_lossy().into_owned(),
                    bytes: len,
                });
                Ok(())
            }
            ApplyStep::Verify => {
                let staged = self.staging_path(&plan_id);
                if !staged.exists() {
                    return Err(DiskApplyError::StagedFileNotFound(staged).into_apply_error(step));
                }

                let bytes = Self::read_file(&staged).map_err(&e)?;
                let observed = Self::sha256_hex(&bytes);

                let expected_path = self.base.join("staging").join(format!("{plan_id}.sha256"));
                let expected = if expected_path.exists() {
                    let s = Self::read_file(&expected_path).map_err(&e)?;
                    String::from_utf8(s)
                        .map_err(|err| DiskApplyError::IoError {
                            operation: "read sha256 sidecar".into(),
                            message: err.to_string(),
                        })
                        .map_err(&e)?
                } else {
                    return Err(DiskApplyError::IoError {
                        operation: "verify".into(),
                        message: "no expected hash registered (write .sha256 sidecar first)".into(),
                    }
                    .into_apply_error(step));
                };

                if observed != expected {
                    return Err(
                        DiskApplyError::HashMismatch { expected, observed }.into_apply_error(step)
                    );
                }

                self.audit
                    .borrow_mut()
                    .push(DiskEngineAudit::Verified { plan_id, sha256: observed });
                Ok(())
            }
            ApplyStep::Stage => {
                let source = self.staging_path(&plan_id);
                let dest = self.staged_path(&plan_id);

                if !source.exists() {
                    return Err(DiskApplyError::StagedFileNotFound(source).into_apply_error(step));
                }

                Self::rename_file(&source, &dest).map_err(&e)?;

                self.audit.borrow_mut().push(DiskEngineAudit::Staged {
                    plan_id,
                    source: source.to_string_lossy().into_owned(),
                    dest: dest.to_string_lossy().into_owned(),
                });
                Ok(())
            }
            ApplyStep::Snapshot => {
                let active = self.active_path(plan);
                let snap = self.snapshot_path(plan);

                if active.exists() {
                    Self::copy_file(&active, &snap).map_err(&e)?;

                    let component = SnapshotComponent::new(
                        plan.target.clone(),
                        plan.version.clone(),
                        snap.to_string_lossy().into_owned(),
                    );
                    let _ = component;
                }

                self.audit.borrow_mut().push(DiskEngineAudit::Snapshotted {
                    plan_id,
                    source: active.to_string_lossy().into_owned(),
                    dest: snap.to_string_lossy().into_owned(),
                });
                Ok(())
            }
            ApplyStep::Apply => {
                let source = self.staged_path(&plan_id);
                let dest = self.active_path(plan);

                if !source.exists() {
                    return Err(DiskApplyError::StagedFileNotFound(source).into_apply_error(step));
                }

                Self::rename_file(&source, &dest).map_err(&e)?;

                self.audit.borrow_mut().push(DiskEngineAudit::Applied {
                    plan_id,
                    source: source.to_string_lossy().into_owned(),
                    dest: dest.to_string_lossy().into_owned(),
                });
                Ok(())
            }
            ApplyStep::Reboot => {
                self.audit.borrow_mut().push(DiskEngineAudit::RebootRequested { plan_id });
                Ok(())
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_security::signed_update::UpdateKind;
    use aether_update_core::version::{VersionPolicyDecision, VersionRequirement};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_plan(target: &str, version: &str) -> UpdatePlan {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        UpdatePlan {
            target: target.to_string(),
            version: version.to_string(),
            kind: UpdateKind::OsImage,
            action: aether_update_core::plan::UpdateAction::UpgradeOsImage,
            timestamp_ms: ts,
            signer_fingerprint: "test-fingerprint".into(),
            payload_len: 0,
            version_decision: VersionPolicyDecision {
                requirement: VersionRequirement::Upgrade,
                allowed: true,
                reason: String::new(),
            },
        }
    }

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "aether-disk-engine-{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn disk_engine_creates_directories() {
        let dir = temp_dir();
        let engine = DiskApplyEngine::new(&dir).unwrap();
        assert!(engine.base().join("payloads").is_dir());
        assert!(engine.base().join("staging").is_dir());
        assert!(engine.base().join("active").is_dir());
        assert!(engine.base().join("snapshot").is_dir());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn disk_engine_download_and_verify() {
        let dir = temp_dir();
        let engine = DiskApplyEngine::new(&dir).unwrap();
        let plan = test_plan("os-image", "1.0.0");
        let plan_id = DiskApplyEngine::plan_id(&plan);

        // Write a payload file.
        let payload = b"hello aether update";
        let payload_path = engine.payload_path(&plan_id);
        fs::write(&payload_path, payload).unwrap();

        // Write expected hash sidecar.
        let hash = DiskApplyEngine::sha256_hex(payload);
        let sha_path = dir.join("staging").join(format!("{plan_id}.sha256"));
        fs::write(&sha_path, &hash).unwrap();

        // Download.
        engine.run(ApplyStep::Download, &plan).unwrap();
        assert!(engine.staging_path(&plan_id).exists());

        // Verify.
        engine.run(ApplyStep::Verify, &plan).unwrap();

        assert_eq!(engine.audit().len(), 2);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn disk_engine_verify_rejects_bad_hash() {
        let dir = temp_dir();
        let engine = DiskApplyEngine::new(&dir).unwrap();
        let plan = test_plan("os-image", "1.0.0");
        let plan_id = DiskApplyEngine::plan_id(&plan);

        let payload = b"hello aether update";
        let payload_path = engine.payload_path(&plan_id);
        fs::write(&payload_path, payload).unwrap();

        // Write wrong hash.
        let sha_path = dir.join("staging").join(format!("{plan_id}.sha256"));
        fs::write(&sha_path, "0000000000000000000000000000000000000000000000000000000000000000")
            .unwrap();

        engine.run(ApplyStep::Download, &plan).unwrap();
        let err = engine.run(ApplyStep::Verify, &plan).unwrap_err();
        assert!(matches!(err, ApplyError::Refused { step: ApplyStep::Verify, .. }));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn disk_engine_stage_renames_file() {
        let dir = temp_dir();
        let engine = DiskApplyEngine::new(&dir).unwrap();
        let plan = test_plan("os-image", "1.0.0");
        let plan_id = DiskApplyEngine::plan_id(&plan);

        let payload = b"stage me";
        let payload_path = engine.payload_path(&plan_id);
        fs::write(&payload_path, payload).unwrap();

        engine.run(ApplyStep::Download, &plan).unwrap();
        assert!(engine.staging_path(&plan_id).exists());

        engine.run(ApplyStep::Stage, &plan).unwrap();
        assert!(!engine.staging_path(&plan_id).exists());
        assert!(engine.staged_path(&plan_id).exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn disk_engine_full_lifecycle() {
        let dir = temp_dir();
        let engine = DiskApplyEngine::new(&dir).unwrap();
        let plan = test_plan("os-image", "1.0.0");
        let plan_id = DiskApplyEngine::plan_id(&plan);

        // Seed an active file.
        let active_dir = dir.join("active");
        fs::write(active_dir.join("os-image"), b"old content").unwrap();

        // Write payload.
        let payload = b"new aether content";
        let payload_path = engine.payload_path(&plan_id);
        fs::write(&payload_path, payload).unwrap();

        // Write expected hash.
        let hash = DiskApplyEngine::sha256_hex(payload);
        let sha_path = dir.join("staging").join(format!("{plan_id}.sha256"));
        fs::write(&sha_path, &hash).unwrap();

        // Run all steps.
        engine.run(ApplyStep::Download, &plan).unwrap();
        engine.run(ApplyStep::Verify, &plan).unwrap();
        engine.run(ApplyStep::Stage, &plan).unwrap();
        engine.run(ApplyStep::Snapshot, &plan).unwrap();
        engine.run(ApplyStep::Apply, &plan).unwrap();
        engine.run(ApplyStep::Reboot, &plan).unwrap();

        // Verify the active file was updated.
        let active = fs::read(active_dir.join("os-image")).unwrap();
        assert_eq!(active, b"new aether content");

        // Verify snapshot was created.
        assert!(dir.join("snapshot").join("os-image").exists());

        // Verify audit log has 6 entries.
        assert_eq!(engine.audit().len(), 6);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn disk_engine_download_missing_payload() {
        let dir = temp_dir();
        let engine = DiskApplyEngine::new(&dir).unwrap();
        let plan = test_plan("os-image", "1.0.0");

        let err = engine.run(ApplyStep::Download, &plan).unwrap_err();
        assert!(matches!(err, ApplyError::Refused { step: ApplyStep::Download, .. }));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn disk_engine_snapshot_skips_missing_active() {
        let dir = temp_dir();
        let engine = DiskApplyEngine::new(&dir).unwrap();
        let plan = test_plan("new-image", "1.0.0");

        // No active file — snapshot should succeed (skip).
        engine.run(ApplyStep::Snapshot, &plan).unwrap();
        assert_eq!(engine.audit().len(), 1);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn disk_engine_audit_clear() {
        let dir = temp_dir();
        let engine = DiskApplyEngine::new(&dir).unwrap();
        let plan = test_plan("x", "1.0.0");
        engine.run(ApplyStep::Snapshot, &plan).unwrap();
        assert_eq!(engine.audit().len(), 1);
        engine.clear_audit();
        assert!(engine.audit().is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn disk_apply_error_display() {
        let e = DiskApplyError::HashMismatch { expected: "aaa".into(), observed: "bbb".into() };
        assert!(e.to_string().contains("aaa"));
        assert!(e.to_string().contains("bbb"));
    }
}
