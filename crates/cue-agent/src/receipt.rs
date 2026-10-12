//! The single JSON receipt cue-agent prints on exit.
//!
//! There is no streaming protocol in this phase: the caller receives one object
//! describing the whole batch. Both later routes stay open (an extension
//! tailing the run files, or a merged tagged event stream) because the files
//! exist from day one as the capture mechanism.

use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct BatchReceipt {
    pub batch_id: String,
    pub batch_path: PathBuf,
    pub cap: usize,
    pub harness: String,
    /// The one version every launched run reported; `None` when unknown or
    /// when tasks resolved different executables.
    pub harness_version: Option<String>,
    /// One run per task, in specification order.
    pub runs: Vec<RunReceipt>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunReceipt {
    pub run_id: String,
    pub batch_id: String,
    pub agent: String,
    pub model: Option<String>,
    pub outcome: Outcome,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub duration_ms: u128,
    pub turns: u64,
    pub tokens_input: u64,
    pub tokens_output: u64,
    pub cost_usd: f64,
    pub response: String,
    pub error: Option<String>,
    pub stderr_excerpt: Option<String>,
    pub events_malformed: u64,
    pub events_oversized: u64,
    pub events_truncated: bool,
    pub run_path: PathBuf,
    pub trace: Option<String>,
    pub trace_error: Option<String>,
    pub persistence_errors: Vec<String>,
    /// Worktree resources that could not be removed, each naming what
    /// survives and where. Independent of the execution outcome.
    pub cleanup_errors: Vec<String>,
    /// Generated identifiers of a retained worktree, present only when the
    /// caller needs them to find the work.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<RetainedWorktree>,
}

/// Where retained work lives, limited to identifiers the caller did not
/// choose itself.
#[derive(Debug, Clone, Serialize)]
pub struct RetainedWorktree {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Completed,
    Failed,
    Timeout,
    Aborted,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Timeout => "timeout",
            Self::Aborted => "aborted",
        }
    }
}

/// A short human-readable account of a batch: one header per run, in
/// specification order, followed by its response or by everything that went
/// wrong. Failures are never reduced to the outcome word alone.
pub fn human(batch: &BatchReceipt) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let _ = writeln!(out, "batch {}", batch.batch_id);
    for (index, run) in batch.runs.iter().enumerate() {
        let mut status = run.outcome.as_str().to_string();
        if let Some(code) = run.exit_code.filter(|code| *code != 0) {
            let _ = write!(status, ", exit {code}");
        }
        if let Some(signal) = run.signal {
            let _ = write!(status, ", signal {signal}");
        }
        let _ = writeln!(
            out,
            "\n[{}] {} ({status}, {} ms)",
            index + 1,
            run.agent,
            run.duration_ms
        );
        if let Some(error) = &run.error {
            let _ = writeln!(out, "error: {error}");
        }
        if let Some(stderr) = &run.stderr_excerpt {
            let _ = writeln!(out, "stderr:\n{}", stderr.trim_end());
        }
        if let Some(trace) = &run.trace {
            let _ = writeln!(out, "trace: {trace}");
        }
        if let Some(error) = &run.trace_error {
            let _ = writeln!(out, "trace error: {error}");
        }
        for error in &run.persistence_errors {
            let _ = writeln!(out, "persistence error: {error}");
        }
        for error in &run.cleanup_errors {
            let _ = writeln!(out, "cleanup error: {error}");
        }
        if let Some(worktree) = &run.worktree {
            if let Some(path) = &worktree.path {
                let _ = writeln!(out, "retained worktree: {}", path.display());
            }
            if let Some(branch) = &worktree.branch {
                let _ = writeln!(out, "retained branch: {branch}");
            }
        }
        let _ = writeln!(out, "record: {}", run.run_path.display());
        if !run.response.is_empty() {
            let _ = writeln!(out, "\n{}", run.response.trim_end());
        }
    }
    out
}
