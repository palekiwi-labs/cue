use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// The result of resolving a cue store directory.
///
/// When a `STORE` redirect file is present in `head_dir`, artifact I/O
/// is directed to `store_dir` (the redirect target). HEAD is always read
/// from and written to `head_dir`.
///
/// When no `STORE` file is present, `head_dir == store_dir`.
#[derive(Debug)]
pub struct ResolvedStore {
    /// Directory to read/write HEAD from. Always the local `.cue/`.
    pub head_dir: PathBuf,
    /// Directory to read/write artifacts from.
    /// Equals `head_dir` unless a `STORE` file redirects it.
    pub store_dir: PathBuf,
}

/// Resolve a cue store directory into a [`ResolvedStore`].
///
/// If `cue_dir/STORE` exists, its contents are read as an absolute path to
/// the real artifact store. The target is validated: it must exist and must
/// contain a `master/` subdirectory.
///
/// If `STORE` is absent, `head_dir` and `store_dir` are both set to `cue_dir`.
pub fn resolve_store(cue_dir: PathBuf) -> Result<ResolvedStore> {
    let store_file = cue_dir.join("STORE");

    if !store_file.exists() {
        return Ok(ResolvedStore {
            head_dir: cue_dir.clone(),
            store_dir: cue_dir,
        });
    }

    let raw = fs::read_to_string(&store_file)?;
    let target_path = PathBuf::from(raw.trim());

    validate_store_target(&target_path)?;

    let store_dir = target_path.canonicalize()?;

    Ok(ResolvedStore {
        head_dir: cue_dir,
        store_dir,
    })
}

/// Validate that a store target path is usable.
///
/// Checks:
/// - The path exists.
/// - The path contains a `master/` subdirectory.
fn validate_store_target(target: &Path) -> Result<()> {
    if !target.exists() {
        bail!("STORE target does not exist: {}", target.display());
    }

    if !target.join("master").is_dir() {
        bail!(
            "STORE target is not a valid cue store \
             (missing master/ subdirectory): {}",
            target.display()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    // Helper: create a minimal valid cue store (contains master/ subdir).
    fn make_store(path: &Path) {
        fs::create_dir_all(path.join("master")).unwrap();
    }

    #[test]
    fn no_store_file_returns_passthrough() {
        let dir = tempdir().unwrap();
        let cue_dir = dir.path().join(".cue");
        fs::create_dir_all(&cue_dir).unwrap();

        let resolved = resolve_store(cue_dir.clone()).unwrap();

        assert_eq!(resolved.head_dir, cue_dir);
        assert_eq!(resolved.store_dir, cue_dir);
    }

    #[test]
    fn store_file_redirects_store_dir() {
        let dir = tempdir().unwrap();
        let proxy_cue = dir.path().join("worktree").join(".cue");
        let real_store = dir.path().join("main").join(".cue");
        fs::create_dir_all(&proxy_cue).unwrap();
        make_store(&real_store);

        fs::write(proxy_cue.join("STORE"), real_store.to_str().unwrap()).unwrap();

        let resolved = resolve_store(proxy_cue.clone()).unwrap();

        assert_eq!(resolved.head_dir, proxy_cue);
        assert_eq!(resolved.store_dir, real_store.canonicalize().unwrap());
    }

    #[test]
    fn store_file_trims_whitespace_from_path() {
        let dir = tempdir().unwrap();
        let proxy_cue = dir.path().join("worktree").join(".cue");
        let real_store = dir.path().join("main").join(".cue");
        fs::create_dir_all(&proxy_cue).unwrap();
        make_store(&real_store);

        // Write path with surrounding whitespace and a trailing newline.
        let store_content = format!("  {}\n", real_store.to_str().unwrap());
        fs::write(proxy_cue.join("STORE"), &store_content).unwrap();

        let resolved = resolve_store(proxy_cue.clone()).unwrap();

        assert_eq!(resolved.store_dir, real_store.canonicalize().unwrap());
    }

    #[test]
    fn store_file_pointing_to_nonexistent_path_errors() {
        let dir = tempdir().unwrap();
        let proxy_cue = dir.path().join(".cue");
        fs::create_dir_all(&proxy_cue).unwrap();

        fs::write(
            proxy_cue.join("STORE"),
            "/nonexistent/path/that/does/not/exist",
        )
        .unwrap();

        let err = resolve_store(proxy_cue).unwrap_err();
        assert!(
            err.to_string().contains("does not exist"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn store_file_pointing_to_path_without_master_errors() {
        let dir = tempdir().unwrap();
        let proxy_cue = dir.path().join("worktree").join(".cue");
        let invalid_store = dir.path().join("invalid").join(".cue");
        fs::create_dir_all(&proxy_cue).unwrap();
        // Create the target dir but do NOT create master/ inside it.
        fs::create_dir_all(&invalid_store).unwrap();

        fs::write(proxy_cue.join("STORE"), invalid_store.to_str().unwrap()).unwrap();

        let err = resolve_store(proxy_cue).unwrap_err();
        assert!(
            err.to_string().contains("missing master/"),
            "unexpected error: {err}"
        );
    }
}
