mod add;
mod cli;
mod commands;
mod git;
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
    let store_root = cli.store.as_deref();

    match cli.command {
        Commands::Init => {
            commands::init::handle(&cwd, store_root)?;
        }
        Commands::Add {
            filename,
            content,
            file,
            clipboard,
            frontmatter,
            cue_type,
            force,
            context,
            group,
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
                    force,
                    scope_name: context,
                    store_root: store_root.map(std::path::Path::to_path_buf),
                    group,
                },
            )?;
        }
        Commands::List {
            task,
            cue_type,
            json,
            frontmatter,
            filters,
        } => {
            commands::list::handle(
                &cwd,
                commands::list::ListOptions {
                    scope: task,
                    cue_type,
                    json,
                    frontmatter,
                    store_root: store_root.map(std::path::Path::to_path_buf),
                    filters,
                },
            )?;
        }
        Commands::Log { command } => {
            commands::log::handle(&cwd, command, store_root)?;
        }
        Commands::Status { context, json } => {
            commands::status::handle(&cwd, context, json, store_root)?;
        }
        Commands::Context { command } => {
            commands::context::handle(&cwd, command, store_root)?;
        }
    }

    Ok(())
}
