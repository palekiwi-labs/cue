use crate::config::Config;
use crate::git;
use anyhow::{Context, Result, bail};
use cuelib::artifact::extract_frontmatter_yaml;
use cuelib::head;
use serde::Deserialize;
use serde_json::json;
use std::fs;
use std::path::Path;

/// Frontmatter `branch:` field, which may be a scalar, inline list, or
/// block list. serde_yaml's untagged enum handles all three YAML forms.
#[derive(Deserialize, Default)]
#[serde(untagged)]
enum BranchField {
    One(String),
    Many(Vec<String>),
    #[default]
    None,
}

impl BranchField {
    fn contains(&self, name: &str) -> bool {
        match self {
            BranchField::One(s) => s == name,
            BranchField::Many(v) => v.iter().any(|s| s == name),
            BranchField::None => false,
        }
    }
}

#[derive(Deserialize, Default)]
struct TaskFm {
    #[serde(default)]
    branch: BranchField,
}

pub fn handle(
    cwd: &Path,
    target: Option<String>,
    branch: Option<String>,
    json: bool,
) -> Result<()> {
    let root = git::get_git_root(cwd).context("Not in a git repository")?;
    let config = Config::load(&root)?;
    let cue_dir = root.join(&config.dir_name);

    if !cue_dir.exists() {
        bail!(
            "{} directory does not exist. Run `cue init` first.",
            config.dir_name
        );
    }

    let slug = if let Some(b) = branch {
        match find_task_for_branch(&cue_dir, &b)? {
            Some(s) => s,
            None => bail!("no task matched branch: {}. HEAD unchanged.", b),
        }
    } else {
        match target {
            None => bail!("Provide a task slug, a task card path, or use --branch <name>"),
            Some(t) => resolve_slug_from_target(&t),
        }
    };

    if slug.trim().is_empty() {
        bail!("Task slug cannot be empty.");
    }

    // Reject traversal / absolute paths / multi-segment slugs before any
    // filesystem write.
    head::validate_slug(&slug)?;

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

/// Scan task cards in `master/task/*.md` for one whose `branch:` field
/// contains `branch_name`. Returns `Ok(Some(slug))` on match, `Ok(None)`
/// when no card matches. The caller decides how to handle the no-match case.
fn find_task_for_branch(cue_dir: &Path, branch_name: &str) -> Result<Option<String>> {
    let task_dir = cue_dir.join("master").join("task");
    if !task_dir.exists() {
        return Ok(None);
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

        if let Some(yaml) = extract_frontmatter_yaml(&path) {
            if let Ok(fm) = serde_yaml::from_str::<TaskFm>(&yaml) {
                if fm.branch.contains(branch_name) {
                    return Ok(Some(slug));
                }
            }
        }
    }

    Ok(None)
}
