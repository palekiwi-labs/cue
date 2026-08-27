use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path};

/// Provenance of the resolved active scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScopeProvenance {
    Flag,
    Env,
    Head,
    Default,
}

impl ScopeProvenance {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Flag => "flag",
            Self::Env => "env",
            Self::Head => "head",
            Self::Default => "default",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Flag => "(flag)",
            Self::Env => "(env)",
            Self::Head => "(head)",
            Self::Default => "(default)",
        }
    }
}

/// The result of resolving active scope precedence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedScope {
    pub slug: String,
    pub provenance: ScopeProvenance,
}

impl std::fmt::Display for ResolvedScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.slug)
    }
}

impl std::ops::Deref for ResolvedScope {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.slug
    }
}

impl AsRef<str> for ResolvedScope {
    fn as_ref(&self) -> &str {
        &self.slug
    }
}

impl AsRef<Path> for ResolvedScope {
    fn as_ref(&self) -> &Path {
        Path::new(&self.slug)
    }
}

/// Read the active task slug from `<cue_dir>/HEAD`.
/// Returns `None` if the file is absent, unreadable, or empty.
pub fn read_head(cue_dir: &Path) -> Option<String> {
    let head_path = cue_dir.join("HEAD");
    let content = fs::read_to_string(&head_path).ok()?;
    let slug = content.trim().to_string();
    if slug.is_empty() { None } else { Some(slug) }
}

/// Write `slug` to `<cue_dir>/HEAD`.
pub fn write_head(cue_dir: &Path, slug: &str) -> Result<()> {
    let head_path = cue_dir.join("HEAD");
    fs::create_dir_all(cue_dir)?;
    fs::write(&head_path, slug)?;
    Ok(())
}

/// Resolve the active scope following precedence:
/// 1. `--task <slug>` flag override (if provided, validated via [`validate_slug`])
/// 2. `$CUE_TASK` environment variable (if set and non-empty, validated via [`validate_slug`])
/// 3. `<cue_dir>/HEAD` file (if present and non-empty)
/// 4. `"master"` default
pub fn resolve_scope(cue_dir: &Path, flag: Option<&str>) -> Result<ResolvedScope> {
    if let Some(flag_slug) = flag {
        validate_slug(flag_slug)?;
        return Ok(ResolvedScope {
            slug: flag_slug.to_string(),
            provenance: ScopeProvenance::Flag,
        });
    }

    if let Ok(env_val) = std::env::var("CUE_TASK") {
        let trimmed = env_val.trim();
        if !trimmed.is_empty() {
            validate_slug(trimmed)?;
            return Ok(ResolvedScope {
                slug: trimmed.to_string(),
                provenance: ScopeProvenance::Env,
            });
        }
    }

    if let Some(head_slug) = read_head(cue_dir) {
        return Ok(ResolvedScope {
            slug: head_slug,
            provenance: ScopeProvenance::Head,
        });
    }

    Ok(ResolvedScope {
        slug: "master".to_string(),
        provenance: ScopeProvenance::Default,
    })
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
        temp_env::with_var_unset("CUE_TASK", || {
            let res = resolve_scope(&cue_dir, None).unwrap();
            assert_eq!(res.slug, "master");
            assert_eq!(res.provenance, ScopeProvenance::Default);
        });
    }

    #[test]
    fn resolve_scope_returns_master_when_head_empty() {
        let dir = tempdir().unwrap();
        let cue_dir = dir.path().join(".cue");
        fs::create_dir_all(&cue_dir).unwrap();
        fs::write(cue_dir.join("HEAD"), "").unwrap();
        temp_env::with_var_unset("CUE_TASK", || {
            let res = resolve_scope(&cue_dir, None).unwrap();
            assert_eq!(res.slug, "master");
            assert_eq!(res.provenance, ScopeProvenance::Default);
        });
    }

    #[test]
    fn resolve_scope_returns_slug_from_head() {
        let dir = tempdir().unwrap();
        let cue_dir = dir.path().join(".cue");
        fs::create_dir_all(&cue_dir).unwrap();
        fs::write(cue_dir.join("HEAD"), "auth-login\n").unwrap();
        temp_env::with_var_unset("CUE_TASK", || {
            let res = resolve_scope(&cue_dir, None).unwrap();
            assert_eq!(res.slug, "auth-login");
            assert_eq!(res.provenance, ScopeProvenance::Head);
        });
    }

    #[test]
    fn resolve_scope_returns_master_when_head_contains_master() {
        let dir = tempdir().unwrap();
        let cue_dir = dir.path().join(".cue");
        fs::create_dir_all(&cue_dir).unwrap();
        fs::write(cue_dir.join("HEAD"), "master").unwrap();
        temp_env::with_var_unset("CUE_TASK", || {
            let res = resolve_scope(&cue_dir, None).unwrap();
            assert_eq!(res.slug, "master");
            assert_eq!(res.provenance, ScopeProvenance::Head);
        });
    }

    #[test]
    fn resolve_scope_flag_wins_over_env_and_head() {
        let dir = tempdir().unwrap();
        let cue_dir = dir.path().join(".cue");
        fs::create_dir_all(&cue_dir).unwrap();
        fs::write(cue_dir.join("HEAD"), "head-task").unwrap();
        temp_env::with_var("CUE_TASK", Some("env-task"), || {
            let res = resolve_scope(&cue_dir, Some("flag-task")).unwrap();
            assert_eq!(res.slug, "flag-task");
            assert_eq!(res.provenance, ScopeProvenance::Flag);
        });
    }

    #[test]
    fn resolve_scope_env_wins_over_head() {
        let dir = tempdir().unwrap();
        let cue_dir = dir.path().join(".cue");
        fs::create_dir_all(&cue_dir).unwrap();
        fs::write(cue_dir.join("HEAD"), "head-task").unwrap();
        temp_env::with_var("CUE_TASK", Some("env-task"), || {
            let res = resolve_scope(&cue_dir, None).unwrap();
            assert_eq!(res.slug, "env-task");
            assert_eq!(res.provenance, ScopeProvenance::Env);
        });
    }

    #[test]
    fn resolve_scope_empty_env_falls_to_head() {
        let dir = tempdir().unwrap();
        let cue_dir = dir.path().join(".cue");
        fs::create_dir_all(&cue_dir).unwrap();
        fs::write(cue_dir.join("HEAD"), "head-task").unwrap();
        temp_env::with_var("CUE_TASK", Some("   "), || {
            let res = resolve_scope(&cue_dir, None).unwrap();
            assert_eq!(res.slug, "head-task");
            assert_eq!(res.provenance, ScopeProvenance::Head);
        });
    }

    #[test]
    fn resolve_scope_env_invalid_slug_errors() {
        let dir = tempdir().unwrap();
        let cue_dir = dir.path().join(".cue");
        fs::create_dir_all(&cue_dir).unwrap();
        temp_env::with_var("CUE_TASK", Some("odd/path..task"), || {
            let err = resolve_scope(&cue_dir, None).unwrap_err();
            assert!(err.to_string().contains("Invalid task slug"));
        });
    }

    #[test]
    fn resolve_scope_flag_invalid_slug_errors() {
        let dir = tempdir().unwrap();
        let cue_dir = dir.path().join(".cue");
        fs::create_dir_all(&cue_dir).unwrap();
        let err = resolve_scope(&cue_dir, Some("../bad")).unwrap_err();
        assert!(err.to_string().contains("Invalid task slug"));
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
