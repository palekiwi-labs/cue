use anyhow::{Context as _, Result, bail};
use std::path::{Component, Path};

use crate::git;

/// The accepted forms of a context selector, quoted back in every error so the
/// message names the shape that was expected.
const SELECTOR_FORM: &str = "expected a context slug in the current repository scope \
     or a canonical '<org>/<repo>/<slug>' address";

/// Resolve the active context for the central store model.
///
/// Precedence is an explicit context, `$CUE_CONTEXT`, then the current branch's
/// `branch.<name>.cue-context` Git configuration. Detached HEAD and absent values
/// leave the context unset.
pub fn resolve_active_context(root: &Path, explicit: Option<&str>) -> Result<Option<String>> {
    if let Some(selector) = explicit {
        return Ok(Some(resolve_selector(root, selector)?));
    }

    if let Ok(value) = std::env::var("CUE_CONTEXT") {
        let selector = value.trim();
        if !selector.is_empty() {
            return Ok(Some(resolve_selector(root, selector)?));
        }
    }

    let Some(branch) = git::current_branch(root) else {
        return Ok(None);
    };
    let Some(selector) = git::get_branch_context(root, &branch) else {
        return Ok(None);
    };
    Ok(Some(resolve_selector(root, &selector)?))
}

/// Resolve a context selector to the slug it names.
///
/// A selector is either a bare slug or the canonical `<org>/<repo>/<slug>`
/// address `cue status` prints, so the one identity cue emits for a context can
/// be handed straight back to any surface that accepts a context.
///
/// The address form is a spelling of the current repository's context, not a
/// way to reach across scopes. Scope is derived from the working directory and
/// nothing else selects it: a write filed under another scope would still be
/// stamped from this repository. An address naming a different scope is
/// therefore an error rather than a cross-scope selection, and `-C <path>` is
/// the supported way to address another repository.
pub fn resolve_selector(root: &Path, selector: &str) -> Result<String> {
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
            Ok(slug.to_string())
        }
        [org, repo, slug] => {
            for segment in [org, repo, slug] {
                validate_segment(selector, segment)?;
            }
            let scope = crate::store::repository_scope(root)?;
            let scope = scope
                .to_str()
                .context("repository scope is not valid UTF-8")?;
            let addressed = format!("{org}/{repo}");
            if addressed != scope {
                bail!(
                    "Context '{selector}' names scope '{addressed}', but the current repository \
                     scope is '{scope}'; run cue from that repository or pass -C <path>"
                );
            }
            Ok(slug.to_string())
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
        let cwd = Path::new(".");
        for selector in [
            "",
            "widgets/release",
            "acme/widgets/release/spec/index.md",
            "~/cue/acme/widgets/release",
        ] {
            assert!(
                resolve_selector(cwd, selector).is_err(),
                "accepted non-context selector: {selector}"
            );
        }
    }
}
