use crate::config::Config;
use anyhow::{Context, Result};
use cuelib::artifact::extract_frontmatter_yaml;
use cuelib::head;
use cuelib::store;
use serde::Deserialize;
use serde_json::json;
use std::path::Path;

/// Frontmatter fields read from the task card for display.
#[derive(Deserialize, Default)]
struct StatusFm {
    title: Option<String>,
    status: Option<String>,
}

pub fn handle(cwd: &Path, task: Option<String>, json: bool) -> Result<()> {
    let store_root = store::main_worktree_root(cwd).context("Not in a git repository")?;
    let config = Config::load(&store_root)?;
    let resolved = store::open(cwd, &config)?;

    let scope = head::resolve_scope(&resolved.head_dir, task.as_deref())?;

    match scope.slug.as_str() {
        "master" => {
            if json {
                let out = json!({
                    "context": "master",
                    "global": true,
                    "provenance": scope.provenance.as_str(),
                    "store": resolved.store_dir.display().to_string(),
                });
                println!("{}", out);
            } else {
                println!(
                    "active context: master (global) {}",
                    scope.provenance.label()
                );
                println!("  store: {}", resolved.store_dir.display());
            }
        }
        s => {
            // Attempt to read task card for title/status
            let task_card = resolved
                .store_dir
                .join("master")
                .join("task")
                .join(format!("{}.md", s));
            let fm = extract_frontmatter_yaml(&task_card)
                .and_then(|yaml| serde_yaml::from_str::<StatusFm>(&yaml).ok())
                .unwrap_or_default();
            let (title, status) = (fm.title, fm.status);

            if json {
                let out = json!({
                    "context": s,
                    "global": false,
                    "provenance": scope.provenance.as_str(),
                    "store": resolved.store_dir.display().to_string(),
                    "title": title,
                    "status": status,
                });
                println!("{}", out);
            } else {
                println!("active task: {} {}", s, scope.provenance.label());
                if let Some(t) = title {
                    println!("  title: {}", t);
                }
                if let Some(st) = status {
                    println!("  status: {}", st);
                }
                println!("  context: {}/{}/", config.dir_name, s);
                println!("  store: {}", resolved.store_dir.display());
            }
        }
    }

    Ok(())
}
