use crate::context::{context_json_path, ContextProfile};
use crate::git::{get_current_branch, get_git_root, sanitize_branch_name};
use std::collections::HashMap;
use std::path::Path;

pub fn handle(cwd: &Path, force: bool) -> anyhow::Result<()> {
    let branch = get_current_branch(cwd)?;
    let sanitized_branch = sanitize_branch_name(&branch);
    let config_path = context_json_path(cwd, &sanitized_branch);

    if config_path.exists() && !force {
        anyhow::bail!(
            "Context file already exists: {}. Use --force to overwrite.",
            config_path.display()
        );
    }

    let git_root = get_git_root(cwd)?;
    let mem_branch_path = cwd.join(".mem").join(&sanitized_branch);
    let spec_path = mem_branch_path.join("spec");

    let mut artifacts = Vec::new();
    if spec_path.exists() {
        let mut entries: Vec<_> = std::fs::read_dir(&spec_path)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            if let Some(name) = entry.file_name().to_str() {
                artifacts.push(format!("./spec/{}", name));
            }
        }
    }

    let profile = ContextProfile {
        artifacts,
        diff: None,
        include: Vec::new(),
    };

    let mut config = HashMap::new();
    config.insert("default".to_string(), profile);

    let json = serde_json::to_string_pretty(&config)?;
    std::fs::create_dir_all(config_path.parent().unwrap())?;
    std::fs::write(&config_path, json)?;

    let relative_path = config_path.strip_prefix(&git_root).unwrap_or(&config_path);
    println!("Created {}", relative_path.display());

    Ok(())
}
