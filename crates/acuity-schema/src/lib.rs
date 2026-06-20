use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Placeholder type — used only to prove the ts-rs codegen pipeline
/// works end-to-end in Phase 0. Will be replaced by real event types
/// (SessionIdle, ToolCallRequested, ToolCallCompleted) in Phase 1.
///
/// The `export_to` attribute routes this type (and all future types in
/// this crate) into a single `types.ts` file rather than per-type files.
/// This is the intended distribution artifact for `cue-plugins`.
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export_to = "types.ts")]
pub struct Placeholder {
    pub name: String,
}
