use anyhow::{Result, bail};
use std::path::{Component, Path};

use crate::git;

/// Resolve the active context for the central store model.
///
/// Precedence is an explicit context, `$CUE_TASK`, then the current branch's
/// `branch.<name>.cue-task` Git configuration. Detached HEAD and absent values
/// leave the context unset.
pub fn resolve_active_context(root: &Path, explicit: Option<&str>) -> Result<Option<String>> {
    if let Some(context) = explicit {
        validate_slug(context)?;
        return Ok(Some(context.to_string()));
    }

    if let Ok(value) = std::env::var("CUE_TASK") {
        let context = value.trim();
        if !context.is_empty() {
            validate_slug(context)?;
            return Ok(Some(context.to_string()));
        }
    }

    let Some(branch) = git::current_branch(root) else {
        return Ok(None);
    };
    let Some(context) = git::get_branch_task(root, &branch) else {
        return Ok(None);
    };
    validate_slug(&context)?;
    Ok(Some(context))
}

/// Validate that a context slug is a single, safe path segment.
pub fn validate_slug(slug: &str) -> Result<()> {
    let mut comps = Path::new(slug).components();
    match (comps.next(), comps.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => bail!(
            "Invalid context slug '{}': must be a single path segment with no '..', '/', or absolute path",
            slug
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_slug_accepts_a_single_segment() {
        assert!(validate_slug("auth-login").is_ok());
        assert!(validate_slug("master").is_ok());
    }

    #[test]
    fn validate_slug_rejects_unsafe_paths() {
        for slug in ["", ".", "..", "../../foo", "/etc/x", "/", "a/b"] {
            assert!(validate_slug(slug).is_err(), "accepted unsafe slug: {slug}");
        }
    }
}
