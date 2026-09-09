use anyhow::{Context, anyhow};
use std::ffi::OsStr;
use std::path::Path;
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
        .with_context(|| format!("Failed to execute git {args_vec:?}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(anyhow!("git {args_vec:?} failed: {error}"))
    }
}

/// `None` when HEAD is detached; an unborn branch still yields its name.
pub fn current_branch(cwd: &Path) -> Option<String> {
    run_git(["symbolic-ref", "--quiet", "--short", "HEAD"], cwd).ok()
}

/// Dotted branch names are safe: the config subsection is opaque up to
/// the last dot.
pub fn branch_task_key(branch: &str) -> String {
    format!("branch.{branch}.cue-task")
}

pub fn get_branch_task(root: &Path, branch: &str) -> Option<String> {
    let value = run_git(
        ["config", "--local", "--get", &branch_task_key(branch)],
        root,
    )
    .ok()?;
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

pub fn get_short_head_hash(cwd: &Path) -> anyhow::Result<String> {
    run_git(["rev-parse", "--short", "HEAD"], cwd)
}

pub fn is_working_tree_dirty(cwd: &Path) -> anyhow::Result<bool> {
    let output = run_git(["status", "--porcelain"], cwd)?;
    Ok(!output.trim().is_empty())
}
