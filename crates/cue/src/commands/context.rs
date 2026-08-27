use crate::cli::ContextCommands;
use crate::config::Config;
use crate::context::{
    context_json_path, gather_context, init_context, load_context_or_config, ContextSource,
};
use cuelib::store;
use std::path::Path;

pub fn handle(cwd: &Path, command: ContextCommands) -> anyhow::Result<()> {
    match command {
        ContextCommands::Init { force } => handle_init(cwd, force),
        ContextCommands::Show { task } => handle_show(cwd, task.as_deref()),
        ContextCommands::Profiles => handle_profiles(cwd),
        ContextCommands::Render { profile, task } => {
            handle_render(cwd, profile, task.as_deref())
        }
        ContextCommands::Path { all } => handle_path(cwd, all),
    }
}

fn handle_init(cwd: &Path, force: bool) -> anyhow::Result<()> {
    let store_root = store::git_root(cwd)?;
    let config = Config::load(&store_root)?;
    let resolved_store = store::open(cwd, &config)?;
    let config_path = init_context(cwd, force)?;
    let relative_path = config_path
        .strip_prefix(&resolved_store.store_dir)
        .or_else(|_| config_path.strip_prefix(&store_root))
        .unwrap_or(&config_path);
    println!("Created {}", relative_path.display());
    Ok(())
}

fn handle_show(cwd: &Path, task: Option<&str>) -> anyhow::Result<()> {
    let store_root = store::git_root(cwd)?;
    let config = Config::load(&store_root)?;
    let resolved = store::open(cwd, &config)?;
    let scope = cuelib::head::resolve_scope(&resolved.head_dir, task)?;
    let config_path = context_json_path(&resolved.store_dir, &scope);

    let (context_config, source) = load_context_or_config(&config_path, &config.context)?;
    if source == ContextSource::ConfigDefault {
        eprintln!("(no context.json; showing config default)");
    }
    println!("{}", serde_json::to_string_pretty(&context_config)?);

    Ok(())
}

fn handle_profiles(cwd: &Path) -> anyhow::Result<()> {
    let store_root = store::git_root(cwd)?;
    let config = Config::load(&store_root)?;
    let resolved = store::open(cwd, &config)?;
    let scope = cuelib::head::resolve_scope(&resolved.head_dir, None)?;
    let config_path = context_json_path(&resolved.store_dir, &scope);

    let (context_config, source) = load_context_or_config(&config_path, &config.context)?;
    if source == ContextSource::ConfigDefault {
        eprintln!("(no context.json; showing config default)");
    }
    let mut names: Vec<_> = context_config.keys().collect();
    names.sort();

    for name in names {
        println!("{}", name);
    }

    Ok(())
}

fn handle_render(cwd: &Path, profile: Option<String>, task: Option<&str>) -> anyhow::Result<()> {
    let store_root = store::git_root(cwd)?;
    let config = Config::load(&store_root)?;
    let resolved_store = store::open(cwd, &config)?;
    let (resolved, source) = gather_context(cwd, profile.as_deref(), task)?;
    if source == ContextSource::ConfigDefault {
        eprintln!("(no context.json; using config default)");
    }

    for artifact in resolved.artifacts {
        let relative_path = artifact
            .path
            .strip_prefix(&resolved_store.store_dir)
            .or_else(|_| artifact.path.strip_prefix(&store_root))
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
    let store_root = store::git_root(cwd)?;
    let config = Config::load(&store_root)?;
    let resolved = store::open(cwd, &config)?;

    if all {
        if !resolved.store_dir.exists() {
            return Ok(());
        }

        let mut paths = Vec::new();
        for entry in std::fs::read_dir(&resolved.store_dir)? {
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
        let scope = cuelib::head::resolve_scope(&resolved.head_dir, None)?;
        let config_path = context_json_path(&resolved.store_dir, &scope);
        if config_path.exists() {
            println!("{}", config_path.display());
        } else {
            anyhow::bail!("Context file not found for scope: {}", scope);
        }
    }

    Ok(())
}
