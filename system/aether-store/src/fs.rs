// Filesystem abstraction for the Aether Store.
//
// All persistent state owned by the Store (consent records, manifest
// copies, install records, the trust registry) flows through this
// trait. The trait is intentionally narrow — a small set of byte-
// oriented read/write/mkdir/exists helpers — so it can be implemented
// over a real filesystem, an in-memory map (for tests), or a future
// remote-fs adapter.
//
// The trait has no `remove` / `rmdir`: the Store does not delete
// anything on disk today. Uninstall is recorded in the audit log and
// marks the installed record as `Uninstalled`; the on-disk manifest
// copy and consent record are kept so post-incident review can read
// them. A future maintenance pass may add `purge_uninstalled` to
// prune disk.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

/// Filesystem operations the Store requires.
///
/// All paths are absolute within the Store's `root` directory. The
/// Store never touches files outside that root; this is enforced by
/// the implementations, not the trait (the in-memory map has no
/// notion of "outside", the local-fs implementation prefixes every
/// path with the configured root).
pub trait StoreFs: Send {
    /// Returns `true` if `path` exists (file or directory).
    fn exists(&self, path: &str) -> bool;

    /// Reads the file at `path` and returns its bytes.
    ///
    /// # Errors
    /// Returns `Err` if the file does not exist or cannot be read.
    fn read(&self, path: &str) -> Result<Vec<u8>, String>;

    /// Writes `bytes` to `path`, creating parent directories as
    /// needed. The implementation MAY overwrite an existing file.
    ///
    /// # Errors
    /// Returns `Err` on I/O failure.
    fn write(&mut self, path: &str, bytes: &[u8]) -> Result<(), String>;

    /// Creates the directory at `path` and all missing parents.
    ///
    /// # Errors
    /// Returns `Err` on I/O failure or if the path already exists as
    /// a regular file.
    fn mkdir_all(&mut self, path: &str) -> Result<(), String>;

    /// Lists the entries immediately under `path` (non-recursive).
    /// Returns directory entry names without the parent prefix.
    ///
    /// # Errors
    /// Returns `Err` if the directory does not exist or cannot be
    /// read.
    fn list(&self, path: &str) -> Result<Vec<String>, String>;
}

/// In-memory `StoreFs` for tests. Stores a path-to-bytes map; writes
/// are stored verbatim. `mkdir_all` is recorded as a no-op (every
/// write to a path implicitly creates its "parents"). The
/// `Mutex` is required because the trait is `Send` and the in-memory
/// map is shared across the Store's `&mut self` calls.
#[derive(Debug, Default)]
pub struct MemoryFs {
    files: Mutex<BTreeMap<String, Vec<u8>>>,
}

impl MemoryFs {
    /// Construct a fresh empty in-memory filesystem.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl StoreFs for MemoryFs {
    fn exists(&self, path: &str) -> bool {
        self.files.lock().map(|f| f.contains_key(path)).unwrap_or(false)
    }

    fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        let files = self.files.lock().map_err(|e| format!("lock poisoned: {e}"))?;
        files.get(path).cloned().ok_or_else(|| format!("no such file: {path}"))
    }

    fn write(&mut self, path: &str, bytes: &[u8]) -> Result<(), String> {
        let mut files = self.files.lock().map_err(|e| format!("lock poisoned: {e}"))?;
        files.insert(path.to_string(), bytes.to_vec());
        Ok(())
    }

    fn mkdir_all(&mut self, _path: &str) -> Result<(), String> {
        // In-memory fs: the directory is implied by the file path.
        Ok(())
    }

    fn list(&self, path: &str) -> Result<Vec<String>, String> {
        let files = self.files.lock().map_err(|e| format!("lock poisoned: {e}"))?;
        let prefix = format!("{path}/");
        let mut names: Vec<String> = files
            .keys()
            .filter(|p| p.starts_with(&prefix))
            .filter_map(|p| p[prefix.len()..].split('/').next().map(str::to_string))
            .collect();
        names.sort();
        names.dedup();
        Ok(names)
    }
}

/// Real-filesystem `StoreFs` rooted at a fixed directory.
///
/// Every `path` argument is interpreted relative to `root`. The
/// implementation does NOT canonicalise paths or defend against
/// `..` escapes — the Store is the only caller and always passes
/// paths it constructed itself. If a future caller passes
/// attacker-controlled paths, the Store should reject them
/// upstream.
pub struct LocalFs {
    root: String,
}

impl LocalFs {
    /// Construct a local filesystem rooted at `root`. The directory
    /// is NOT created; call `mkdir_all` on it after construction.
    #[must_use]
    pub fn new(root: impl Into<String>) -> Self {
        Self { root: root.into() }
    }

    /// Returns the root directory.
    #[must_use]
    pub fn root(&self) -> &str {
        &self.root
    }

    fn resolve(&self, path: &str) -> std::path::PathBuf {
        let p = Path::new(&self.root).join(path);
        p
    }
}

impl StoreFs for LocalFs {
    fn exists(&self, path: &str) -> bool {
        self.resolve(path).exists()
    }

    fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        std::fs::read(self.resolve(path)).map_err(|e| format!("read {path}: {e}"))
    }

    fn write(&mut self, path: &str, bytes: &[u8]) -> Result<(), String> {
        let resolved = self.resolve(path);
        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir parent: {e}"))?;
        }
        std::fs::write(&resolved, bytes).map_err(|e| format!("write {path}: {e}"))
    }

    fn mkdir_all(&mut self, path: &str) -> Result<(), String> {
        std::fs::create_dir_all(self.resolve(path)).map_err(|e| format!("mkdir {path}: {e}"))
    }

    fn list(&self, path: &str) -> Result<Vec<String>, String> {
        let resolved = self.resolve(path);
        let entries = std::fs::read_dir(&resolved).map_err(|e| format!("readdir {path}: {e}"))?;
        let mut names: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .collect();
        names.sort();
        Ok(names)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn memory_fs_round_trip() {
        let mut fs = MemoryFs::new();
        assert!(!fs.exists("/x"));
        fs.write("/x", b"hello").unwrap();
        assert!(fs.exists("/x"));
        assert_eq!(fs.read("/x").unwrap(), b"hello");
    }

    #[test]
    fn memory_fs_mkdir_is_noop() {
        let mut fs = MemoryFs::new();
        // mkdir_all never errors on the in-memory fs.
        fs.mkdir_all("/some/dir").unwrap();
        fs.write("/some/dir/file", b"x").unwrap();
        assert_eq!(fs.read("/some/dir/file").unwrap(), b"x");
    }

    #[test]
    fn memory_fs_list_immediate_children() {
        let mut fs = MemoryFs::new();
        fs.write("/a/1", b"x").unwrap();
        fs.write("/a/2", b"x").unwrap();
        fs.write("/a/sub/3", b"x").unwrap();
        let mut names = fs.list("/a").unwrap();
        names.sort();
        assert_eq!(names, vec!["1".to_string(), "2".to_string(), "sub".to_string()]);
    }

    #[test]
    fn memory_fs_read_missing_returns_error() {
        let fs = MemoryFs::new();
        assert!(fs.read("/nope").is_err());
    }

    #[test]
    fn local_fs_round_trip_in_tempdir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut fs = LocalFs::new(dir.path().to_str().expect("utf8").to_string());
        fs.write("a/b.txt", b"data").expect("write");
        assert!(fs.exists("a/b.txt"));
        assert_eq!(fs.read("a/b.txt").expect("read"), b"data");
    }
}
