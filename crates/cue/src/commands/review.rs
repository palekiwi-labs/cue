use crate::cli::ReviewCommands;
use anyhow::{Result, bail};
use cue_review::model_slug;
use std::path::Path;

/// Dispatch a `cue review` stage.
///
/// All pipeline logic lives in the `cue-review` library crate; this module
/// only adapts CLI arguments to library calls. The stages are stubs until
/// their respective phases land.
pub fn handle(_cwd: &Path, command: ReviewCommands) -> Result<()> {
    match command {
        ReviewCommands::Init { .. } => not_implemented("init"),
        ReviewCommands::Submit { model, .. } => {
            // The slug names both the persisted trace and its candidate
            // ids, so an unusable model id is rejected before any work.
            model_slug(&model)?;
            not_implemented("submit")
        }
        ReviewCommands::Verdict { model, .. } => {
            model_slug(&model)?;
            not_implemented("verdict")
        }
        ReviewCommands::Finalize { .. } => not_implemented("finalize"),
        ReviewCommands::Schema { .. } => not_implemented("schema"),
    }
}

fn not_implemented(stage: &str) -> Result<()> {
    bail!("cue review {stage}: not implemented yet")
}
