use crate::context::{context_json_path, load_context_config};
use crate::git::{get_current_branch, sanitize_branch_name};
use std::path::Path;

pub fn handle(cwd: &Path) -> anyhow::Result<()> {
    let branch = get_current_branch(cwd)?;
    let sanitized_branch = sanitize_branch_name(&branch);
    let config_path = context_json_path(cwd, &sanitized_branch);

    let config = load_context_config(&config_path)?;
    println!("{}", serde_json::to_string_pretty(&config)?);

    Ok(())
}
