use anyhow::{bail, Result};
use std::path::{Component, Path, PathBuf};

use crate::git;

/// The accepted forms of a context selector, quoted back in every error so the
/// message names the shape that was expected.
const SELECTOR_FORM: &str = "expected a context slug in the current repository scope \
     or a canonical '<org>/<repo>/<slug>' address";

/// A context selection resolved against the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedContext {
    /// The context slug.
    pub slug: String,
    /// The `<org>/<repo>` scope the selector addressed explicitly. `None`
    /// means the working directory's repository scope selects it.
    pub scope: Option<String>,
}

impl ResolvedContext {
    /// The canonical address of the selected context: the addressed scope
    /// when the selector named one, the cwd repository scope otherwise.
    pub fn address(&self, root: &Path) -> Result<String> {
        match &self.scope {
            Some(scope) => Ok(format!("{scope}/{}", self.slug)),
            None => Ok(format!(
                "{}/{}",
                crate::store::repository_scope(root)?.to_string_lossy(),
                self.slug
            )),
        }
    }

    /// The scope directory of this selection inside the store.
    pub fn scope_dir(&self, root: &Path, store_root: Option<&Path>) -> Result<PathBuf> {
        let scope = match &self.scope {
            Some(scope) => PathBuf::from(scope),
            None => crate::store::repository_scope(root)?,
        };
        Ok(crate::store::root(store_root)?.join(scope))
    }

    /// The context directory of this selection inside the store.
    pub fn context_dir(&self, root: &Path, store_root: Option<&Path>) -> Result<PathBuf> {
        Ok(self.scope_dir(root, store_root)?.join(&self.slug))
    }
}

/// Resolve the active context for the central store model.
///
/// Precedence is an explicit context, `$CUE_CONTEXT`, then the current branch's
/// `branch.<name>.cue-context` Git configuration. Detached HEAD and absent values
/// leave the context unset.
pub fn resolve_active_context(
    root: &Path,
    explicit: Option<&str>,
) -> Result<Option<ResolvedContext>> {
    if let Some(selector) = explicit {
        return Ok(Some(resolve_selector(selector)?));
    }

    if let Ok(value) = std::env::var("CUE_CONTEXT") {
        let selector = value.trim();
        if !selector.is_empty() {
            return Ok(Some(resolve_selector(selector)?));
        }
    }

    let Some(branch) = git::current_branch(root) else {
        return Ok(None);
    };
    let Some(selector) = git::get_branch_context(root, &branch) else {
        return Ok(None);
    };
    Ok(Some(resolve_selector(&selector)?))
}

/// Resolve a context selector to the context it names.
///
/// A selector is either a bare slug or the canonical `<org>/<repo>/<slug>`
/// address `cue status` prints, so the one identity cue emits for a context can
/// be handed straight back to any surface that accepts a context.
///
/// The address form is authoritative for destination resolution: it names the
/// scope the command operates on, matching the cwd scope or not. A canonical
/// address therefore reaches any scope of the selected store without a
/// checkout of that repository, for reads and writes alike. A bare slug keeps
/// the working directory's scope, so nothing changes for anyone who does not
/// type an address.
pub fn resolve_selector(selector: &str) -> Result<ResolvedContext> {
    // Checked on the whole value rather than per segment: `~` is a shell and
    // path convention, and a value leading with it is a path being passed
    // where a context belongs.
    if selector.starts_with('~') {
        bail!(
            "Invalid context '{selector}': home-relative paths are not addresses; {SELECTOR_FORM}"
        );
    }

    let segments: Vec<&str> = selector.split('/').collect();
    match segments.as_slice() {
        [slug] => {
            validate_segment(selector, slug)?;
            Ok(ResolvedContext {
                slug: slug.to_string(),
                scope: None,
            })
        }
        [org, repo, slug] => {
            for segment in [org, repo, slug] {
                validate_segment(selector, segment)?;
            }
            Ok(ResolvedContext {
                slug: slug.to_string(),
                scope: Some(format!("{org}/{repo}")),
            })
        }
        _ => bail!("Invalid context '{selector}': {SELECTOR_FORM}"),
    }
}

/// Validate one segment of a selector, naming the whole value in the error so
/// the operator sees what they typed rather than the fragment that failed.
fn validate_segment(selector: &str, segment: &str) -> Result<()> {
    validate_slug(segment)
        .map_err(|_| anyhow::anyhow!("Invalid context '{selector}': {SELECTOR_FORM}"))
}

/// Validate that a context slug is a single, safe path segment.
///
/// Path semantics alone are too permissive: a line break would split one
/// address across two lines wherever cue prints it back, and a whitespace-only
/// slug would print as a gap that names nothing, so both are rejected on top of
/// the path rules. An ordinary internal space is left alone: it is a character
/// of the slug.
pub fn validate_slug(slug: &str) -> Result<()> {
    let invalid = || {
        anyhow::anyhow!(
            "Invalid context slug '{}': must be a single path segment with no '..', '/', or absolute path",
            slug
        )
    };
    if slug.contains(['\n', '\r']) || slug.trim().is_empty() {
        return Err(invalid());
    }
    let mut comps = Path::new(slug).components();
    match (comps.next(), comps.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => Err(invalid()),
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

    #[test]
    fn validate_slug_rejects_unprintable_segments() {
        for slug in [" ", "\t", "auth\nlogin", "auth\rlogin"] {
            assert!(
                validate_slug(slug).is_err(),
                "accepted unprintable slug: {slug:?}"
            );
        }
        assert!(
            validate_slug("auth login").is_ok(),
            "an internal space is a character of the slug"
        );
    }

    #[test]
    fn resolve_selector_rejects_shapes_that_are_not_a_context() {
        for selector in [
            "",
            "widgets/release",
            "acme/widgets/release/spec/index.md",
            "~/cue/acme/widgets/release",
        ] {
            assert!(
                resolve_selector(selector).is_err(),
                "accepted non-context selector: {selector}"
            );
        }
    }
}
