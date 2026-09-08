use crate::config::Config;
use crate::git;
use crate::init;
use anyhow::{Context, Result};
use cuelib::store;
use std::path::Path;

pub fn handle(cwd: &Path) -> Result<()> {
    // 1. Verify git repo
    git::run_git(["rev-parse", "--git-dir"], cwd).context("Not in a git repository")?;

    // 2. Get store root (main git root)
    let store_root = store::main_worktree_root(cwd).context("Not in a git repository")?;

    // 3. Load config from store root
    let config = Config::load(&store_root)?;

    // 4. Get local git root
    let local_root = git::current_worktree_root(cwd)?;

    if local_root != store_root {
        let resolved = store::open(cwd, &config)?;
        println!("Store already exists at {}", resolved.store_dir.display());
        return Ok(());
    }

    // 5. Delegate to domain module (idempotent — ok if already initialized)
    init::init(&local_root, &config)?;

    // 6. Register project in store (idempotent — add_path is a no-op if present)
    let key = cuelib::project::derive_project_key(&local_root);
    let mut store = cuelib::project::ProjectStore::load()?;
    store.add_path(key, &local_root);
    store.save()?;

    Ok(())
}
