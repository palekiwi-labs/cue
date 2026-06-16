pub use crate::add::AddOptions;

use crate::add;
use crate::config::Config;
use crate::git;
use anyhow::{Context, Result};
use std::path::Path;

pub fn handle(cwd: &Path, opts: AddOptions) -> Result<()> {
    // 1. Verify git repo
    git::run_git(["rev-parse", "--git-dir"], cwd).context("Not in a git repository")?;

    // 2. Get git root
    let root = git::get_git_root(cwd)?;

    // 3. Load config
    let config = Config::load(&root)?;

    // 4. Delegate to domain module
    let file_path = add::add(&root, &config, opts)?;

    // 5. Print confirmation
    let rel_path = file_path.strip_prefix(&root).unwrap_or(&file_path);
    eprintln!("✓ Created");
    println!("{}", rel_path.to_string_lossy());

    Ok(())
}
