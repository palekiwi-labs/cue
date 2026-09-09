use crate::cli::LogCommands;
use crate::git;
use crate::log::{self, LogAddOptions, LogEntry};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn handle(cwd: &Path, command: LogCommands, store_root: Option<&Path>) -> Result<()> {
    // 1. Verify git repo
    git::run_git(["rev-parse", "--git-dir"], cwd).context("Not in a git repository")?;

    match command {
        LogCommands::Add {
            title,
            trace,
            found,
            decided,
            open,
            file,
            context,
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
                    trace,
                    found,
                    decided,
                    open,
                }
            };

            let log_file_path = log::add_entry(
                cwd,
                LogAddOptions {
                    entry,
                    scope_name: context,
                    store_root: store_root.map(Path::to_path_buf),
                },
            )?;
            let rel_path = log_file_path.strip_prefix(cwd).unwrap_or(&log_file_path);
            eprintln!("Logged");
            println!("{}", rel_path.display());
        }
        LogCommands::List { task } => {
            let entries = log::list_entries(cwd, task.as_deref(), store_root)?;
            println!("{}", serde_json::to_string_pretty(&entries)?);
        }
    }

    Ok(())
}
