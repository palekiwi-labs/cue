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

    // 4. Delegate to domain module (idempotent — ok if already initialized)
    init::init(&root, &config)?;

    // 5. Register project in store (idempotent — add_path is a no-op if present)
    let key = cuelib::project::derive_project_key(&root);
    let mut store = cuelib::project::ProjectStore::load()?;
    store.add_path(key, &root);
    store.save()?;

    Ok(())
}
