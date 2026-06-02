use crate::cli::ConfigCommands;
use crate::config::Config;
use anyhow::Result;
use std::path::Path;

pub fn handle(cwd: &Path, command: ConfigCommands) -> Result<()> {
    match command {
        ConfigCommands::Show => {
            let config = Config::load(cwd)?;
            println!("{}", serde_json::to_string_pretty(&config)?);
        }
    }
    Ok(())
}
