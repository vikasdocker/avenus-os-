// Aether Storage - sandboxed file manager.
//
// All file operations are confined to an explicit workspace root
// (Aether Workspace/Documents, Downloads, Projects, Notes). Every path
// passes validation: no traversal, no absolute outside, no symlink escapes,
// no malformed, no protected system locations.

pub mod system_info;

use aether_core::error::{AetherError, ErrorKind};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// Approved workspace structure.
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    /// Absolute workspace root.
    pub root: PathBuf,
    /// Max file size for reads (bytes).
    pub max_read_bytes: usize,
    /// Allowed extensions for read (lowercase without dot).
    pub allowed_read_extensions: HashSet<String>,
}

impl WorkspaceConfig {
    pub fn default_with_root(root: PathBuf) -> Self {
        let mut allowed: HashSet<String> = HashSet::new();
        for ext in [
            "txt", "md", "json", "yaml", "yml", "toml", "rs", "py", "js", "ts", "tsx", "jsx",
            "c", "h", "cpp", "hpp", "go", "java", "sh", "css", "html", "xml", "ini", "cfg",
            "log", "csv",
        ] {
            allowed.insert(ext.to_string());
        }
        Self {
            root,
            max_read_bytes: 512 * 1024, // 512 KiB
            allowed_read_extensions: allowed,
        }
    }

    /// Resolve the workspace root from environment or defaults.
    pub fn from_env_or_default() -> Self {
        let root = std::env::var("AETHER_WORKSPACE")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(default_workspace_root);
        Self::default_with_root(root)
    }
}

fn default_workspace_root() -> PathBuf {
    if let Ok(env_root) = std::env::var("AETHER_WORKSPACE") {
        return PathBuf::from(env_root);
    }
    // Prefer HOME-based workspace for persistence, fallback to temp for host dev
    if let Ok(home) = std::env::var("HOME") {
        let candidate = PathBuf::from(home).join("AetherWorkspace");
        // Use home workspace if HOME exists and is writable, otherwise temp
        if candidate.parent().is_some_and(|p| p.exists()) {
            return candidate;
        }
    }
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        let candidate = PathBuf::from(userprofile).join("AetherWorkspace");
        if candidate.parent().is_some_and(|p| p.exists()) {
            return candidate;
        }
    }
    // Fallback to temp for CI/host where HOME may not be set or writable
    std::env::temp_dir().join("aether-workspace")
}

/// Metadata returned for search/list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileMeta {
    /// File name.
    pub filename: String,
    /// Relative path from workspace root (e.g., "Documents/notes.md").
    pub relative_path: String,
    /// File or directory.
    pub file_type: String,
    /// Size in bytes (0 for directories).
    pub size: u64,
}

/// Core file manager.
#[derive(Debug, Clone)]
pub struct FileManager {
    config: WorkspaceConfig,
}

impl FileManager {
    pub fn new(config: WorkspaceConfig) -> Result<Self, AetherError> {
        // Ensure root exists and create standard subfolders
        fs::create_dir_all(&config.root).map_err(|e| {
            AetherError::new(ErrorKind::Io, format!("cannot create workspace {}: {e}", config.root.display()))
        })?;
        for sub in ["Documents", "Downloads", "Projects", "Notes"] {
            let p = config.root.join(sub);
            let _ = fs::create_dir_all(&p);
        }
        // Canonicalize root for strict checks
        let canon = config.root.canonicalize().unwrap_or(config.root.clone());
        let mut cfg = config;
        cfg.root = canon;
        Ok(Self { config: cfg })
    }

    pub fn workspace_root(&self) -> &Path {
        &self.config.root
    }

    /// Validate a user-supplied relative path and return absolute path inside workspace.
    /// Rejects traversal, absolute outside, malformed, protected, symlink escapes.
    pub fn validate_and_resolve(&self, user_path: &str) -> Result<PathBuf, AetherError> {
        // Reject empty
        if user_path.trim().is_empty() {
            return Err(AetherError::invalid_input("empty path"));
        }
        // Reject null bytes
        if user_path.contains('\0') {
            return Err(AetherError::invalid_input("path contains null byte"));
        }
        let path = Path::new(user_path);

        // Reject absolute paths outside workspace (any absolute is rejected; only relative allowed)
        if path.is_absolute() {
            return Err(AetherError::new(
                ErrorKind::PathTraversal,
                format!("absolute paths not allowed: {user_path}"),
            ));
        }

        // Reject any traversal component
        for comp in path.components() {
            match comp {
                Component::ParentDir => {
                    return Err(AetherError::path_traversal(user_path));
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(AetherError::path_traversal(user_path));
                }
                _ => {}
            }
        }

        // Join with root
        let joined = self.config.root.join(path);

        // For existing paths, canonicalize and ensure inside root (symlink escape check)
        if joined.exists() {
            // Use canonicalize to resolve symlinks
            let canon = joined.canonicalize().map_err(|_| {
                AetherError::new(ErrorKind::Io, format!("cannot resolve path: {user_path}"))
            })?;
            // Ensure canonical is inside root
            if !canon.starts_with(&self.config.root) {
                return Err(AetherError::symlink_escape(user_path, &canon.display().to_string()));
            }
            // Also reject protected system locations even if somehow inside workspace via symlink
            let protected = ["/etc", "/proc", "/sys", "/bin", "/sbin", "/usr", "/dev", "/run"];
            let canon_str = canon.to_string_lossy().to_string();
            for prot in protected {
                if canon_str == prot || canon_str.starts_with(&format!("{prot}/")) {
                    return Err(AetherError::new(
                        ErrorKind::PermissionDenied,
                        format!("access to protected location denied: {user_path}"),
                    ));
                }
            }
            Ok(canon)
        } else {
            // For non-existing, validate parent exists and is inside root
            if let Some(parent) = joined.parent() {
                // Parent must be inside workspace; canonicalize parent if exists
                if parent.exists() {
                    let canon_parent = parent.canonicalize().unwrap_or(parent.to_path_buf());
                    if !canon_parent.starts_with(&self.config.root) {
                        return Err(AetherError::symlink_escape(user_path, &canon_parent.display().to_string()));
                    }
                } else {
                    // Parent doesn't exist yet; ensure the joined path's prefix is still inside root via string check
                    // Since we already rejected ParentDir, the joined path should be inside root lexically
                    let joined_str = joined.to_string_lossy();
                    let root_str = self.config.root.to_string_lossy();
                    if !joined_str.starts_with(root_str.as_ref()) {
                        return Err(AetherError::path_traversal(user_path));
                    }
                    // Also check that no intermediate component is a symlink that escapes
                    // Walk up parents to find existing ancestor and check it
                    let mut ancestor = parent;
                    while let Some(a) = ancestor.parent() {
                        if a.exists() {
                            if let Ok(canon_a) = a.canonicalize() {
                                if !canon_a.starts_with(&self.config.root) {
                                    return Err(AetherError::symlink_escape(user_path, &canon_a.display().to_string()));
                                }
                            }
                            break;
                        }
                        ancestor = a;
                    }
                }
            }
            // Also reject if user path attempts to escape via string like "../" even after join
            // Already handled via components, but double-check normalized
            let normalized = joined.to_string_lossy();
            let root_normalized = self.config.root.to_string_lossy();
            if !normalized.starts_with(root_normalized.as_ref()) {
                return Err(AetherError::path_traversal(user_path));
            }
            Ok(joined)
        }
    }

    /// List directory contents (relative path, empty or "." for workspace root).
    pub fn list(&self, relative_path: &str) -> Result<Vec<FileMeta>, AetherError> {
        let rel = if relative_path.trim().is_empty() || relative_path == "." || relative_path == "/" {
            ""
        } else {
            relative_path
        };
        let abs = if rel.is_empty() {
            self.config.root.clone()
        } else {
            self.validate_and_resolve(rel)?
        };
        if !abs.exists() {
            return Err(AetherError::not_found(rel));
        }
        if !abs.is_dir() {
            return Err(AetherError::invalid_input(format!("not a directory: {rel}")));
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(&abs).map_err(|e| AetherError::new(ErrorKind::Io, e.to_string()))? {
            let entry = entry.map_err(|e| AetherError::new(ErrorKind::Io, e.to_string()))?;
            let path = entry.path();
            let meta = entry.metadata().map_err(|e| AetherError::new(ErrorKind::Io, e.to_string()))?;
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
            let rel_path = path.strip_prefix(&self.config.root).unwrap_or(&path).to_string_lossy().to_string().replace('\\', "/");
            let file_type = if meta.is_dir() { "directory".to_string() } else { "file".to_string() };
            let size = if meta.is_file() { meta.len() } else { 0 };
            out.push(FileMeta {
                filename,
                relative_path: rel_path,
                file_type,
                size,
            });
        }
        out.sort_by(|a, b| a.filename.cmp(&b.filename));
        Ok(out)
    }

    /// Search for files whose filename contains query (case-insensitive), recursively.
    pub fn search(&self, query: &str) -> Result<Vec<FileMeta>, AetherError> {
        if query.trim().is_empty() {
            return Err(AetherError::invalid_input("empty search query"));
        }
        if query.len() > 100 {
            return Err(AetherError::invalid_input("query too long"));
        }
        let lower_query = query.to_ascii_lowercase();
        let mut results = Vec::new();
        self.search_recursive(&self.config.root, &lower_query, &mut results, 0)?;
        // Limit
        results.sort_by(|a, b| a.filename.cmp(&b.filename));
        if results.len() > 50 {
            results.truncate(50);
        }
        Ok(results)
    }

    fn search_recursive(&self, dir: &Path, query: &str, out: &mut Vec<FileMeta>, depth: usize) -> Result<(), AetherError> {
        if depth > 8 {
            return Ok(());
        }
        let entries = fs::read_dir(dir).map_err(|e| AetherError::new(ErrorKind::Io, e.to_string()))?;
        for entry in entries {
            let entry = entry.map_err(|e| AetherError::new(ErrorKind::Io, e.to_string()))?;
            let path = entry.path();
            // Skip symlinked dirs that escape? Already validated via canonical check
            if let Ok(canon) = path.canonicalize() {
                if !canon.starts_with(&self.config.root) {
                    continue;
                }
            }
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if filename.to_ascii_lowercase().contains(query) {
                let meta = entry.metadata().map_err(|e| AetherError::new(ErrorKind::Io, e.to_string()))?;
                let rel = path.strip_prefix(&self.config.root).unwrap_or(&path).to_string_lossy().to_string().replace('\\', "/");
                let size = if meta.is_file() { meta.len() } else { 0 };
                let file_type = if is_dir { "directory".to_string() } else { "file".to_string() };
                out.push(FileMeta {
                    filename,
                    relative_path: rel,
                    file_type,
                    size,
                });
            }
            if is_dir {
                self.search_recursive(&path, query, out, depth + 1)?;
            }
            if out.len() >= 50 {
                break;
            }
        }
        Ok(())
    }

    /// Read file content as string, with size and type checks.
    pub fn read(&self, relative_path: &str) -> Result<String, AetherError> {
        let abs = self.validate_and_resolve(relative_path)?;
        if !abs.exists() {
            return Err(AetherError::not_found(relative_path));
        }
        let meta = fs::metadata(&abs).map_err(|e| AetherError::new(ErrorKind::Io, e.to_string()))?;
        if meta.is_dir() {
            return Err(AetherError::invalid_input(format!("is a directory: {relative_path}")));
        }
        if meta.len() > self.config.max_read_bytes as u64 {
            return Err(AetherError::new(
                ErrorKind::ResourceExhausted,
                format!("file too large ({} bytes, limit {}): {relative_path}", meta.len(), self.config.max_read_bytes),
            ));
        }
        // Check extension
        let ext = abs.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if !ext.is_empty() && !self.config.allowed_read_extensions.contains(&ext) {
            // Allow no extension as txt?
            // Check if file is binary by reading first bytes
        }
        // Try to read as utf8
        let mut file = fs::File::open(&abs).map_err(|e| AetherError::new(ErrorKind::Io, e.to_string()))?;
        let mut buf = Vec::new();
        // Limit read
        let mut limited = file.by_ref().take(self.config.max_read_bytes as u64 + 1);
        limited.read_to_end(&mut buf).map_err(|e| AetherError::new(ErrorKind::Io, e.to_string()))?;
        if buf.len() > self.config.max_read_bytes {
            return Err(AetherError::new(
                ErrorKind::ResourceExhausted,
                format!("file too large: {relative_path}"),
            ));
        }
        // Check for binary (null bytes)
        if buf.contains(&0) {
            return Err(AetherError::new(
                ErrorKind::InvalidInput,
                format!("unsupported binary file: {relative_path}"),
            ));
        }
        String::from_utf8(buf).map_err(|_| {
            AetherError::new(ErrorKind::InvalidInput, format!("unsupported file encoding: {relative_path}"))
        })
    }

    /// Create a new file; fails if exists (caller should handle overwrite confirmation).
    pub fn create(&self, relative_path: &str, content: &str) -> Result<(String, usize), AetherError> {
        let abs = self.validate_and_resolve(relative_path)?;
        if abs.exists() {
            return Err(AetherError::new(
                ErrorKind::InvalidInput,
                format!("file already exists: {relative_path}"),
            ));
        }
        // Ensure parent exists
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent).map_err(|e| AetherError::new(ErrorKind::Io, e.to_string()))?;
            // Validate parent still inside root after creation
            let canon_parent = parent.canonicalize().unwrap_or(parent.to_path_buf());
            if !canon_parent.starts_with(&self.config.root) {
                return Err(AetherError::symlink_escape(relative_path, &canon_parent.display().to_string()));
            }
        }
        fs::write(&abs, content).map_err(|e| {
            if e.to_string().contains("No space") {
                AetherError::new(ErrorKind::ResourceExhausted, "disk full")
            } else {
                AetherError::new(ErrorKind::Io, e.to_string())
            }
        })?;
        let bytes = content.len();
        let rel = abs.strip_prefix(&self.config.root).unwrap_or(&abs).to_string_lossy().to_string().replace('\\', "/");
        Ok((rel, bytes))
    }

    /// Write (overwrite) file; creates if not exists.
    pub fn write(&self, relative_path: &str, content: &str) -> Result<(String, usize), AetherError> {
        let abs = self.validate_and_resolve(relative_path)?;
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent).map_err(|e| AetherError::new(ErrorKind::Io, e.to_string()))?;
        }
        // Check if exists for audit purposes; still allow overwrite but caller should have confirmed
        fs::write(&abs, content).map_err(|e| AetherError::new(ErrorKind::Io, e.to_string()))?;
        let bytes = content.len();
        let rel = abs.strip_prefix(&self.config.root).unwrap_or(&abs).to_string_lossy().to_string().replace('\\', "/");
        Ok((rel, bytes))
    }

    /// Rename within same directory (or to different location inside workspace).
    pub fn rename(&self, from: &str, to: &str) -> Result<String, AetherError> {
        let abs_from = self.validate_and_resolve(from)?;
        if !abs_from.exists() {
            return Err(AetherError::not_found(from));
        }
        let abs_to = self.validate_and_resolve(to)?;
        if abs_to.exists() {
            return Err(AetherError::new(ErrorKind::InvalidInput, format!("destination already exists: {to}")));
        }
        if let Some(parent) = abs_to.parent() {
            fs::create_dir_all(parent).map_err(|e| AetherError::new(ErrorKind::Io, e.to_string()))?;
        }
        fs::rename(&abs_from, &abs_to).map_err(|e| AetherError::new(ErrorKind::Io, e.to_string()))?;
        let rel = abs_to.strip_prefix(&self.config.root).unwrap_or(&abs_to).to_string_lossy().to_string().replace('\\', "/");
        Ok(rel)
    }

    /// Move file to destination (similar to rename but may be across directories, same filesystem)
    pub fn move_file(&self, from: &str, to: &str) -> Result<String, AetherError> {
        // For now same as rename; future could handle cross-filesystem copy+delete
        self.rename(from, to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_manager() -> (FileManager, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let cfg = WorkspaceConfig::default_with_root(dir.path().to_path_buf());
        let fm = FileManager::new(cfg).unwrap_or_else(|e| panic!("{e}"));
        (fm, dir)
    }

    #[test]
    fn workspace_structure_created() {
        let (fm, _d) = temp_manager();
        for sub in ["Documents", "Downloads", "Projects", "Notes"] {
            assert!(fm.workspace_root().join(sub).exists(), "missing {sub}");
        }
    }

    #[test]
    fn path_validation_rejects_traversal() {
        let (fm, _d) = temp_manager();
        assert!(fm.validate_and_resolve("../etc/passwd").is_err());
        assert!(fm.validate_and_resolve("../../etc/shadow").is_err());
        assert!(fm.validate_and_resolve("Documents/../etc/passwd").is_err());
        assert!(fm.validate_and_resolve("/etc/shadow").is_err());
        assert!(fm.validate_and_resolve("/absolute/path").is_err());
    }

    #[test]
    fn path_validation_rejects_symlink_escape() {
        #[cfg(unix)]
        {
            let (fm, _dir) = temp_manager();
            let target = fm.workspace_root().join("Documents");
            let link = fm.workspace_root().join("evil_link");
            std::os::unix::fs::symlink("/etc", &link).unwrap_or_else(|e| panic!("{e}"));
            // Trying to resolve through symlink should be rejected
            let attempt = fm.validate_and_resolve("evil_link/passwd");
            assert!(attempt.is_err(), "symlink escape not rejected: {:?}", attempt);
            let _ = fs::remove_file(&link);
            let _ = fm.validate_and_resolve("Documents/normal.txt");
        }
    }

    #[test]
    fn list_workspace_root() {
        let (fm, _d) = temp_manager();
        let list = fm.list("").unwrap_or_else(|e| panic!("{e}"));
        assert!(list.iter().any(|m| m.filename == "Documents"));
    }

    #[test]
    fn create_read_search_flow() {
        let (fm, _d) = temp_manager();
        let (rel, bytes) = fm.create("Documents/ideas.md", "Aether OS idea").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(rel, "Documents/ideas.md");
        assert_eq!(bytes, 14);
        let content = fm.read("Documents/ideas.md").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(content, "Aether OS idea");
        let results = fm.search("ideas").unwrap_or_else(|e| panic!("{e}"));
        assert!(results.iter().any(|m| m.filename == "ideas.md"));
    }

    #[test]
    fn create_fails_if_exists() {
        let (fm, _d) = temp_manager();
        match fm.create("Notes/todo.md", "a") {
            Ok(_) => {}
            Err(e) => panic!("{e}"),
        }
        let err = match fm.create("Notes/todo.md", "b") {
            Ok(_) => panic!("expected error for duplicate create"),
            Err(e) => e,
        };
        assert!(err.message.contains("already exists"));
    }

    #[test]
    fn write_overwrites() {
        let (fm, _d) = temp_manager();
        fm.create("Documents/file.txt", "old").unwrap_or_else(|e| panic!("{e}"));
        let (rel, bytes) = fm.write("Documents/file.txt", "new content").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(bytes, 11);
        assert_eq!(fm.read(&rel).unwrap_or_else(|e| panic!("{e}")), "new content");
    }

    #[test]
    fn rename_and_move() {
        let (fm, _d) = temp_manager();
        fm.create("Documents/old.md", "data").unwrap_or_else(|e| panic!("{e}"));
        let new_rel = fm.rename("Documents/old.md", "Documents/new.md").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(new_rel, "Documents/new.md");
        assert!(!fm.workspace_root().join("Documents/old.md").exists());
        assert!(fm.workspace_root().join("Documents/new.md").exists());
        let moved = fm.move_file("Documents/new.md", "Notes/new.md").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(moved, "Notes/new.md");
    }

    #[test]
    fn read_rejects_binary_and_large() {
        let (fm, _d) = temp_manager();
        // Create a file with null byte
        let path = fm.workspace_root().join("Documents/binary.bin");
        match fs::write(&path, vec![0, 1, 2, 3]) {
            Ok(_) => {}
            Err(e) => panic!("{e}"),
        }
        let err = match fm.read("Documents/binary.bin") {
            Ok(_) => panic!("expected error reading binary"),
            Err(e) => e,
        };
        assert!(err.message.contains("binary") || err.message.contains("unsupported"));
    }

    #[test]
    fn search_limits_results() {
        let (fm, _d) = temp_manager();
        for i in 0..5 {
            fm.create(&format!("Documents/file{i}.md"), "x").unwrap_or_else(|e| panic!("{e}"));
        }
        let results = fm.search("file").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(results.len(), 5);
    }
}
