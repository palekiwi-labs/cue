use crate::cli::ConfigCommands;
use crate::config::Config;
use anyhow::Result;
use std::path::Path;

pub fn handle(cwd: &Path, command: ConfigCommands) -> Result<()> {
    match command {
        ConfigCommands::Show => {
            let root = cuelib::store::main_worktree_root(cwd).unwrap_or_else(|_| cwd.to_path_buf());
            let config = Config::load(&root)?;
            println!("{}", serde_json::to_string_pretty(&config)?);
        }
    }
    Ok(())
}
