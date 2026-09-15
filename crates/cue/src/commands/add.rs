pub use crate::add::AddOptions;

use crate::add;
use crate::git;
use anyhow::{Context, Result};
use std::path::Path;

pub fn handle(cwd: &Path, opts: AddOptions) -> Result<()> {
    // 1. Verify git repo
    git::run_git(["rev-parse", "--git-dir"], cwd).context("Not in a git repository")?;

    // 2. Delegate to domain module
    let file_path = add::add(cwd, opts)?;

    // 3. Print confirmation
    let rel_path = file_path.strip_prefix(cwd).unwrap_or(&file_path);
    eprintln!("✓ Created");
    println!("{}", rel_path.to_string_lossy());

    Ok(())
}
