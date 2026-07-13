use anyhow::{bail, Result};
use std::fs;
use std::path::{Component, Path};

/// Read the active task slug from `<cue_dir>/HEAD`.
/// Returns `None` if the file is absent, unreadable, or empty.
pub fn read_head(cue_dir: &Path) -> Option<String> {
    let head_path = cue_dir.join("HEAD");
    let content = fs::read_to_string(&head_path).ok()?;
    let slug = content.trim().to_string();
    if slug.is_empty() {
        None
    } else {
        Some(slug)
    }
}

/// Write `slug` to `<cue_dir>/HEAD`.
pub fn write_head(cue_dir: &Path, slug: &str) -> Result<()> {
    let head_path = cue_dir.join("HEAD");
    fs::create_dir_all(cue_dir)?;
    fs::write(&head_path, slug)?;
    Ok(())
}

/// Resolve the active scope directory name.
///
/// Reads `<cue_dir>/HEAD`; returns `"master"` when the file is absent or empty.
/// The returned string is used as the subdirectory under `.cue/`.
pub fn resolve_scope(cue_dir: &Path) -> Result<String> {
    Ok(read_head(cue_dir).unwrap_or_else(|| "master".to_string()))
}

/// Validate that a task slug is a single, safe path segment.
///
/// Rejects traversal (`..`), separators (`/`, `\`), absolute paths, and the
/// current-dir marker (`.`). A valid slug is exactly one `Component::Normal`
/// with nothing else.
pub fn validate_slug(slug: &str) -> Result<()> {
    let mut comps = Path::new(slug).components();
    match (comps.next(), comps.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => bail!(
            "Invalid task slug '{}': must be a single path segment with no '..', '/', or absolute path",
            slug
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn resolve_scope_returns_master_when_head_absent() {
        let dir = tempdir().unwrap();
        let cue_dir = dir.path().join(".cue");
        fs::create_dir_all(&cue_dir).unwrap();
        assert_eq!(resolve_scope(&cue_dir).unwrap(), "master");
    }

    #[test]
    fn resolve_scope_returns_master_when_head_empty() {
        let dir = tempdir().unwrap();
        let cue_dir = dir.path().join(".cue");
        fs::create_dir_all(&cue_dir).unwrap();
        fs::write(cue_dir.join("HEAD"), "").unwrap();
        assert_eq!(resolve_scope(&cue_dir).unwrap(), "master");
    }

    #[test]
    fn resolve_scope_returns_slug_from_head() {
        let dir = tempdir().unwrap();
        let cue_dir = dir.path().join(".cue");
        fs::create_dir_all(&cue_dir).unwrap();
        fs::write(cue_dir.join("HEAD"), "auth-login\n").unwrap();
        assert_eq!(resolve_scope(&cue_dir).unwrap(), "auth-login");
    }

    #[test]
    fn resolve_scope_returns_master_when_head_contains_master() {
        let dir = tempdir().unwrap();
        let cue_dir = dir.path().join(".cue");
        fs::create_dir_all(&cue_dir).unwrap();
        fs::write(cue_dir.join("HEAD"), "master").unwrap();
        assert_eq!(resolve_scope(&cue_dir).unwrap(), "master");
    }

    #[test]
    fn write_and_read_head_roundtrip() {
        let dir = tempdir().unwrap();
        let cue_dir = dir.path().join(".cue");
        write_head(&cue_dir, "my-task").unwrap();
        assert_eq!(read_head(&cue_dir).unwrap(), "my-task");
    }

    #[test]
    fn validate_slug_accepts_simple_slug() {
        assert!(validate_slug("auth-login").is_ok());
    }

    #[test]
    fn validate_slug_accepts_master() {
        assert!(validate_slug("master").is_ok());
    }

    #[test]
    fn validate_slug_rejects_parent_dir() {
        assert!(validate_slug("..").is_err());
        assert!(validate_slug("../../foo").is_err());
    }

    #[test]
    fn validate_slug_rejects_absolute_path() {
        assert!(validate_slug("/etc/x").is_err());
        assert!(validate_slug("/").is_err());
    }

    #[test]
    fn validate_slug_rejects_multi_segment() {
        assert!(validate_slug("a/b").is_err());
    }

    #[test]
    fn validate_slug_rejects_current_dir() {
        assert!(validate_slug(".").is_err());
    }

    #[test]
    fn validate_slug_rejects_empty() {
        assert!(validate_slug("").is_err());
    }
}
