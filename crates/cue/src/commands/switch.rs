use crate::config::Config;
use crate::git;
use anyhow::{bail, Context, Result};
use cuelib::head;
use cuelib::store;
use serde_json::json;
use std::fs;
use std::path::Path;

pub fn handle(cwd: &Path, target: Option<String>, json: bool) -> Result<()> {
    let root = git::get_git_root(cwd).context("Not in a git repository")?;
    let config = Config::load(&root)?;
    let cue_dir = root.join(&config.dir_name);
    let resolved = store::resolve_store(cue_dir)?;

    if !resolved.head_dir.exists() {
        bail!(
            "{} directory does not exist. Run `cue init` first.",
            config.dir_name
        );
    }

    let slug = match target {
        None => bail!("Provide a task slug or a task card path"),
        Some(t) => resolve_slug_from_target(&t),
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
