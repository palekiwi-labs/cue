use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Returns the path to the project store JSON file.
/// Respects `CUE_DATA_DIR` for test isolation.
pub fn store_path() -> PathBuf {
    if let Ok(dir) = std::env::var("CUE_DATA_DIR") {
        return PathBuf::from(dir).join("projects.json");
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cue")
        .join("projects.json")
}

/// Derive the project key for `root`.
///
/// Reads the `origin` remote URL. If it looks like a GitHub remote
/// (SSH or HTTPS), the key is `github:org/repo`. Otherwise falls back
/// to `local:<dirname>`.
pub fn derive_project_key(root: &Path) -> String {
    if let Some(url) = crate::git::get_remote_url(root)
        && let Some(key) = parse_github_key(&url)
    {
        return key;
    }

    let name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    format!("local:{}", name)
}

/// Parse a GitHub remote URL into a `github:org/repo` key.
///
/// Handles both:
/// - HTTPS: `https://github.com/org/repo.git`
/// - SSH:   `git@github.com:org/repo.git`
fn parse_github_key(url: &str) -> Option<String> {
    let url = url.trim();

    // SSH: git@github.com:org/repo.git
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        let slug = rest.trim_end_matches(".git");
        if slug.contains('/') {
            return Some(format!("github:{}", slug));
        }
    }

    // HTTPS: https://github.com/org/repo.git
    if let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    {
        let slug = rest.trim_end_matches(".git");
        if slug.contains('/') {
            return Some(format!("github:{}", slug));
        }
    }

    None
}

/// In-memory representation of the project registry.
///
/// Keys are project identifiers (e.g. `github:org/repo` or `local:name`).
/// Values are lists of filesystem paths for that project.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectStore {
    #[serde(flatten)]
    entries: BTreeMap<String, Vec<PathBuf>>,
}

impl ProjectStore {
    /// Load the store from disk. Returns an empty store if the file does
    /// not exist.
    pub fn load() -> Result<Self> {
        let path = store_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        if data.trim().is_empty() {
            return Ok(Self::default());
        }
        let store = serde_json::from_str(&data)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        Ok(store)
    }

    /// Save the store to disk, creating parent directories as needed.
    pub fn save(&self) -> Result<()> {
        let path = store_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }
        let data = serde_json::to_string_pretty(&self.entries)?;
        std::fs::write(&path, data)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        Ok(())
    }

    /// Add `path` under `key`. Idempotent — does nothing if already present.
    pub fn add_path(&mut self, key: impl Into<String>, path: impl Into<PathBuf>) {
        let key = key.into();
        let path = path.into();
        let paths = self.entries.entry(key).or_default();
        if !paths.contains(&path) {
            paths.push(path);
        }
    }

    /// Remove `path` from under `key`. If it was the last path, removes the
    /// key entirely. Returns `true` if anything changed.
    pub fn remove_path(&mut self, key: &str, path: &Path) -> bool {
        let Some(paths) = self.entries.get_mut(key) else {
            return false;
        };
        let before = paths.len();
        paths.retain(|p| p != path);
        let changed = paths.len() != before;
        if paths.is_empty() {
            self.entries.remove(key);
        }
        changed
    }

    /// Remove all paths for `key`. Returns `true` if the key existed.
    pub fn remove_key(&mut self, key: &str) -> bool {
        self.entries.remove(key).is_some()
    }

    /// Return a reference to all entries.
    pub fn entries(&self) -> &BTreeMap<String, Vec<PathBuf>> {
        &self.entries
    }

    /// Return paths registered under `key`.
    pub fn paths_for(&self, key: &str) -> &[PathBuf] {
        self.entries
            .get(key)
            .map(|v| v.as_slice())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn data_dir_path(dir: &TempDir) -> &str {
        dir.path().to_str().unwrap()
    }

    // ── derive_project_key ──────────────────────────────────────────────────

    #[test]
    fn derive_key_github_https() {
        let key = parse_github_key("https://github.com/acme/myrepo.git").unwrap();
        assert_eq!(key, "github:acme/myrepo");
    }

    #[test]
    fn derive_key_github_https_no_git_suffix() {
        let key = parse_github_key("https://github.com/acme/myrepo").unwrap();
        assert_eq!(key, "github:acme/myrepo");
    }

    #[test]
    fn derive_key_github_ssh() {
        let key = parse_github_key("git@github.com:acme/myrepo.git").unwrap();
        assert_eq!(key, "github:acme/myrepo");
    }

    #[test]
    fn derive_key_non_github_returns_none() {
        assert!(parse_github_key("https://gitlab.com/acme/repo.git").is_none());
        assert!(parse_github_key("git@bitbucket.org:acme/repo.git").is_none());
    }

    #[test]
    fn derive_key_local_fallback() {
        let dir = TempDir::new().unwrap();
        // No git remote in a bare temp dir — expect local: key
        let key = derive_project_key(dir.path());
        let dir_name = dir.path().file_name().unwrap().to_str().unwrap();
        assert_eq!(key, format!("local:{}", dir_name));
    }

    // ── ProjectStore ────────────────────────────────────────────────────────

    #[test]
    fn store_empty_by_default() {
        let store = ProjectStore::default();
        assert!(store.entries().is_empty());
    }

    #[test]
    fn add_path_is_idempotent() {
        let mut store = ProjectStore::default();
        store.add_path("github:acme/repo", "/path/a");
        store.add_path("github:acme/repo", "/path/a");
        assert_eq!(store.paths_for("github:acme/repo").len(), 1);
    }

    #[test]
    fn add_multiple_paths_for_same_key() {
        let mut store = ProjectStore::default();
        store.add_path("github:acme/repo", "/path/a");
        store.add_path("github:acme/repo", "/path/b");
        assert_eq!(store.paths_for("github:acme/repo").len(), 2);
    }

    #[test]
    fn remove_path_shrinks_list() {
        let mut store = ProjectStore::default();
        store.add_path("github:acme/repo", "/path/a");
        store.add_path("github:acme/repo", "/path/b");
        let changed = store.remove_path("github:acme/repo", Path::new("/path/a"));
        assert!(changed);
        assert_eq!(store.paths_for("github:acme/repo").len(), 1);
    }

    #[test]
    fn remove_path_removes_key_when_last() {
        let mut store = ProjectStore::default();
        store.add_path("github:acme/repo", "/path/a");
        store.remove_path("github:acme/repo", Path::new("/path/a"));
        assert!(!store.entries().contains_key("github:acme/repo"));
    }

    #[test]
    fn remove_key_removes_all_paths() {
        let mut store = ProjectStore::default();
        store.add_path("github:acme/repo", "/path/a");
        store.add_path("github:acme/repo", "/path/b");
        let removed = store.remove_key("github:acme/repo");
        assert!(removed);
        assert!(!store.entries().contains_key("github:acme/repo"));
    }

    #[test]
    fn remove_key_nonexistent_returns_false() {
        let mut store = ProjectStore::default();
        assert!(!store.remove_key("github:nobody/nothing"));
    }

    #[test]
    fn store_round_trips_through_json() {
        let dir = TempDir::new().unwrap();
        temp_env::with_var("CUE_DATA_DIR", Some(data_dir_path(&dir)), || {
            let mut store = ProjectStore::default();
            store.add_path("github:acme/repo", "/path/a");
            store.add_path("github:acme/repo", "/path/b");
            store.add_path("local:myproject", "/home/user/myproject");
            store.save().unwrap();

            let loaded = ProjectStore::load().unwrap();
            assert_eq!(store, loaded);
        });
    }

    #[test]
    fn load_returns_empty_when_file_absent() {
        let dir = TempDir::new().unwrap();
        temp_env::with_var("CUE_DATA_DIR", Some(data_dir_path(&dir)), || {
            let store = ProjectStore::load().unwrap();
            assert!(store.entries().is_empty());
        });
    }

    #[test]
    fn load_returns_empty_when_file_is_empty() {
        let dir = TempDir::new().unwrap();
        temp_env::with_var("CUE_DATA_DIR", Some(data_dir_path(&dir)), || {
            let path = store_path();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, "").unwrap();
            let store = ProjectStore::load().unwrap();
            assert!(store.entries().is_empty());
        });
    }
}
