//! Syntactic validation of canonical cue addresses.
//!
//! A reference recorded inside an artifact (`parent`, `refs`, a log entry's
//! `trace`) is identity, not location: it is always a canonical address, and
//! its filesystem path is derived by joining it onto the store root. cue is
//! the sole writer of that metadata, so this is the only place the invariant
//! can be enforced without a separate linter.
//!
//! Validation here is purely syntactic and touches no filesystem. Whether an
//! address resolves to an existing artifact is a different question and is
//! deliberately not asked.
//!
//! Syntax alone cannot separate every partial form from a real address: a
//! context address `<org>/<repo>/<context>` and a three-segment nested
//! artifact tail such as `note/ideas/rollout.md` have the same shape. This
//! catches the forms an agent actually produces by accident (an absolute
//! path, a `~` path, a bare slug, `<type>/<name>`) and leaves the residual
//! ambiguity to resolution.

use anyhow::{Result, bail};

/// The canonical address forms, quoted back to the caller in every error so
/// the message names the shape that was expected.
const CANONICAL_FORM: &str = "expected a canonical address \
     '<org>/<repo>/<context>' or '<org>/<repo>/<context>/<type>/<name>'";

/// The shortest canonical address is a context: `<org>/<repo>/<context>`.
const MIN_SEGMENTS: usize = 3;

/// Validate that `value` has the shape of a canonical address.
///
/// `field` names the input being validated (for example `parent` or
/// `--trace`) and appears in the error message.
pub fn validate_reference(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("Invalid {field} '{value}': must not be empty; {CANONICAL_FORM}");
    }
    if value.starts_with('~') {
        bail!("Invalid {field} '{value}': home-relative paths are not addresses; {CANONICAL_FORM}");
    }
    if value.starts_with('/') || std::path::Path::new(value).is_absolute() {
        bail!("Invalid {field} '{value}': absolute paths are not addresses; {CANONICAL_FORM}");
    }

    let segments: Vec<&str> = value.split('/').collect();
    for segment in &segments {
        if segment.trim().is_empty() {
            bail!(
                "Invalid {field} '{value}': empty path segments are not allowed; {CANONICAL_FORM}"
            );
        }
        if *segment == "." || *segment == ".." {
            bail!(
                "Invalid {field} '{value}': relative path components are not allowed; \
                 {CANONICAL_FORM}"
            );
        }
    }
    if segments.len() < MIN_SEGMENTS {
        bail!("Invalid {field} '{value}': too short to be a canonical address; {CANONICAL_FORM}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_context_and_artifact_addresses() {
        for valid in [
            "acme/widgets/release",
            "acme/widgets/release/spec/index.md",
            "acme/widgets/release/note/ideas/canonical-addresses.md",
            "acme/widgets/release/trace/evidence.md",
            "palekiwi-labs/cue/data-model-spike/task/reference-shape-validation.md",
        ] {
            assert!(
                validate_reference("parent", valid).is_ok(),
                "'{valid}' should be a valid address"
            );
        }
    }

    #[test]
    fn rejects_absolute_paths() {
        for invalid in ["/home/pl/cue/acme/widgets/release", "/acme/widgets/release"] {
            let error = validate_reference("parent", invalid)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("absolute paths are not addresses"),
                "'{invalid}' should be rejected as absolute, got: {error}"
            );
        }
    }

    #[test]
    fn rejects_home_relative_paths() {
        let error = validate_reference("parent", "~/cue/acme/widgets/release")
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("home-relative paths are not addresses"),
            "got: {error}"
        );
    }

    #[test]
    fn rejects_partial_addresses() {
        for invalid in ["trace/evidence.md", "release", "plan/index.md"] {
            let error = validate_reference("parent", invalid)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("too short to be a canonical address"),
                "'{invalid}' should be rejected as too short, got: {error}"
            );
        }
    }

    #[test]
    fn rejects_empty_and_relative_segments() {
        for invalid in ["acme//widgets/release", "acme/ /release"] {
            let error = validate_reference("parent", invalid)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("empty path segments are not allowed"),
                "'{invalid}' should be rejected for an empty segment, got: {error}"
            );
        }
        for invalid in ["acme/../widgets/release", "./acme/widgets/release"] {
            let error = validate_reference("parent", invalid)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("relative path components are not allowed"),
                "'{invalid}' should be rejected for a relative component, got: {error}"
            );
        }
    }

    #[test]
    fn rejects_empty_input() {
        for invalid in ["", "   "] {
            let error = validate_reference("--trace", invalid)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("must not be empty"),
                "'{invalid}' should be rejected as empty, got: {error}"
            );
        }
    }

    #[test]
    fn every_error_names_the_canonical_form() {
        for invalid in ["", "~/cue/a/b/c", "/a/b/c", "a//b/c", "a/../b/c", "release"] {
            let error = validate_reference("parent", invalid)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("<org>/<repo>/<context>"),
                "'{invalid}' error should name the canonical form, got: {error}"
            );
        }
    }
}
