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
    pub context: Option<String>,
    pub harness: String,
    pub harness_version: Option<String>,
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
