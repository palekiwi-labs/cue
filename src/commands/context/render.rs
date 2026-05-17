use crate::commands::context::{context_json_path, load_context_config, resolve_profile};
use crate::config::Config;
use crate::git::{get_current_branch, get_git_root, run_git};
use std::collections::HashSet;
use std::path::Path;

pub fn handle(cwd: &Path, profile: Option<String>) -> anyhow::Result<()> {
    let profile_name = profile.unwrap_or_else(|| "default".to_string());
    let branch = get_current_branch(cwd)?;
    let sanitized_branch = branch.replace("/", "-").replace("\\", "-");
    let git_root = get_git_root(cwd)?;
    let config = Config::load(&git_root)?;

    let mut visited = HashSet::new();
    let paths = resolve_profile(&sanitized_branch, &profile_name, &git_root, &mut visited)?;

    // 1. & 2. Output artifacts
    for path in paths {
        if !path.exists() {
            eprintln!("Warning: Artifact not found: {}", path.display());
            continue;
        }

        let content = std::fs::read_to_string(&path)?;
        let relative_path = path.strip_prefix(&git_root).unwrap_or(&path);

        println!(
            "<artifact path=\"{}\">\n{}\n</artifact>\n",
            relative_path.display(),
            content
        );
    }

    // 3. Diff block
    let context_path = context_json_path(&git_root, &sanitized_branch);
    let context_config = load_context_config(&context_path)?;
    let profile_obj = context_config.get(&profile_name).ok_or_else(|| {
        anyhow::anyhow!(
            "Profile '{}' not found in {}",
            profile_name,
            context_path.display()
        )
    })?;

    if let Some(diff_args) = &profile_obj.diff {
        let mut args = vec!["diff"];
        let split_args: Vec<&str> = diff_args.split_whitespace().collect();
        args.extend(split_args);

        // Apply diff_exclude_paths
        let mut exclude_args = Vec::new();
        if !config.diff_exclude_paths.is_empty() {
            args.push("--");
            for pattern in &config.diff_exclude_paths {
                exclude_args.push(format!(":(exclude){}", pattern));
            }
            for arg in &exclude_args {
                args.push(arg);
            }
        }

        match run_git(args, &git_root) {
            Ok(diff_output) => {
                println!("<diff args=\"{}\">\n{}\n</diff>", diff_args, diff_output);
            }
            Err(e) => {
                eprintln!("Warning: git diff failed: {}", e);
            }
        }
    }

    Ok(())
}
