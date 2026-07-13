use crate::config::Config;
use crate::git;
use anyhow::{Context, Result, bail};
use cuelib::head;
use std::fs;
use std::path::Path;

pub fn handle(cwd: &Path, target: Option<String>, use_branch: bool) -> Result<()> {
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

    let slug = if use_branch {
        // Auto-select based on current git branch
        let current_branch = git::get_current_branch(cwd)?;
        find_task_for_branch(&cue_dir, &current_branch)?
    } else {
        match target {
            None => bail!("Provide a task slug, a task card path, or use --branch"),
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
        println!("switched to task: {}", slug);
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
/// contains `branch_name`. Returns the slug if found, or an error if no match.
fn find_task_for_branch(cue_dir: &Path, branch_name: &str) -> Result<String> {
    let task_dir = cue_dir.join("master").join("task");
    if !task_dir.exists() {
        println!("no task matched branch: {}", branch_name);
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

    println!("no task matched branch: {}", branch_name);
    // No match: return master as fallback
    Ok("master".to_string())
}

fn branch_in_markdown(content: &str, branch_name: &str) -> bool {
    // Very simple frontmatter-only branch check
    if let Some(fm) = extract_fm(content) {
        for line in fm.lines() {
            let line = line.trim();
            if line.starts_with("branch:") {
                let rest = line.strip_prefix("branch:").unwrap().trim();
                // Check for inline list [a, b] or single string
                if rest.starts_with('[') && rest.ends_with(']') {
                    let items = &rest[1..rest.len() - 1];
                    return items
                        .split(',')
                        .any(|i| i.trim().trim_matches('"').trim_matches('\'') == branch_name);
                } else {
                    return rest.trim_matches('"').trim_matches('\'') == branch_name;
                }
            }
            // Check for multiline list
            //   branch:
            //     - branch-name
            // This is harder without a real YAML parser, but let's try a simple heuristic
            if line == "- ".to_string() + branch_name
                || line == "- \"".to_string() + branch_name + "\""
            {
                return true;
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
