pub mod init;
pub mod profiles;
pub mod render;
pub mod show;

#[cfg(test)]
mod tests;

use crate::cli::ContextCommands;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct ContextProfile {
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub diff: Option<String>,
    #[serde(default)]
    pub include: Vec<String>,
}

pub type ContextConfig = HashMap<String, ContextProfile>;

pub fn context_json_path(cwd: &Path, branch_dir: &str) -> PathBuf {
    cwd.join(".mem").join(branch_dir).join("context.json")
}

pub fn load_context_config(path: &Path) -> anyhow::Result<ContextConfig> {
    if !path.exists() {
        anyhow::bail!("Context file not found: {}", path.display());
    }
    let content = std::fs::read_to_string(path)?;
    let config: ContextConfig = serde_json::from_str(&content)?;
    Ok(config)
}

pub fn handle(cwd: &Path, command: ContextCommands) -> anyhow::Result<()> {
    match command {
        ContextCommands::Init { force } => init::handle(cwd, force),
        ContextCommands::Show => show::handle(cwd),
        ContextCommands::Profiles => profiles::handle(cwd),
        ContextCommands::Render { profile } => render::handle(cwd, profile),
    }
}
