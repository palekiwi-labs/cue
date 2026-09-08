use crate::git;
use anyhow::{Context, Result};
use cuelib::store;
use std::path::Path;

pub fn handle(cwd: &Path, store_root: Option<&Path>) -> Result<()> {
    // 1. Verify git repo
    git::run_git(["rev-parse", "--git-dir"], cwd).context("Not in a git repository")?;

    let scope = store::repository_scope(cwd)?;
    let store_dir = store::root(store_root)?.join(scope);
    std::fs::create_dir_all(&store_dir).with_context(|| {
        format!(
            "Failed to create central cue store at {}",
            store_dir.display()
        )
    })?;

    println!("Initialized cue store at {}", store_dir.display());

    Ok(())
}
