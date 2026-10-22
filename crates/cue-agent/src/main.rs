mod batch;
mod cli;
mod engine;
mod harness;
mod ids;
mod manifest;
mod monitor;
mod receipt;
mod run_spec;
mod state;
mod string_source;
mod trace;
mod worktree;

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
        Commands::Ps { all, json } => {
            let report = monitor::ps(all)?;
            if json {
                println!("{}", serde_json::to_string(&report)?);
            } else {
                for batch in report["batches"].as_array().into_iter().flatten() {
                    print!("{}", monitor::human_batch(batch));
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Status { id, json } => {
            let report = monitor::status(&id)?;
            if json {
                println!("{}", serde_json::to_string(&report)?);
            } else if report.get("runs").is_some() {
                print!("{}", monitor::human_batch(&report));
            } else {
                print!("{}", monitor::human_run(&report));
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Logs { id, stderr, follow } => {
            monitor::logs(&id, stderr, follow)?;
            Ok(ExitCode::SUCCESS)
        }
        Commands::Stop { id, wait, json } => {
            let report = monitor::stop(&id, wait)?;
            if json {
                println!("{}", serde_json::to_string(&report)?);
            } else {
                println!("{}: {}", id, report["result"].as_str().unwrap_or_default());
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Run(args) => {
            let outcome = batch::execute(&args)?;
            if args.json {
                println!("{}", serde_json::to_string(&outcome.receipt)?);
            } else {
                print!("{}", receipt::human(&outcome.receipt));
            }
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
                    "models": agent.models,
                    "default_model": agent.model_alias,
                    "model": agent.model,
                    "system_prompt": agent.system_prompt,
                    "tools": agent.tools,
                    "thinking": agent.thinking,
                    "source": agent.source.as_str(),
                    "field_sources": agent
                        .field_sources
                        .iter()
                        .map(|(field, source)| (field.to_string(), source.as_str().into()))
                        .collect::<serde_json::Map<String, serde_json::Value>>(),
                })
            })
            .collect();
        let models: serde_json::Map<String, serde_json::Value> = manifest
            .models()
            .iter()
            .map(|(alias, profile)| {
                let entry = serde_json::json!({
                    "id": profile.id,
                    "thinking": profile.thinking,
                    "provider": profile.provider,
                });
                (alias.clone(), entry)
            })
            .collect();
        let listing = serde_json::json!({
            "models": models,
            "agents": agents,
            "global_manifest": manifest.global_path,
            "project_manifest": manifest.project_path,
        });
        println!("{}", serde_json::to_string(&listing)?);
        return Ok(());
    }

    for agent in manifest.agents() {
        let description = agent.description.as_deref().unwrap_or("");
        // Allowlist with the default first and starred, as the delegate
        // roster renders it.
        let models = match &agent.models {
            Some(aliases) => aliases
                .iter()
                .enumerate()
                .map(|(index, alias)| {
                    if index == 0 {
                        format!("{alias}*")
                    } else {
                        alias.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(", "),
            None => "(harness default)".to_string(),
        };
        println!("{}  [{}]  {}", agent.name, models, description);
    }
    Ok(())
}
