//! Domain model and pipeline logic for agent-driven code reviews.
//!
//! This crate is library-only. The entire CLI surface lives in `crates/cue`
//! as `cue review <sub>`, following the wire contract in
//! `.cue/cue-review-mvp/doc/schema-contract.md`.

use anyhow::{Result, bail};

/// Schema version stamped onto every artifact the pipeline persists.
pub const SCHEMA_VERSION: &str = "1.0.0";

/// Derive the filename slug for a model id.
///
/// The slug is the final path segment of the model id, lowercased and
/// reduced to characters that are safe in a filename. It names both the
/// persisted trace (`reviewer-<slug>.json`) and the candidate ids within
/// it (`<slug>-<n>`), so a run's ids are unique by construction.
///
/// ```
/// # use cue_review::model_slug;
/// assert_eq!(model_slug("anthropic/claude-opus-5").unwrap(), "claude-opus-5");
/// ```
pub fn model_slug(model: &str) -> Result<String> {
    let tail = model.rsplit('/').next().unwrap_or_default();

    let mut slug = String::with_capacity(tail.len());
    for ch in tail.chars() {
        let mapped = match ch.to_ascii_lowercase() {
            c @ ('a'..='z' | '0'..='9' | '.' | '_') => c,
            _ => '-',
        };
        if mapped == '-' && slug.ends_with('-') {
            continue;
        }
        slug.push(mapped);
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        bail!("model id yields an empty slug: {model:?}");
    }
    Ok(slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_the_provider_prefix() {
        assert_eq!(
            model_slug("anthropic/claude-opus-5").unwrap(),
            "claude-opus-5"
        );
        assert_eq!(
            model_slug("google/gemini-3.7-flash").unwrap(),
            "gemini-3.7-flash"
        );
    }

    #[test]
    fn keeps_a_bare_model_id_intact() {
        assert_eq!(model_slug("claude-opus-5").unwrap(), "claude-opus-5");
    }

    #[test]
    fn uses_only_the_final_path_segment() {
        assert_eq!(
            model_slug("openrouter/anthropic/claude-opus-5").unwrap(),
            "claude-opus-5"
        );
    }

    #[test]
    fn lowercases_and_replaces_unsafe_characters() {
        assert_eq!(
            model_slug("Anthropic/Claude Opus:5").unwrap(),
            "claude-opus-5"
        );
    }

    #[test]
    fn collapses_and_trims_separator_runs() {
        assert_eq!(model_slug("x/  opus  ").unwrap(), "opus");
    }

    #[test]
    fn rejects_a_model_id_with_no_usable_characters() {
        assert!(model_slug("").is_err());
        assert!(model_slug("anthropic/").is_err());
        assert!(model_slug("///").is_err());
    }
}
