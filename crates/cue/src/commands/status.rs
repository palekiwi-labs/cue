use crate::config::Config;
use crate::git;
use anyhow::{Context, Result};
use cuelib::artifact::extract_frontmatter_yaml;
use cuelib::head;
use serde::Deserialize;
use serde_json::json;
use std::path::Path;

/// Frontmatter fields read from the task card for display.
#[derive(Deserialize, Default)]
struct StatusFm {
    title: Option<String>,
    status: Option<String>,
}

pub fn handle(cwd: &Path, json: bool) -> Result<()> {
    let root = git::get_git_root(cwd).context("Not in a git repository")?;
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
            let fm = extract_frontmatter_yaml(&task_card)
                .and_then(|yaml| serde_yaml::from_str::<StatusFm>(&yaml).ok())
                .unwrap_or_default();
            let (title, status) = (fm.title, fm.status);

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
