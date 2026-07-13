use crate::cli::LogCommands;
use crate::config::Config;
use crate::git;
use crate::log::{self, LogAddOptions, LogEntry};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn handle(cwd: &Path, command: LogCommands) -> Result<()> {
    // 1. Verify git repo
    git::run_git(["rev-parse", "--git-dir"], cwd).context("Not in a git repository")?;

    // 2. Get git root
    let root = git::get_git_root(cwd)?;

    // 3. Load config
    let config = Config::load(&root)?;

    match command {
        LogCommands::Add {
            title,
            body,
            found,
            decided,
            open,
            file,
            task,
        } => {
            let entry = if let Some(path) = file {
                let content = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read JSON file: {}", path))?;
                let entry: LogEntry = serde_json::from_str(&content)
                    .with_context(|| format!("Failed to parse JSON file: {}", path))?;
                entry
            } else {
                let title =
                    title.context("The --title argument is required when not using --file")?;
                LogEntry {
                    title,
                    body,
                    found,
                    decided,
                    open,
                }
            };

            let log_file_path = log::add_entry(
                &root,
                &config,
                LogAddOptions {
                    entry,
                    scope_name: task,
                },
            )?;
            let rel_path = log_file_path.strip_prefix(&root).unwrap_or(&log_file_path);
            eprintln!("✓ Logged");
            println!("{}", rel_path.display());
        }
        LogCommands::List { branch } => {
            let branch_name = if let Some(b) = branch {
                b
            } else {
                let cue_path = root.join(&config.dir_name);
                cuelib::head::resolve_scope(&cue_path)?
            };
            let branch_dir = git::sanitize_branch_name(&branch_name);

            let cue_path = root.join(&config.dir_name);
            if !cue_path.exists() {
                return Ok(()); // Silently exit
            }

            let log_file_path = cue_path.join(&branch_dir).join("log.md");

            match fs::read_to_string(&log_file_path) {
                Ok(content) => print!("{}", content),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {} // Silently exit
                Err(e) => {
                    return Err(e)
                        .with_context(|| format!("Failed to read {}", log_file_path.display()));
                }
            }
        }
    }

    Ok(())
}
