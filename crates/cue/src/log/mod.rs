use crate::config::Config;
use crate::git;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Default)]
pub struct LogEntry {
    pub title: String,
    pub body: Option<String>,
    #[serde(default)]
    pub found: Vec<String>,
    #[serde(default)]
    pub decided: Vec<String>,
    #[serde(default)]
    pub open: Vec<String>,
}

pub struct LogAddOptions {
    pub entry: LogEntry,
    pub scope_name: Option<String>,
}

pub fn add_entry(root: &Path, config: &Config, opts: LogAddOptions) -> Result<PathBuf> {
    let LogAddOptions { entry, scope_name } = opts;

    // 1. Validate
    if entry.title.trim().is_empty() {
        bail!("Title cannot be empty.");
    }
    if entry.title.chars().count() > 120 {
        bail!("Title must be 120 characters or fewer.");
    }

    // 2. Gather Git context
    let mut hash = git::get_short_head_hash(root).unwrap_or_else(|_| "initial".to_string());
    if git::is_working_tree_dirty(root).unwrap_or(false) {
        hash.push_str("-dirty");
    }

    // 3. Resolve path
    let cue_path = root.join(&config.dir_name);
    if !cue_path.exists() {
        bail!(
            "{} directory does not exist. Run `cue init` first.",
            config.dir_name
        );
    }

    let scope = if let Some(s) = scope_name {
        s
    } else {
        let cue_path = root.join(&config.dir_name);
        cuelib::head::resolve_scope(&cue_path)?
    };
    if scope.trim().is_empty() {
        bail!("Scope name cannot be empty.");
    }
    let scope_dir = git::sanitize_branch_name(&scope);

    let log_file_path = cue_path.join(&scope_dir).join("log.md");

    // 4. Open file and get metadata (to check if it's new) before building markdown
    if let Some(parent) = log_file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
        .with_context(|| format!("Failed to open {}", log_file_path.display()))?;

    let is_new = file.metadata()?.len() == 0;

    // 5. Build Markdown
    let md = build_log_markdown(&entry, &hash, is_new);

    // 6. Append to file
    file.write_all(md.as_bytes())
        .with_context(|| format!("Failed to write to {}", log_file_path.display()))?;

    Ok(log_file_path)
}

fn build_log_markdown(entry: &LogEntry, hash: &str, is_new: bool) -> String {
    let mut md = String::new();

    if is_new {
        md.push_str("# Project Log\n\n");
    }

    writeln!(&mut md, "## [{}] {}", hash, entry.title.trim()).unwrap();

    if let Some(b) = &entry.body {
        let b = b.trim();
        if !b.is_empty() {
            writeln!(&mut md, "\n{}", b).unwrap();
        }
    }

    let push_bullets = |label: &str, items: &[String], md: &mut String| {
        for item in items {
            let item = item.trim();
            if !item.is_empty() {
                writeln!(md, "- **{}:** {}", label, item).unwrap();
            }
        }
    };

    let has_bullets = entry
        .found
        .iter()
        .chain(entry.decided.iter())
        .chain(entry.open.iter())
        .any(|i| !i.trim().is_empty());

    if has_bullets {
        writeln!(&mut md).unwrap();
        push_bullets("Found", &entry.found, &mut md);
        push_bullets("Decided", &entry.decided, &mut md);
        push_bullets("Open", &entry.open, &mut md);
    }

    writeln!(&mut md).unwrap();

    md
}
