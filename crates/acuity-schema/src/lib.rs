use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const SCHEMA_VERSION: u8 = 1;

/// Represents an opencode session that has gone idle.
/// Emitted by the acuity opencode plugin and consumed by the acuity server.
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export_to = "types.ts")]
pub struct SessionIdle {
    pub session_id: String,
    pub project_dir: String,
    pub session_title: Option<String>,
}
