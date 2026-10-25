use crate::activity;
use crate::cli::ActivityCommands;
use anyhow::{Context, Result};
use std::path::Path;

pub fn handle(cwd: &Path, command: ActivityCommands, store: Option<&Path>) -> Result<()> {
    match command {
        ActivityCommands::Add {
            context,
            kind,
            details,
            session_id,
            harness,
            host,
        } => {
            let details =
                serde_json::from_str(&details).context("Details must be a JSON object")?;
            if let Some(dir) = activity::context_dir(cwd, context.as_deref(), store)? {
                let path = activity::record(&dir, kind, details, session_id, harness, host)?;
                println!("{}", path.display());
            }
        }
        ActivityCommands::List {
            context,
            json,
            since,
            limit,
        } => {
            let dir = activity::context_dir(cwd, context.as_deref(), store)?
                .context("No context selected; pass --context <context>")?;
            let events = activity::list(&dir, since, limit)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&events)?);
            } else {
                for event in events {
                    println!(
                        "{} {} {} {}",
                        event.timestamp,
                        event.id,
                        event.kind.as_str(),
                        serde_json::to_string(&event.details)?
                    );
                }
            }
        }
    }
    Ok(())
}
