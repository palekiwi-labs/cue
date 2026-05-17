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

pub fn parse_artifact_path(
    raw: &str,
    current_branch_dir: &str,
    git_root: &Path,
) -> anyhow::Result<PathBuf> {
    let (branch, rest) = if let Some(stripped) = raw.strip_prefix('@') {
        // Cross-branch reference
        // Use rsplit_once to allow colons in branch names (splitting on the last colon)
        let (b, p) = match stripped.rsplit_once(':') {
            Some((branch, path)) => (branch, path),
            None => (stripped, ""),
        };

        if b.contains('/') || b.contains('\\') {
            anyhow::bail!(
                "Branch component in cross-branch reference must be a sanitized name (no slashes)"
            );
        }

        (b, p)
    } else {
        // Local artifact. Defaults to current branch.
        // We optionally strip a leading "./" for cleaner aesthetics.
        let p = raw.strip_prefix("./").unwrap_or(raw);
        (current_branch_dir, p)
    };

    let rest_path = Path::new(rest);

    // Prevent base path overwrite via `join`
    if rest_path.has_root() {
        anyhow::bail!(
            "Absolute or root paths are not allowed in artifact paths: {}",
            raw
        );
    }

    Ok(git_root.join(".mem").join(branch).join(rest_path))
}

pub fn resolve_profile(
    branch_dir: &str,
    profile_name: &str,
    git_root: &Path,
    visited: &mut std::collections::HashSet<(String, String)>,
) -> anyhow::Result<Vec<PathBuf>> {
    let key = (branch_dir.to_string(), profile_name.to_string());
    if visited.contains(&key) {
        anyhow::bail!(
            "Cycle detected in context profile includes: {}:{}",
            branch_dir,
            profile_name
        );
    }
    visited.insert(key.clone());

    let config_path = context_json_path(git_root, branch_dir);
    let config = match load_context_config(&config_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!(
                "Warning: Could not load context for branch {}, skipping",
                branch_dir
            );
            visited.remove(&key);
            return Ok(Vec::new());
        }
    };

    let profile = config.get(profile_name).ok_or_else(|| {
        visited.remove(&key);
        anyhow::anyhow!(
            "Profile '{}' not found in {}",
            profile_name,
            config_path.display()
        )
    })?;

    let mut accumulator = Vec::new();

    for inc in &profile.include {
        let (inc_branch, inc_profile) = if let Some(rest) = inc.strip_prefix('@') {
            match rest.split_once(':') {
                Some((b, p)) => (b.to_string(), p.to_string()),
                None => (rest.to_string(), "default".to_string()),
            }
        } else {
            visited.remove(&key);
            anyhow::bail!(
                "Invalid include format: {}. Expected @branch or @branch:profile",
                inc
            );
        };

        let inc_paths = resolve_profile(&inc_branch, &inc_profile, git_root, visited)?;
        accumulator.extend(inc_paths);
    }

    for art in &profile.artifacts {
        let path = parse_artifact_path(art, branch_dir, git_root)?;
        accumulator.push(path);
    }

    visited.remove(&key);

    // Deduplicate: first occurrence wins
    let mut final_paths = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for path in accumulator {
        if seen.insert(path.clone()) {
            final_paths.push(path);
        }
    }

    Ok(final_paths)
}

pub fn handle(cwd: &Path, command: ContextCommands) -> anyhow::Result<()> {
    match command {
        ContextCommands::Init { force } => init::handle(cwd, force),
        ContextCommands::Show => show::handle(cwd),
        ContextCommands::Profiles => profiles::handle(cwd),
        ContextCommands::Render { profile } => render::handle(cwd, profile),
    }
}
