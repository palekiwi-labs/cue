use crate::config::Config;
use crate::git;
use anyhow::{Context, Result};
use cuelib::head;
use serde_json::json;
use std::path::Path;

pub fn handle(cwd: &Path, json: bool) -> Result<()> {
    git::run_git(["rev-parse", "--git-dir"], cwd).context("Not in a git repository")?;
    let root = git::get_git_root(cwd)?;
    let config = Config::load(&root)?;
    let cue_dir = root.join(&config.dir_name);

    let slug = head::read_head(&cue_dir);

    match slug.as_deref() {
        None | Some("master") => {
            if json {
                let out = json!({
                    "context": "master",
                    "global": true,
                });
                println!("{}", out);
            } else {
                println!("active context: master (global)");
            }
        }
        Some(s) => {
            // Attempt to read task card for title/status
            let task_card = cue_dir
                .join("master")
                .join("task")
                .join(format!("{}.md", s));
            let (title, status) = if task_card.exists() {
                if let Ok(content) = std::fs::read_to_string(&task_card) {
                    (
                        extract_frontmatter_field(&content, "title"),
                        extract_frontmatter_field(&content, "status"),
                    )
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            if json {
                let out = json!({
                    "context": s,
                    "global": false,
                    "title": title,
                    "status": status,
                });
                println!("{}", out);
            } else {
                println!("active task: {}", s);
                if let Some(t) = title {
                    println!("  title: {}", t);
                }
                if let Some(st) = status {
                    println!("  status: {}", st);
                }
                println!("  context: .cue/{}/", s);
            }
        }
    }

    Ok(())
}

/// Extract a simple scalar frontmatter field value from raw markdown content.
/// Returns `None` if the field is not found in the frontmatter block.
fn extract_frontmatter_field(content: &str, field: &str) -> Option<String> {
    let inner = content.strip_prefix("---\n")?;
    let end = inner.find("\n---")?;
    let fm = &inner[..end];
    for line in fm.lines() {
        if let Some(rest) = line.strip_prefix(&format!("{}:", field)) {
            let val = rest.trim().trim_matches('"').trim_matches('\'').to_string();
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    None
}
