mod add;
mod cli;
mod commands;
mod config;
mod context;
mod git;
mod init;
mod list;
mod log;

use crate::add::resolve_clipboard;
use crate::cli::{Cli, Commands};
use anyhow::Context;
use clap::Parser;
use std::env;
use std::io::{self, Read};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cwd = match cli.dir {
        Some(ref path) => {
            let md = std::fs::metadata(path)
                .map_err(|_| anyhow::anyhow!("--dir: path does not exist: {}", path.display()))?;
            if !md.is_dir() {
                anyhow::bail!("--dir: not a directory: {}", path.display());
            }
            path.canonicalize()?
        }
        None => env::current_dir()?,
    };

    match cli.command {
        Commands::Init => {
            commands::init::handle(&cwd)?;
        }
        Commands::Add {
            filename,
            content,
            file,
            clipboard,
            frontmatter,
            cue_type,
            root,
            force,
            task,
        } => {
            let resolved_content: Vec<u8> = if clipboard {
                resolve_clipboard(&filename)?
            } else if let Some(path) = file {
                std::fs::read(&path).with_context(|| format!("Failed to read file {}", path))?
            } else {
                let c = content.unwrap_or_else(|| "-".to_string());
                if c == "-" {
                    let mut buf = Vec::new();
                    io::stdin()
                        .read_to_end(&mut buf)
                        .context("Failed to read from stdin")?;
                    buf
                } else {
                    c.into_bytes()
                }
            };

            commands::add::handle(
                &cwd,
                commands::add::AddOptions {
                    filename,
                    content: resolved_content,
                    frontmatter,
                    cue_type,
                    save_at_root: root,
                    force,
                    scope_name: task,
                },
            )?;
        }
        Commands::List {
            task,
            all,
            cue_type,
            include_gitignored,
            json,
            frontmatter,
            filters,
        } => {
            commands::list::handle(
                &cwd,
                commands::list::ListOptions {
                    branch_name: task,
                    all,
                    cue_type,
                    include_gitignored,
                    json,
                    frontmatter,
                    filters,
                },
            )?;
        }
        Commands::Log { command } => {
            commands::log::handle(&cwd, command)?;
        }
        Commands::Switch {
            target,
            branch,
            json,
        } => {
            commands::switch::handle(&cwd, target, branch, json)?;
        }
        Commands::Status { json } => {
            commands::status::handle(&cwd, json)?;
        }
        Commands::Context { command } => {
            commands::context::handle(&cwd, command)?;
        }
        Commands::Config { command } => {
            commands::config::handle(&cwd, command)?;
        }
        Commands::Project { command } => {
            commands::project::handle(&cwd, command)?;
        }
    }

    Ok(())
}
