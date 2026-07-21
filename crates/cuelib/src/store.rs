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
/// If `cue_dir/STORE` exists, its contents are interpreted as the redirect
/// target for artifact I/O. The file must contain a single **absolute** path
/// (non-absolute and empty/whitespace-only contents are rejected with a loud
/// error). The target is then validated via [`validate_store_target`]: it must
/// exist, contain a `master/` subdirectory, and must not itself contain a
/// `STORE` file (chaining is not supported).
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
    let trimmed = raw.trim();

    if trimmed.is_empty() {
        bail!("STORE file is empty: {}", store_file.display());
    }

    let target_path = PathBuf::from(trimmed);

    if !target_path.is_absolute() {
        bail!(
            "STORE must contain an absolute path (got '{}') in {}",
            trimmed,
            store_file.display()
        );
    }

    validate_store_target(&target_path)?;

    let store_dir = target_path.canonicalize()?;

    Ok(ResolvedStore {
        head_dir: cue_dir,
        store_dir,
    })
}

/// Validate that a store target path is usable as a redirect target.
///
/// Checks:
/// - The path exists.
/// - The path contains a `master/` subdirectory.
/// - The path does not itself contain a `STORE` file (chaining is not
///   supported and will error loudly).
///
/// The caller is responsible for ensuring `target` is an absolute path before
/// calling this function.
pub fn validate_store_target(target: &Path) -> Result<()> {
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

    if target.join("STORE").exists() {
        bail!(
            "STORE target is itself a proxy (chaining not supported): {}",
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
    fn empty_store_file_errors() {
        let dir = tempdir().unwrap();
        let proxy_cue = dir.path().join(".cue");
        fs::create_dir_all(&proxy_cue).unwrap();
        fs::write(proxy_cue.join("STORE"), "").unwrap();

        let err = resolve_store(proxy_cue).unwrap_err();
        assert!(err.to_string().contains("empty"), "unexpected error: {err}");
    }

    #[test]
    fn whitespace_only_store_file_errors() {
        let dir = tempdir().unwrap();
        let proxy_cue = dir.path().join(".cue");
        fs::create_dir_all(&proxy_cue).unwrap();
        fs::write(proxy_cue.join("STORE"), "   \n  ").unwrap();

        let err = resolve_store(proxy_cue).unwrap_err();
        assert!(err.to_string().contains("empty"), "unexpected error: {err}");
    }

    #[test]
    fn relative_path_in_store_file_errors() {
        let dir = tempdir().unwrap();
        let proxy_cue = dir.path().join(".cue");
        fs::create_dir_all(&proxy_cue).unwrap();
        fs::write(proxy_cue.join("STORE"), "relative/path/.cue").unwrap();

        let err = resolve_store(proxy_cue).unwrap_err();
        assert!(
            err.to_string().contains("absolute"),
            "unexpected error: {err}"
        );
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
    fn chained_store_target_errors() {
        let dir = tempdir().unwrap();
        let proxy_cue = dir.path().join(".cue");
        let chained_store = dir.path().join("chained").join(".cue");
        fs::create_dir_all(&proxy_cue).unwrap();
        // The chained store is a valid store (has master/) but is itself
        // a proxy (contains a STORE file) — chaining must be rejected.
        make_store(&chained_store);
        fs::write(chained_store.join("STORE"), "/some/other/store").unwrap();

        fs::write(proxy_cue.join("STORE"), chained_store.to_str().unwrap()).unwrap();

        let err = resolve_store(proxy_cue).unwrap_err();
        assert!(
            err.to_string().contains("chaining"),
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
