use crate::config::Config;
use crate::git;
use anyhow::{bail, Context, Result};
use cuelib::head;
use cuelib::store;
use serde_json::json;
use std::fs;
use std::path::Path;

pub fn handle(cwd: &Path, target: Option<String>, json: bool) -> Result<()> {
    let store_root = store::git_root(cwd).context("Not in a git repository")?;
    let config = Config::load(&store_root)?;
    let resolved = store::open(cwd, &config)?;
    let root = git::get_git_root(cwd)?;

    let branch = git::current_branch(&root);

    let slug = match target.as_deref() {
        None => {
            let branch = branch
                .as_ref()
                .context("detached HEAD: no branch task association to restore")?;
            git::get_branch_task(&root, branch)
                .with_context(|| format!("no task associated with branch '{}'", branch))?
        }
        Some(t) => resolve_slug_from_target(t),
    };

    if slug.trim().is_empty() {
        bail!("Task slug cannot be empty.");
    }

    // Reject traversal / absolute paths / multi-segment slugs before any
    // filesystem write.
    head::validate_slug(&slug)?;

    // Write HEAD to the local head_dir (always local, never redirected)
    head::write_head(&resolved.head_dir, &slug)?;

    // Create context directory in the shared store
    if slug != "master" {
        let task_dir = resolved.store_dir.join(&slug);
        fs::create_dir_all(&task_dir).with_context(|| {
            format!("Failed to create context directory: {}", task_dir.display())
        })?;
    }

    // Best-effort association write so future checkouts can restore the
    // context; the HEAD switch above is the primary action. Restore mode
    // reads the association and never rewrites it.
    if let (Some(branch), Some(_)) = (&branch, &target) {
        let result = if slug == "master" {
            git::clear_branch_task(&root, branch)
        } else {
            git::set_branch_task(&root, branch, &slug)
        };
        if let Err(err) = result {
            eprintln!(
                "warning: failed to update task association for branch '{}': {}",
                branch, err
            );
        }
    }

    if let Ok(env_task) = std::env::var("CUE_TASK") {
        let trimmed = env_task.trim();
        if !trimmed.is_empty() {
            eprintln!(
                "warning: $CUE_TASK is set ('{}'); switch wrote local HEAD, but $CUE_TASK takes precedence",
                trimmed
            );
        }
    }

    if slug != "master" {
        if json {
            let out = json!({
                "context": slug,
                "global": false,
            });
            println!("{}", out);
        } else {
            println!("switched to task: {}", slug);
        }
    } else if json {
        let out = json!({
            "context": "master",
            "global": true,
        });
        println!("{}", out);
    } else {
        println!("switched to global context");
    }

    Ok(())
}

/// Derive slug from a target string (filepath stem or plain slug).
fn resolve_slug_from_target(target: &str) -> String {
    let path = std::path::Path::new(target);
    if path.extension().and_then(|e| e.to_str()) == Some("md") {
        // Extract filename stem (e.g. "auth-login.md" -> "auth-login")
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(target)
            .to_string()
    } else {
        target.to_string()
    }
}
