use crate::config::Config;
use crate::git;
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

#[derive(Deserialize, Default)]
struct LogEntry {
    title: String,
    body: Option<String>,
    #[serde(default)]
    found: Vec<String>,
    #[serde(default)]
    decided: Vec<String>,
    #[serde(default)]
    open: Vec<String>,
}

pub fn handle(
    cwd: &Path,
    title: Option<String>,
    body: Option<String>,
    found: Vec<String>,
    decided: Vec<String>,
    open: Vec<String>,
    file: Option<String>,
) -> Result<()> {
    // 1. Parse or collect entry
    let entry = if let Some(path) = file {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read JSON file: {}", path))?;
        let entry: LogEntry = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON file: {}", path))?;
        entry
    } else {
        let title = title.context("The --title argument is required when not using --file")?;
        LogEntry {
            title,
            body,
            found,
            decided,
            open,
        }
    };

    // 2. Validate
    if entry.title.trim().is_empty() {
        bail!("Title cannot be empty.");
    }
    if entry.title.chars().count() > 120 {
        bail!("Title must be 120 characters or fewer.");
    }

    // 3. Gather Git context
    git::run_git(["rev-parse", "--git-dir"], cwd).context("Not in a git repository")?;
    let root = git::get_git_root(cwd)?;

    // We need the hash of the project branch
    let mut hash = git::get_short_head_hash(&root).unwrap_or_else(|_| "initial".to_string());
    if git::is_working_tree_dirty(&root).unwrap_or(false) {
        hash.push_str("-dirty");
    }

    // 4. Resolve path
    let config = Config::load(&root)?;
    let mem_path = root.join(&config.dir_name);
    if !mem_path.exists() {
        bail!(
            "{} directory does not exist. Run `mem init` first.",
            config.dir_name
        );
    }

    let branch = git::get_current_branch(&root)
        .context("Could not determine current branch. Have you made your first commit yet?")?;
    let branch_dir = branch.replace(['/', '\\'], "-");

    let log_file_path = mem_path.join(&branch_dir).join("spec").join("log.md");

    // 6. Open file and get metadata (to check if it's new) before building markdown
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
    let mut md = String::new();

    // If file doesn't exist, start with header
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

    // Add an extra newline before bullets if we are going to add bullets
    let has_bullets = entry
        .found
        .iter()
        .chain(entry.decided.iter())
        .chain(entry.open.iter())
        .any(|i| !i.trim().is_empty());

    if has_bullets {
        writeln!(&mut md).unwrap(); // Add space before bullets
        push_bullets("Found", &entry.found, &mut md);
        push_bullets("Decided", &entry.decided, &mut md);
        push_bullets("Open", &entry.open, &mut md);
    }

    writeln!(&mut md).unwrap(); // Ensure final separation newline

    // 7. Append to file
    file.write_all(md.as_bytes())
        .with_context(|| format!("Failed to write to {}", log_file_path.display()))?;

    let rel_path = log_file_path.strip_prefix(&root).unwrap_or(&log_file_path);
    eprintln!("✓ Logged");
    println!("{}", rel_path.display());

    Ok(())
}
