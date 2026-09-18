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

/// The artifact name: `agent/<slug>-<agent>-<short-run-id>.md`.
///
/// Nested so agent runs do not crowd hand-off traces. The short run id makes
/// the name unique by construction, which a scan-then-suffix counter could not
/// do: concurrent runs would all see no `-2` and all try to write it.
pub fn artifact_name(slug: &str, agent: &str, short_run_id: &str) -> String {
    format!(
        "agent/{}-{}-{}.md",
        crate::ids::sanitize(slug),
        crate::ids::sanitize(agent),
        short_run_id
    )
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
    if let Some(code) = run.exit_code {
        fields.push(("exit_code".to_string(), code.to_string()));
    }
    fields
}

/// Write the trace by invoking `cue add`, returning the canonical address.
pub fn write(
    repo: &Path,
    context: &str,
    name: &str,
    body_path: &Path,
    fields: &[(String, String)],
) -> Result<String> {
    let mut command = Command::new(cue_binary());
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

    let scope = cuelib::store::repository_scope(repo)?;
    Ok(format!(
        "{}/{context}/trace/{name}",
        scope.to_string_lossy()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_artifact_name_is_unique_by_construction() {
        assert_eq!(
            artifact_name("Review the diff", "consultant-opus", "0a1b2c3d"),
            "agent/review-the-diff-consultant-opus-0a1b2c3d.md"
        );
    }
}
