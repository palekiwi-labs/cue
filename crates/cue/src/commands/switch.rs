use crate::config::Config;
use crate::git;
use anyhow::{Context, Result, bail};
use cuelib::head;
use serde_json::json;
use std::fs;
use std::path::Path;

pub fn handle(
    cwd: &Path,
    target: Option<String>,
    branch: Option<String>,
    json: bool,
) -> Result<()> {
    // 1. Verify git repo and get root
    git::run_git(["rev-parse", "--git-dir"], cwd).context("Not in a git repository")?;
    let root = git::get_git_root(cwd)?;
    let config = Config::load(&root)?;
    let cue_dir = root.join(&config.dir_name);

    if !cue_dir.exists() {
        bail!(
            "{} directory does not exist. Run `cue init` first.",
            config.dir_name
        );
    }

    let slug = if let Some(b) = branch {
        find_task_for_branch(&cue_dir, &b, json)?
    } else {
        match target {
            None => bail!("Provide a task slug, a task card path, or use --branch <name>"),
            Some(t) => resolve_slug_from_target(&t),
        }
    };

    if slug.trim().is_empty() {
        bail!("Task slug cannot be empty.");
    }

    // Write HEAD
    head::write_head(&cue_dir, &slug)?;

    // Create context directory if needed (for non-master slugs)
    if slug != "master" {
        let task_dir = cue_dir.join(&slug);
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
    if target.ends_with(".md") {
        // Extract filename stem
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(target)
            .to_string()
    } else {
        target.to_string()
    }
}

/// Scan task cards in `master/task/*.md` for one whose `branch:` list
/// contains `branch_name`. Returns the slug if found, or master as fallback
/// when no match. When `json` is true, the human "no task matched" message
/// is suppressed to keep stdout a single JSON document.
fn find_task_for_branch(cue_dir: &Path, branch_name: &str, json: bool) -> Result<String> {
    let task_dir = cue_dir.join("master").join("task");
    if !task_dir.exists() {
        if !json {
            println!("no task matched branch: {}", branch_name);
        }
        // Return master as fallback (no-op)
        return Ok("master".to_string());
    }

    for entry in fs::read_dir(&task_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let slug = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if slug.is_empty() {
            continue;
        }

        // Read file and check if the branch name appears in a branch: list
        if let Ok(content) = fs::read_to_string(&path) {
            if branch_in_markdown(&content, branch_name) {
                return Ok(slug);
            }
        }
    }

    if !json {
        println!("no task matched branch: {}", branch_name);
    }
    // No match: return master as fallback
    Ok("master".to_string())
}

fn branch_in_markdown(content: &str, branch_name: &str) -> bool {
    // Simple frontmatter-only branch check. Handles three forms:
    //   branch: single-value
    //   branch: [a, b, c]
    //   branch:
    //     - a
    //     - b
    if let Some(fm) = extract_fm(content) {
        let mut in_branch_list = false;
        for line in fm.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("branch:") {
                let rest = rest.trim();
                if rest.is_empty() {
                    // Multiline list follows on subsequent lines.
                    in_branch_list = true;
                } else if rest.starts_with('[') && rest.ends_with(']') {
                    // Inline list: branch: [a, b, c]
                    let items = &rest[1..rest.len() - 1];
                    return items
                        .split(',')
                        .any(|i| i.trim().trim_matches('"').trim_matches('\'') == branch_name);
                } else {
                    // Single scalar: branch: value
                    return rest.trim_matches('"').trim_matches('\'') == branch_name;
                }
            } else if in_branch_list {
                if let Some(item) = trimmed.strip_prefix("- ") {
                    if item.trim_matches('"').trim_matches('\'') == branch_name {
                        return true;
                    }
                } else if !trimmed.is_empty() {
                    // Non-list line: branch block ended.
                    in_branch_list = false;
                }
            }
        }
    }
    false
}

fn extract_fm(content: &str) -> Option<&str> {
    let start_marker = "---\n";
    if !content.starts_with(start_marker) {
        return None;
    }
    let rest = &content[start_marker.len()..];
    let end_marker = "\n---";
    let end_idx = rest.find(end_marker)?;
    Some(&rest[..end_idx])
}
