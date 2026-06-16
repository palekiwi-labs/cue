use crate::config::Config;
use crate::git;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn handle(cwd: &Path) -> Result<()> {
    // 1. Verify git repo
    git::run_git(["rev-parse", "--git-dir"], cwd).context("Not in a git repository")?;

    // 2. Get git root
    let root = git::get_git_root(cwd)?;

    // 3. Load config
    let config = Config::load(&root)?;

    // 4. Resolve cue path
    let cue_path = root.join(&config.dir_name);

    // 5. Check if already initialized
    if cue_path.exists() {
        println!(
            "{}/ directory already exists. Already initialized?",
            config.dir_name
        );
        return Ok(());
    }

    // 6. Check if worktree exists but dir is missing
    let worktrees = git::list_worktrees(&root)?;
    if worktrees.contains(&cue_path) && !cue_path.exists() {
        anyhow::bail!(
            "worktree for {} exists at {:?} but directory is missing",
            config.dir_name,
            cue_path
        );
    }

    // 7. Ensure worktree
    ensure_worktree(&root, &cue_path, &config)?;

    println!("✓ Initialized {}/ directory", config.dir_name);
    Ok(())
}

fn ensure_worktree(root: &Path, cue_path: &Path, config: &Config) -> Result<()> {
    let branch = &config.branch_name;

    if git::branch_is_checked_out(root, branch) {
        anyhow::bail!(
            "branch '{}' is already checked out in another worktree",
            branch
        );
    }

    if git::branch_exists_local(root, branch) {
        git::add_worktree(root, cue_path, branch)?;
    } else if git::branch_exists_on_remote(root, "origin", branch) {
        println!("Found {} branch on remote, fetching...", branch);
        git::fetch_branch(root, "origin", branch)?;
        git::add_worktree(root, cue_path, branch)?;
    } else {
        git::add_worktree_orphan(root, cue_path, branch)?;

        // Initialize orphan branch with config-driven ignore patterns
        let gitignore_content: String = config
            .ignored_types
            .iter()
            .map(|t| format!("*/{}/\n", t))
            .collect();
        fs::write(cue_path.join(".gitignore"), &gitignore_content)?;

        let rgignore_content: String = config
            .ignored_types
            .iter()
            .map(|t| format!("!*/{}/\n", t))
            .collect();
        fs::write(cue_path.join(".rgignore"), &rgignore_content)?;

        git::git_add(cue_path, &[".gitignore", ".rgignore"])?;
        git::git_commit(cue_path, &format!("Initialize {} branch", branch))?;
    }

    Ok(())
}
