mod batch;
mod cli;
mod engine;
mod harness;
mod ids;
mod manifest;
mod receipt;
mod state;
mod trace;

use crate::cli::{AgentCommands, Cli, Commands};
use anyhow::Result;
use clap::Parser;
use std::process::ExitCode;

/// A batch that produced a receipt but contains a failed run.
const EXIT_RUN_FAILED: u8 = 1;
/// A request that never reached the harness: bad arguments, an unknown agent,
/// an oversized batch, an unreadable manifest.
const EXIT_USAGE: u8 = 2;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("cue-agent: {err:#}");
            ExitCode::from(EXIT_USAGE)
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Agents { command } => {
            let AgentCommands::List { json } = command;
            list_agents(json)?;
            Ok(ExitCode::SUCCESS)
        }
        Commands::Run(args) => {
            let outcome = batch::execute(&args)?;
            println!("{}", serde_json::to_string(&outcome.receipt)?);
            Ok(if outcome.all_completed {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(EXIT_RUN_FAILED)
            })
        }
    }
}

fn list_agents(json: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let manifest = manifest::load(&cwd)?;

    if json {
        let agents: Vec<serde_json::Value> = manifest
            .agents()
            .map(|agent| {
                serde_json::json!({
                    "name": agent.name,
                    "description": agent.description,
                    "model": agent.model,
                    "system_prompt": agent.system_prompt,
                    "tools": agent.tools,
                    "thinking": agent.thinking,
                    "timeout_secs": agent.timeout_secs,
                    "source": agent.source.as_str(),
                })
            })
            .collect();
        let listing = serde_json::json!({
            "agents": agents,
            "global_manifest": manifest.global_path,
            "project_manifest": manifest.project_path,
        });
        println!("{}", serde_json::to_string(&listing)?);
        return Ok(());
    }

    for agent in manifest.agents() {
        let description = agent.description.as_deref().unwrap_or("");
        let model = agent.model.as_deref().unwrap_or("(harness default)");
        println!("{}  [{}]  {}", agent.name, model, description);
    }
    Ok(())
}
