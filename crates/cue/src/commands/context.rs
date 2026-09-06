use crate::cli::ContextCommands;
use crate::config::Config;
use crate::context::{
    ContextSource, context_json_path, gather_context, init_context, load_context_or_config,
};
use cuelib::store;
use serde::Serialize;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
struct NewContextMetadata {
    kind: &'static str,
    created_at: u64,
}

pub fn handle(cwd: &Path, command: ContextCommands) -> anyhow::Result<()> {
    match command {
        ContextCommands::Create { name, kind } => handle_create(cwd, &name, kind.as_str()),
        ContextCommands::Init { force, task } => handle_init(cwd, force, task.as_deref()),
        ContextCommands::Show { task } => handle_show(cwd, task.as_deref()),
        ContextCommands::Profiles { task } => handle_profiles(cwd, task.as_deref()),
        ContextCommands::Render { profile, task } => handle_render(cwd, profile, task.as_deref()),
        ContextCommands::Path { all, task } => handle_path(cwd, all, task.as_deref()),
    }
}

fn handle_create(cwd: &Path, name: &str, kind: &'static str) -> anyhow::Result<()> {
    cuelib::head::validate_slug(name)?;

    let repository_scope = store::repository_scope(cwd)?;
    let repository_dir = store::root()?.join(&repository_scope);
    if !repository_dir.is_dir() {
        anyhow::bail!(
            "no cue store at {}; run `cue init` to create it",
            repository_dir.display()
        );
    }

    let context_dir = repository_dir.join(name);
    let context_path = context_dir.join("context.md");
    if context_path.exists() {
        anyhow::bail!("Context already exists: {name}");
    }

    let metadata = NewContextMetadata {
        kind,
        created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
    };
    let frontmatter = serde_yaml::to_string(&metadata)?;
    std::fs::create_dir(&context_dir)?;
    std::fs::write(context_path, format!("---\n{frontmatter}---\n"))?;

    println!("Created {}/{}", repository_scope.display(), name);
    Ok(())
}

fn handle_init(cwd: &Path, force: bool, task: Option<&str>) -> anyhow::Result<()> {
    let store_root = store::main_worktree_root(cwd)?;
    let config = Config::load(&store_root)?;
    let resolved_store = store::open(cwd, &config)?;
    let config_path = init_context(cwd, force, task)?;
    let relative_path = config_path
        .strip_prefix(&resolved_store.store_dir)
        .or_else(|_| config_path.strip_prefix(&store_root))
        .unwrap_or(&config_path);
    println!("Created {}", relative_path.display());
    Ok(())
}

fn handle_show(cwd: &Path, task: Option<&str>) -> anyhow::Result<()> {
    let store_root = store::main_worktree_root(cwd)?;
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

fn handle_profiles(cwd: &Path, task: Option<&str>) -> anyhow::Result<()> {
    let store_root = store::main_worktree_root(cwd)?;
    let config = Config::load(&store_root)?;
    let resolved = store::open(cwd, &config)?;
    let scope = cuelib::head::resolve_scope(&resolved.head_dir, task)?;
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
    let store_root = store::main_worktree_root(cwd)?;
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

fn handle_path(cwd: &Path, all: bool, task: Option<&str>) -> anyhow::Result<()> {
    let store_root = store::main_worktree_root(cwd)?;
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
        let scope = cuelib::head::resolve_scope(&resolved.head_dir, task)?;
        let config_path = context_json_path(&resolved.store_dir, &scope);
        if config_path.exists() {
            println!("{}", config_path.display());
        } else {
            anyhow::bail!("Context file not found for scope: {}", scope);
        }
    }

    Ok(())
}
