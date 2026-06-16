use crate::config::Config;
use crate::git;
use crate::init;
use anyhow::{Context, Result};
use std::path::Path;

pub fn handle(cwd: &Path) -> Result<()> {
    // 1. Verify git repo
    git::run_git(["rev-parse", "--git-dir"], cwd).context("Not in a git repository")?;

    // 2. Get git root
    let root = git::get_git_root(cwd)?;

    // 3. Load config
    let config = Config::load(&root)?;

    // 4. Delegate to domain module
    init::init(&root, &config)
}
