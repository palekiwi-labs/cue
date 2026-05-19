use crate::cli::ContextCommands;
use crate::context::{context_json_path, gather_context, init_context, load_context_config};
use crate::git::{get_current_branch, get_git_root, sanitize_branch_name};
use std::path::Path;

pub fn handle(cwd: &Path, command: ContextCommands) -> anyhow::Result<()> {
    match command {
        ContextCommands::Init { force } => handle_init(cwd, force),
        ContextCommands::Show => handle_show(cwd),
        ContextCommands::Profiles => handle_profiles(cwd),
        ContextCommands::Render { profile } => handle_render(cwd, profile),
        ContextCommands::Path { all } => handle_path(cwd, all),
    }
}

fn handle_init(cwd: &Path, force: bool) -> anyhow::Result<()> {
    let git_root = get_git_root(cwd)?;
    let config_path = init_context(cwd, force)?;
    let relative_path = config_path.strip_prefix(&git_root).unwrap_or(&config_path);
    println!("Created {}", relative_path.display());
    Ok(())
}

fn handle_show(cwd: &Path) -> anyhow::Result<()> {
    let git_root = get_git_root(cwd)?;
    let branch = get_current_branch(cwd)?;
    let sanitized_branch = sanitize_branch_name(&branch);
    let config_path = context_json_path(&git_root, &sanitized_branch);

    let config = load_context_config(&config_path)?;
    println!("{}", serde_json::to_string_pretty(&config)?);

    Ok(())
}

fn handle_profiles(cwd: &Path) -> anyhow::Result<()> {
    let git_root = get_git_root(cwd)?;
    let branch = get_current_branch(cwd)?;
    let sanitized_branch = sanitize_branch_name(&branch);
    let config_path = context_json_path(&git_root, &sanitized_branch);

    let config = load_context_config(&config_path)?;
    let mut names: Vec<_> = config.keys().collect();
    names.sort();

    for name in names {
        println!("{}", name);
    }

    Ok(())
}

fn handle_render(cwd: &Path, profile: Option<String>) -> anyhow::Result<()> {
    let git_root = get_git_root(cwd)?;
    let resolved = gather_context(cwd, profile.as_deref())?;

    for artifact in resolved.artifacts {
        let relative_path = artifact
            .path
            .strip_prefix(&git_root)
            .unwrap_or(&artifact.path);
        let normalized_path = relative_path.display().to_string().replace('\\', "/");

        println!(
            "<artifact path=\"{}\">\n{}\n</artifact>\n",
            normalized_path, artifact.content
        );
    }

    if let Some(instructions) = resolved.instructions {
        println!("<instructions>\n{}\n</instructions>", instructions);
    }

    Ok(())
}

fn handle_path(cwd: &Path, all: bool) -> anyhow::Result<()> {
    let git_root = get_git_root(cwd)?;

    if all {
        let mem_dir = git_root.join(".mem");
        if !mem_dir.exists() {
            return Ok(());
        }

        let mut paths = Vec::new();
        for entry in std::fs::read_dir(mem_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let context_file = entry.path().join("context.json");
                if context_file.exists() {
                    paths.push(context_file);
                }
            }
        }
        paths.sort();
        for path in paths {
            println!("{}", path.display());
        }
    } else {
        let branch = get_current_branch(cwd)?;
        let sanitized_branch = sanitize_branch_name(&branch);
        let config_path = context_json_path(&git_root, &sanitized_branch);
        if config_path.exists() {
            println!("{}", config_path.display());
        } else {
            anyhow::bail!("Context file not found for branch: {}", branch);
        }
    }

    Ok(())
}
