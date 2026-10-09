//! Plane 1: the trace artifact in the caller's context.
//!
//! The body is the agent's final message verbatim, with nothing added, so the
//! trace and the response extracted from `events.jsonl` are byte-identical and
//! the trace can be verified or regenerated. Writing goes through the `cue`
//! CLI rather than a reimplementation, because `cue add` owns frontmatter
//! encoding and the `repo_id` and `commit_hash` stamps.

use crate::receipt::RunReceipt;
use anyhow::{Result, bail};
use std::path::Path;
use std::process::Command;

/// Resolve the `cue` binary: `$CUE_AGENT_CUE_BIN`, then `cue` on `PATH`.
fn cue_binary() -> std::path::PathBuf {
    std::env::var_os("CUE_AGENT_CUE_BIN")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("cue"))
}

/// The artifact name:
/// `agent/<batch-label-or-"batch">-<batch-id>/<position>-<agent>.md`.
///
/// Nested so agent runs do not crowd hand-off traces. Every task of a batch
/// shares the directory in whichever context it captures to; the batch id
/// makes it unique to the batch, and the one-based position in the original
/// tasks array, zero-padded to three digits, keeps repeated agents apart and
/// reflects specification order rather than completion order.
pub fn artifact_name(
    batch_label: Option<&str>,
    batch_id: &str,
    position: usize,
    agent: &str,
) -> String {
    // Labels are for humans: one with nothing path-safe in it falls back to
    // the neutral prefix rather than to the agent placeholder.
    let prefix = batch_label
        .filter(|label| label.chars().any(|ch| ch.is_ascii_alphanumeric()))
        .map(crate::ids::sanitize)
        .unwrap_or_else(|| "batch".to_string());
    format!(
        "agent/{prefix}-{}/{position:03}-{}.md",
        crate::ids::sanitize(batch_id),
        crate::ids::sanitize(agent),
    )
}

/// The repository and revision a run actually executed, read from its
/// directory before anything there may be removed. `None` where the
/// directory has no repository, origin or commit.
#[derive(Debug, Clone, Default)]
pub struct Provenance {
    /// The `<org>/<repo>` scope derived from the origin, as cue derives it.
    pub repo_id: Option<String>,
    pub commit_hash: Option<String>,
}

/// Snapshot `dir`'s provenance with bounded Git commands that ignore any
/// inherited repository-routing variables, so only `dir` selects the
/// repository.
pub fn provenance(dir: &Path) -> Provenance {
    let repo_id = crate::worktree::inspect(dir, ["remote", "get-url", "origin"])
        .ok()
        .and_then(|origin| cuelib::store::parse_origin_scope(&origin))
        .and_then(|scope| scope.to_str().map(str::to_string));
    let commit_hash = crate::worktree::inspect(dir, ["rev-parse", "--short", "HEAD"]).ok();
    Provenance {
        repo_id,
        commit_hash,
    }
}

/// Frontmatter for one agent run.
///
/// The prompt is deliberately absent: prompts are long and multi-line, which
/// YAML handles badly and which would bury the artifact head. It is written
/// verbatim to plane 2 as `prompt.md` and reached through `run_path`. What
/// survives independently of plane 2 is `description`, the caller's label.
pub fn frontmatter(
    run: &RunReceipt,
    harness: &str,
    harness_version: Option<&str>,
    description: Option<&str>,
    batch_label: Option<&str>,
    source: &Provenance,
) -> Vec<(String, String)> {
    let mut fields = vec![
        ("kind".to_string(), "agent-run".to_string()),
        ("agent".to_string(), run.agent.clone()),
        ("harness".to_string(), harness.to_string()),
        ("outcome".to_string(), run.outcome.as_str().to_string()),
        ("duration_ms".to_string(), run.duration_ms.to_string()),
        ("turns".to_string(), run.turns.to_string()),
        ("tokens_input".to_string(), run.tokens_input.to_string()),
        ("tokens_output".to_string(), run.tokens_output.to_string()),
        ("cost_usd".to_string(), format!("{:.6}", run.cost_usd)),
        ("run_id".to_string(), run.run_id.clone()),
        ("batch_id".to_string(), run.batch_id.clone()),
        (
            "run_path".to_string(),
            run.run_path.to_string_lossy().into_owned(),
        ),
    ];
    if let Some(model) = &run.model {
        fields.push(("model".to_string(), model.clone()));
    }
    if let Some(version) = harness_version {
        fields.push(("harness_version".to_string(), version.to_string()));
    }
    if let Some(description) = description {
        fields.push(("description".to_string(), description.to_string()));
    }
    if let Some(label) = batch_label {
        fields.push(("batch_label".to_string(), label.to_string()));
    }
    if let Some(code) = run.exit_code {
        fields.push(("exit_code".to_string(), code.to_string()));
    }
    // Explicit stamps win over cue's own, which would describe the directory
    // cue was started in rather than the revision the run executed.
    if let Some(repo_id) = &source.repo_id {
        fields.push(("repo_id".to_string(), repo_id.clone()));
    }
    if let Some(commit) = &source.commit_hash {
        fields.push(("commit_hash".to_string(), commit.clone()));
    }
    fields
}

/// Write the trace by invoking `cue add`, returning the canonical address.
///
/// `context` is the canonical `<org>/<repo>/<context>` destination; `repo` is
/// the directory the run executed in. Its revision is stamped explicitly by
/// the caller where known, otherwise by cue from `repo`. Cross-scope writes
/// require both source revision fields explicitly; the destination address
/// selects the scope independently of the execution directory.
pub fn write(
    repo: &Path,
    context: &str,
    name: &str,
    body_path: &Path,
    fields: &[(String, String)],
) -> Result<String> {
    let mut command = Command::new(cue_binary());
    // Inherited repository-routing variables would make cue resolve the
    // scope of another repository than `repo`.
    for name in crate::worktree::REPOSITORY_ENV {
        command.env_remove(name);
    }
    command
        .arg("-C")
        .arg(repo)
        .arg("add")
        .arg(name)
        .arg("--file")
        .arg(body_path)
        .args(["--type", "trace", "--context", context]);
    for (key, value) in fields {
        command.arg("-f").arg(format!("{key}={value}"));
    }

    let output = command.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!("cue add failed: {stderr}");
    }

    Ok(format!("{context}/trace/{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_names_group_a_batch_and_number_tasks_by_position() {
        assert_eq!(
            artifact_name(
                Some("Review the diff"),
                "20260918-120301-3f9a2b",
                1,
                "consultant-opus"
            ),
            "agent/review-the-diff-20260918-120301-3f9a2b/001-consultant-opus.md"
        );
        assert_eq!(
            artifact_name(None, "20260918-120301-3f9a2b", 12, "explore"),
            "agent/batch-20260918-120301-3f9a2b/012-explore.md"
        );
        assert_eq!(
            artifact_name(Some("../.."), "b", 1000, "a/../b"),
            "agent/batch-b/1000-a-b.md"
        );
    }
}
