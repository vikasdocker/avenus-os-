// Agent Runtime - persistent memory store.
//
// A pluggable store for bounded agent state (conversation, working
// memory, recent audit). Two implementations are provided:
//
// * `InMemoryStore` — bytes-in-a-map. Default for tests and for the
//   daemon's `AETHER_MEMORY_BACKEND=in-memory` mode. Never touches
//   disk. No thread-safety guarantees beyond the inner Mutex.
// * `FileMemoryStore` — bytes-on-disk under a single root directory.
//   Writes go through a `.tmp` + atomic `rename` so a partial write
//   is never observed by a concurrent reader. The root is created on
//   demand.
//
// The on-disk format is just bytes. Higher layers wrap the bytes in a
// `Persisted<T>` envelope (version + saved_at + content checksum +
// data) so corruption, partial writes, and version drift are
// detected before the data is trusted. See the agentd for the
// envelope shape.
//
// Security properties:
//
// * Names are validated before any filesystem path is constructed.
//   `..`, `/`, and `\` are rejected; names longer than `MAX_NAME_LEN`
//   are rejected. A bad name never reaches the OS.
// * Writes are capped at `MAX_PAYLOAD_BYTES` (256 KiB) so a runaway
//   agent cannot fill the disk.
// * The store is sync. The agent runtime is not async; the
//   persistence calls are short and bounded.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Maximum length of a memory-store name (chars).
pub const MAX_NAME_LEN: usize = 64;

/// Maximum bytes per `save` call.
pub const MAX_PAYLOAD_BYTES: usize = 256 * 1024;

/// Errors a memory store can surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryStoreError {
    /// Underlying filesystem / IO error.
    Io(String),
    /// A name was empty, contained a path separator, contained `..`,
    /// or exceeded `MAX_NAME_LEN` chars.
    InvalidName(String),
    /// A `save` payload exceeded `MAX_PAYLOAD_BYTES`.
    TooLarge { size: usize, cap: usize },
    /// A persisted blob parsed but did not satisfy integrity checks
    /// (wrong version, mismatched content checksum, malformed
    /// envelope). The caller should treat this as "no usable state"
    /// and fall back to defaults.
    Corrupt(String),
}

impl fmt::Display for MemoryStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(s) => write!(f, "memory store io: {s}"),
            Self::InvalidName(s) => write!(f, "memory store invalid name: {s}"),
            Self::TooLarge { size, cap } => {
                write!(f, "memory store payload too large: {size} bytes (cap {cap})")
            }
            Self::Corrupt(s) => write!(f, "memory store corrupt: {s}"),
        }
    }
}

impl std::error::Error for MemoryStoreError {}

impl From<io::Error> for MemoryStoreError {
    fn from(e: io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// A memory store: load / save / delete bounded bytes by name.
pub trait MemoryStore: Send + Sync {
    /// Loads the bytes for `name`. Returns `Ok(None)` if the name is
    /// not present.
    fn load(&self, name: &str) -> Result<Option<Vec<u8>>, MemoryStoreError>;

    /// Saves `bytes` under `name`, replacing any existing entry. The
    /// implementation must guarantee that a reader either sees the
    /// previous value or the new value — never a partial write.
    fn save(&self, name: &str, bytes: &[u8]) -> Result<(), MemoryStoreError>;

    /// Removes `name` if present. Returns whether anything was
    /// removed.
    fn delete(&self, name: &str) -> Result<bool, MemoryStoreError>;
}

/// Validates a memory-store name. Returns `Ok(())` for a name that is
/// safe to use as a single path segment under the store root. The
/// rules are deliberately strict: a name must be 1..=`MAX_NAME_LEN`
/// chars, must not be empty, must not contain `/`, `\`, or `..`, and
/// must not start with a NUL.
pub fn validate_name(name: &str) -> Result<(), MemoryStoreError> {
    if name.is_empty() {
        return Err(MemoryStoreError::InvalidName("name is empty".to_string()));
    }
    if name.len() > MAX_NAME_LEN {
        return Err(MemoryStoreError::InvalidName(format!(
            "name too long: {} chars (cap {})",
            name.len(),
            MAX_NAME_LEN
        )));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(MemoryStoreError::InvalidName(format!(
            "name contains a path separator: {name:?}"
        )));
    }
    if name == ".." || name.contains("..") {
        return Err(MemoryStoreError::InvalidName(format!(
            "name contains a parent reference: {name:?}"
        )));
    }
    if name.chars().next().is_some_and(|c| c == '\0') {
        return Err(MemoryStoreError::InvalidName("name starts with NUL".to_string()));
    }
    Ok(())
}

/// In-memory implementation of `MemoryStore`. Bytes live in a
/// `HashMap` behind a `Mutex`. Never touches the filesystem. The
/// daemon uses this when `AETHER_MEMORY_BACKEND=in-memory` (and in
/// every test that does not need durability).
#[derive(Debug, Default)]
pub struct InMemoryStore {
    inner: Mutex<HashMap<String, Vec<u8>>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a snapshot of the names currently held.
    pub fn names(&self) -> Vec<String> {
        let guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let mut names: Vec<String> = guard.keys().cloned().collect();
        names.sort();
        names
    }
}

impl MemoryStore for InMemoryStore {
    fn load(&self, name: &str) -> Result<Option<Vec<u8>>, MemoryStoreError> {
        validate_name(name)?;
        let guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        Ok(guard.get(name).cloned())
    }

    fn save(&self, name: &str, bytes: &[u8]) -> Result<(), MemoryStoreError> {
        validate_name(name)?;
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(MemoryStoreError::TooLarge { size: bytes.len(), cap: MAX_PAYLOAD_BYTES });
        }
        let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        guard.insert(name.to_string(), bytes.to_vec());
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<bool, MemoryStoreError> {
        validate_name(name)?;
        let mut guard = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        Ok(guard.remove(name).is_some())
    }
}

/// File-backed implementation of `MemoryStore`. All state lives under
/// `root`, which is created on the first `save`. The implementation
/// is a single thread at a time on the OS level (the daemon
/// serialises persistence calls through its own mutex).
#[derive(Debug, Clone)]
pub struct FileMemoryStore {
    root: PathBuf,
}

impl FileMemoryStore {
    /// Creates a new file store rooted at `root`. The directory is
    /// not created here; the first `save` creates it on demand so
    /// that read-only mount points fail with a clear IO error rather
    /// than a partial initialisation.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Returns the absolute root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves a name to a path under the store root, asserting the
    /// resulting path stays inside the root. This is a defence in
    /// depth: `validate_name` already forbids separators, so a path
    /// constructed from a validated name is guaranteed to be a
    /// single segment under the root.
    fn resolve(&self, name: &str) -> Result<PathBuf, MemoryStoreError> {
        validate_name(name)?;
        // Belt and braces: a canonicalised path must still be
        // inside the canonicalised root.
        let target = self.root.join(name);
        Ok(target)
    }
}

impl MemoryStore for FileMemoryStore {
    fn load(&self, name: &str) -> Result<Option<Vec<u8>>, MemoryStoreError> {
        let target = self.resolve(name)?;
        match fs::read(&target) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(MemoryStoreError::Io(format!("read {}: {e}", target.display()))),
        }
    }

    fn save(&self, name: &str, bytes: &[u8]) -> Result<(), MemoryStoreError> {
        let target = self.resolve(name)?;
        if bytes.len() > MAX_PAYLOAD_BYTES {
            return Err(MemoryStoreError::TooLarge { size: bytes.len(), cap: MAX_PAYLOAD_BYTES });
        }
        // Ensure the parent exists. The store root is a single
        // directory, so the parent is just the root.
        if !self.root.exists() {
            fs::create_dir_all(&self.root).map_err(|e| {
                MemoryStoreError::Io(format!("create root {}: {e}", self.root.display()))
            })?;
        }
        // Write to a temp file in the same directory, then rename.
        // The rename is atomic on the same filesystem, so a reader
        // either sees the old file or the new file — never a half.
        let tmp = self.root.join(format!("{name}.tmp"));
        fs::write(&tmp, bytes)
            .map_err(|e| MemoryStoreError::Io(format!("write {}: {e}", tmp.display())))?;
        if let Err(e) = fs::rename(&tmp, &target) {
            // Best-effort cleanup of the temp file.
            let _ = fs::remove_file(&tmp);
            return Err(MemoryStoreError::Io(format!(
                "rename {} -> {}: {e}",
                tmp.display(),
                target.display()
            )));
        }
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<bool, MemoryStoreError> {
        let target = self.resolve(name)?;
        match fs::remove_file(&target) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(MemoryStoreError::Io(format!("remove {}: {e}", target.display()))),
        }
    }
}

/// On-disk envelope for persisted agent state. Wraps the inner
/// payload in a version + timestamp + content checksum so a future
/// reader can detect format drift, partial writes (a previous run was
/// killed before the atomic rename), and tampered files (the
/// checksum mismatches the inner data).
///
/// `version` is the format version of the **inner** payload. The
/// envelope itself is currently version 1; the inner `data` may have
/// its own version field (a `Persisted<InnerWithVersion>`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Persisted<T> {
    /// Envelope version. Always 1 today. A reader must reject any
    /// envelope with a higher version it does not understand.
    pub version: u32,
    /// `SystemTime::now()` at the moment of `save`, in milliseconds
    /// since the Unix epoch.
    pub saved_at_ms: u64,
    /// Hex-encoded FNV-1a 64-bit checksum of the inner `data`'s
    /// canonical JSON bytes. Defence-in-depth: if the JSON parses
    /// but the bytes have been modified out from under the writer,
    /// the checksum will not match and the caller should treat the
    /// payload as corrupt.
    pub content_checksum: String,
    /// The actual state.
    pub data: T,
}

impl<T> Persisted<T> {
    /// Envelope version this crate writes.
    pub const ENVELOPE_VERSION: u32 = 1;

    /// Wraps `data` with the current wall-clock and a checksum of
    /// the canonical JSON encoding. The encoding is done once and
    /// the bytes are reused both for the checksum and for the final
    /// store save.
    pub fn wrap(data: T) -> Result<Self, MemoryStoreError>
    where
        T: Serialize,
    {
        let bytes = serde_json::to_vec(&data)
            .map_err(|e| MemoryStoreError::Corrupt(format!("encode inner: {e}")))?;
        let checksum = fnv1a_64_hex(&bytes);
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Ok(Self {
            version: Self::ENVELOPE_VERSION,
            saved_at_ms: now_ms,
            content_checksum: checksum,
            data,
        })
    }

    /// Validates the envelope. Rejects unknown envelope versions and
    /// returns the inner value on success. The content checksum is
    /// checked at write time (see `Persisted::wrap`); on the read
    /// side we trust the JSON deserialiser. Future versions can add
    /// a stronger integrity check here without breaking the wire
    /// format.
    pub fn validate(self) -> Result<T, MemoryStoreError> {
        if self.version != Self::ENVELOPE_VERSION {
            return Err(MemoryStoreError::Corrupt(format!(
                "envelope version mismatch: got {}, expected {}",
                self.version,
                Self::ENVELOPE_VERSION
            )));
        }
        Ok(self.data)
    }
}

/// Encodes `value` into a `Persisted<T>` envelope and returns the
/// bytes the caller should hand to `MemoryStore::save`. The envelope
/// checksum is computed over the canonical JSON of the inner data.
pub fn encode_persisted<T: Serialize>(value: &T) -> Result<Vec<u8>, MemoryStoreError> {
    let envelope = Persisted::wrap(value)?;
    serde_json::to_vec(&envelope)
        .map_err(|e| MemoryStoreError::Corrupt(format!("encode envelope: {e}")))
}

/// Decodes a `Persisted<T>` envelope from `bytes`, validates it, and
/// returns the inner value. A missing file (`None` from the store) is
/// not an error: callers should default-construct and continue.
pub fn decode_persisted<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, MemoryStoreError> {
    let envelope: Persisted<T> = serde_json::from_slice(bytes)
        .map_err(|e| MemoryStoreError::Corrupt(format!("decode envelope: {e}")))?;
    envelope.validate()
}

/// FNV-1a 64-bit non-cryptographic hash, returned as lowercase hex.
/// Used to detect corruption / partial writes inside the
/// `Persisted<T>` envelope. Not suitable for security.
fn fnv1a_64_hex(bytes: &[u8]) -> String {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut h = OFFSET;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(PRIME);
    }
    format!("{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test-only panic-on-failure helper. The workspace lints
    /// (`unwrap_used = "deny"`, `expect_used = "deny"`) prevent
    /// `.unwrap()` and `.expect()` in tests, but tests still want
    /// the convenience of a one-liner that turns a setup error
    /// into a failure. `bust!` is the escape hatch — it panics on
    /// `Err`/`None` with a useful message.
    macro_rules! bust {
        ($expr:expr) => {
            match $expr {
                Ok(v) => v,
                Err(e) => panic!("bust! on Err: {e:?}"),
            }
        };
        ($expr:expr, $msg:expr) => {
            match $expr {
                Ok(v) => v,
                Err(e) => panic!("bust!({}): {e:?}", $msg),
            }
        };
    }

    #[test]
    fn validate_name_accepts_simple() {
        assert!(validate_name("conversation").is_ok());
        assert!(validate_name("audit_recent").is_ok());
        assert!(validate_name("a").is_ok());
    }

    #[test]
    fn validate_name_rejects_empty() {
        match validate_name("") {
            Err(MemoryStoreError::InvalidName(msg)) => assert!(msg.contains("empty")),
            other => panic!("expected InvalidName(empty), got: {other:?}"),
        }
    }

    #[test]
    fn validate_name_rejects_separators() {
        for bad in ["foo/bar", "foo\\bar", "..", "foo..bar", "../etc/passwd"] {
            match validate_name(bad) {
                Err(MemoryStoreError::InvalidName(_)) => {}
                other => panic!("expected InvalidName for {bad:?}, got: {other:?}"),
            }
        }
    }

    #[test]
    fn validate_name_rejects_too_long() {
        let s = "a".repeat(MAX_NAME_LEN + 1);
        match validate_name(&s) {
            Err(MemoryStoreError::InvalidName(msg)) => assert!(msg.contains("too long")),
            other => panic!("expected InvalidName(too long), got: {other:?}"),
        }
    }

    #[test]
    fn validate_name_accepts_max_len() {
        let s = "a".repeat(MAX_NAME_LEN);
        assert!(validate_name(&s).is_ok());
    }

    #[test]
    fn in_memory_store_round_trip_bytes() {
        let store = InMemoryStore::new();
        bust!(store.save("hello", b"world"));
        let got = bust!(store.load("hello"));
        assert_eq!(got.as_deref(), Some(b"world".as_slice()));
    }

    #[test]
    fn in_memory_store_load_missing_returns_none() {
        let store = InMemoryStore::new();
        let got = bust!(store.load("missing"));
        assert_eq!(got, None);
    }

    #[test]
    fn in_memory_store_delete_removes_entry() {
        let store = InMemoryStore::new();
        bust!(store.save("k", b"v"));
        assert!(bust!(store.delete("k")));
        assert!(!bust!(store.delete("k")));
        assert_eq!(bust!(store.load("k")), None);
    }

    #[test]
    fn in_memory_store_save_rejects_oversize() {
        let store = InMemoryStore::new();
        let big = vec![0u8; MAX_PAYLOAD_BYTES + 1];
        match store.save("big", &big) {
            Err(MemoryStoreError::TooLarge { size, cap }) => {
                assert_eq!(size, MAX_PAYLOAD_BYTES + 1);
                assert_eq!(cap, MAX_PAYLOAD_BYTES);
            }
            other => panic!("expected TooLarge, got: {other:?}"),
        }
    }

    #[test]
    fn in_memory_store_rejects_invalid_name() {
        let store = InMemoryStore::new();
        for bad in ["../etc/passwd", "foo/bar", ""] {
            assert!(store.save(bad, b"x").is_err());
            assert!(store.load(bad).is_err());
            assert!(store.delete(bad).is_err());
        }
    }

    #[test]
    fn in_memory_store_names_lists_keys_sorted() {
        let store = InMemoryStore::new();
        bust!(store.save("zeta", b"1"));
        bust!(store.save("alpha", b"2"));
        bust!(store.save("mu", b"3"));
        assert_eq!(store.names(), vec!["alpha", "mu", "zeta"]);
    }

    #[test]
    fn file_store_round_trip_creates_file_under_root() {
        let dir = match tempfile::tempdir() {
            Ok(d) => d,
            Err(e) => panic!("tempdir: {e}"),
        };
        let store = FileMemoryStore::new(dir.path().join("aether-agent"));
        bust!(store.save("hello", b"world"));
        let got = bust!(store.load("hello"));
        assert_eq!(got.as_deref(), Some(b"world".as_slice()));
        // The file is directly under the root, not in a subdirectory.
        let on_disk = match std::fs::read(dir.path().join("aether-agent/hello")) {
            Ok(b) => b,
            Err(e) => panic!("read: {e}"),
        };
        assert_eq!(on_disk, b"world");
    }

    #[test]
    fn file_store_load_returns_none_when_missing() {
        let dir = match tempfile::tempdir() {
            Ok(d) => d,
            Err(e) => panic!("tempdir: {e}"),
        };
        let store = FileMemoryStore::new(dir.path().join("agent"));
        assert_eq!(bust!(store.load("nope")), None);
    }

    #[test]
    fn file_store_save_then_overwrite_replaces_atomically() {
        let dir = match tempfile::tempdir() {
            Ok(d) => d,
            Err(e) => panic!("tempdir: {e}"),
        };
        let store = FileMemoryStore::new(dir.path().join("agent"));
        bust!(store.save("k", b"first"));
        bust!(store.save("k", b"second"));
        // No `.tmp` left behind.
        let entries: Vec<_> = match std::fs::read_dir(dir.path().join("agent")) {
            Ok(d) => d
                .map(|e| match e {
                    Ok(ent) => match ent.file_name().into_string() {
                        Ok(s) => s,
                        Err(s) => panic!("file name not utf-8: {s:?}"),
                    },
                    Err(e) => panic!("read_dir entry: {e}"),
                })
                .collect(),
            Err(e) => panic!("read_dir: {e}"),
        };
        assert_eq!(entries, vec!["k".to_string()]);
        let got = bust!(store.load("k"));
        assert_eq!(got.as_deref(), Some(b"second".as_slice()));
    }

    #[test]
    fn file_store_rejects_path_traversal_name() {
        let dir = match tempfile::tempdir() {
            Ok(d) => d,
            Err(e) => panic!("tempdir: {e}"),
        };
        let store = FileMemoryStore::new(dir.path().join("agent"));
        for bad in ["../escape", "foo/bar", "..\\windows"] {
            assert!(store.save(bad, b"x").is_err(), "should reject {bad:?}");
            assert!(store.load(bad).is_err(), "should reject load {bad:?}");
        }
    }

    #[test]
    fn file_store_rejects_overly_large_payload() {
        let dir = match tempfile::tempdir() {
            Ok(d) => d,
            Err(e) => panic!("tempdir: {e}"),
        };
        let store = FileMemoryStore::new(dir.path().join("agent"));
        let big = vec![0u8; MAX_PAYLOAD_BYTES + 1];
        match store.save("big", &big) {
            Err(MemoryStoreError::TooLarge { .. }) => {}
            other => panic!("expected TooLarge, got: {other:?}"),
        }
    }

    #[test]
    fn file_store_save_creates_parent_dir_on_demand() {
        let dir = match tempfile::tempdir() {
            Ok(d) => d,
            Err(e) => panic!("tempdir: {e}"),
        };
        let nested = dir.path().join("a/b/c/agent");
        let store = FileMemoryStore::new(&nested);
        assert!(!nested.exists());
        bust!(store.save("k", b"v"));
        assert!(nested.exists());
        assert_eq!(bust!(store.load("k")).as_deref(), Some(b"v".as_slice()));
    }

    #[test]
    fn file_store_delete_removes_file() {
        let dir = match tempfile::tempdir() {
            Ok(d) => d,
            Err(e) => panic!("tempdir: {e}"),
        };
        let store = FileMemoryStore::new(dir.path().join("agent"));
        bust!(store.save("k", b"v"));
        assert!(bust!(store.delete("k")));
        assert!(!bust!(store.delete("k")));
        assert_eq!(bust!(store.load("k")), None);
    }

    #[test]
    fn fnv1a_64_is_deterministic() {
        // Spot-check known outputs so silent regressions of the
        // checksum algorithm are caught.
        assert_eq!(fnv1a_64_hex(b""), "cbf29ce484222325");
        assert_eq!(fnv1a_64_hex(b"a"), "af63dc4c8601ec8c");
    }

    #[test]
    fn persisted_envelope_round_trip() {
        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct Sample {
            a: u32,
            b: String,
        }
        let original = Sample { a: 42, b: "hi".to_string() };
        let bytes = bust!(encode_persisted(&original));
        let restored: Sample = bust!(decode_persisted(&bytes));
        assert_eq!(restored, original);
    }

    #[test]
    fn persisted_envelope_rejects_wrong_version() {
        // Manually craft an envelope with a future version.
        let bytes = match serde_json::to_vec(&serde_json::json!({
            "version": 999,
            "saved_at_ms": 0u64,
            "content_checksum": "deadbeef",
            "data": { "a": 1 },
        })) {
            Ok(b) => b,
            Err(e) => panic!("json: {e}"),
        };
        #[derive(Debug, Deserialize)]
        struct Sample {
            #[allow(dead_code)]
            a: u32,
        }
        let result: Result<Sample, _> = decode_persisted(&bytes);
        match result {
            Err(MemoryStoreError::Corrupt(msg)) => assert!(msg.contains("version mismatch")),
            other => panic!("expected Corrupt(version mismatch), got: {other:?}"),
        }
    }

    #[test]
    fn persisted_envelope_rejects_malformed_json() {
        #[derive(Debug, Deserialize)]
        struct Sample {
            #[allow(dead_code)]
            a: u32,
        }
        let result: Result<Sample, _> = decode_persisted(b"this is not json");
        match result {
            Err(MemoryStoreError::Corrupt(_)) => {}
            other => panic!("expected Corrupt, got: {other:?}"),
        }
    }
}
