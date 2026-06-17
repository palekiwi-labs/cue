use anyhow::{Context, anyhow};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn run_git<I, S>(args: I, cwd: &Path) -> anyhow::Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr> + std::fmt::Debug,
{
    let args_vec: Vec<_> = args.into_iter().collect();
    let output = Command::new("git")
        .args(&args_vec)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("Failed to execute git {:?}", args_vec))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(anyhow!("git {:?} failed: {}", args_vec, err))
    }
}

pub fn get_git_root(cwd: &Path) -> anyhow::Result<PathBuf> {
    let root = run_git(["rev-parse", "--show-toplevel"], cwd)?;
    Ok(PathBuf::from(root))
}

pub fn list_worktrees(cwd: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let output = run_git(["worktree", "list", "--porcelain"], cwd)?;
    let mut worktrees = Vec::new();
    for line in output.lines() {
        if let Some(path_str) = line.strip_prefix("worktree ") {
            worktrees.push(PathBuf::from(path_str.trim()));
        }
    }
    Ok(worktrees)
}

pub fn branch_exists_local(root: &Path, name: &str) -> bool {
    run_git(
        ["rev-parse", "--verify", &format!("refs/heads/{}", name)],
        root,
    )
    .is_ok()
}

pub fn branch_is_checked_out(root: &Path, name: &str) -> bool {
    match run_git(["worktree", "list", "--porcelain"], root) {
        Ok(output) => {
            let needle = format!("branch refs/heads/{}", name);
            output.lines().any(|l| l.trim() == needle)
        }
        Err(_) => false,
    }
}

pub fn branch_exists_on_remote(root: &Path, remote: &str, name: &str) -> bool {
    match run_git(
        [
            "ls-remote",
            "--heads",
            remote,
            &format!("refs/heads/{}", name),
        ],
        root,
    ) {
        Ok(output) => !output.trim().is_empty(),
        Err(_) => false,
    }
}

pub fn add_worktree(root: &Path, path: &Path, branch: &str) -> anyhow::Result<()> {
    run_git(
        [
            OsStr::new("worktree"),
            OsStr::new("add"),
            path.as_os_str(),
            OsStr::new(branch),
        ],
        root,
    )?;
    Ok(())
}

pub fn add_worktree_orphan(root: &Path, path: &Path, branch: &str) -> anyhow::Result<()> {
    run_git(
        vec![
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("--orphan"),
            OsStr::new("-b"),
            OsStr::new(branch),
            path.as_os_str(),
        ],
        root,
    )?;
    Ok(())
}

pub fn fetch_branch(root: &Path, remote: &str, branch: &str) -> anyhow::Result<()> {
    let refspec = format!("+refs/heads/{0}:refs/remotes/{1}/{0}", branch, remote);
    run_git(["fetch", remote, &refspec], root)?;
    Ok(())
}

pub fn git_add(cwd: &Path, files: &[&str]) -> anyhow::Result<()> {
    let mut args = vec!["add"];
    args.extend(files);
    run_git(args, cwd)?;
    Ok(())
}

pub fn git_commit(cwd: &Path, msg: &str) -> anyhow::Result<()> {
    run_git(["commit", "-m", msg], cwd)?;
    Ok(())
}

pub fn get_current_branch(cwd: &Path) -> anyhow::Result<String> {
    run_git(["rev-parse", "--abbrev-ref", "HEAD"], cwd)
}

pub fn get_short_head_hash(cwd: &Path) -> anyhow::Result<String> {
    run_git(["rev-parse", "--short", "HEAD"], cwd)
}

pub fn get_head_timestamp(cwd: &Path) -> anyhow::Result<u64> {
    let output = run_git(["log", "-1", "--format=%ct", "HEAD"], cwd)?;
    output
        .parse::<u64>()
        .with_context(|| format!("Failed to parse commit timestamp: '{}'", output))
}

pub fn is_working_tree_dirty(cwd: &Path) -> anyhow::Result<bool> {
    let output = run_git(["status", "--porcelain"], cwd)?;
    Ok(!output.trim().is_empty())
}

pub fn sanitize_branch_name(branch: &str) -> String {
    branch.replace(['/', '\\'], "-")
}
