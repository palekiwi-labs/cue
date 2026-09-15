pub use crate::list::ListOptions;

use crate::git;
use crate::list;
use anyhow::{Context, Result};
use std::path::Path;

pub fn handle(cwd: &Path, opts: ListOptions) -> Result<()> {
    let json_output = opts.json || opts.frontmatter;

    // 1. Verify git repo
    git::run_git(["rev-parse", "--git-dir"], cwd).context("Not in a git repository")?;

    // 2. Delegate to domain module
    let cue_files = list::list(cwd, opts)?;

    // 3. Output
    if !json_output {
        for cue_file in cue_files {
            println!("{}", cue_file.path);
        }
    } else {
        println!("{}", serde_json::to_string_pretty(&cue_files)?);
    }

    Ok(())
}
