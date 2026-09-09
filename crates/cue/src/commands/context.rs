use crate::cli::ContextCommands;
use anyhow::Context as _;
use cuelib::artifact::extract_frontmatter_yaml;
use cuelib::store;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
struct NewContextMetadata<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refs: Option<&'a [String]>,
}

struct NewContextOptions<'a> {
    title: Option<&'a str>,
    kind: &'static str,
    mode: Option<&'static str>,
    description: Option<&'a str>,
    parent: Option<&'a str>,
    refs: &'a [String],
}

#[derive(Deserialize)]
struct StoredContextMetadata {
    title: Option<String>,
    kind: String,
    mode: Option<String>,
    description: Option<String>,
    created_at: u64,
    parent: Option<String>,
    #[serde(default)]
    refs: Vec<String>,
}

#[derive(Serialize)]
struct ContextListEntry {
    context: String,
    scope: String,
    title: Option<String>,
    kind: String,
    mode: Option<String>,
    description: Option<String>,
    created_at: u64,
    parent: Option<String>,
    refs: Vec<String>,
    path: PathBuf,
}

pub fn handle(
    cwd: &Path,
    command: ContextCommands,
    store_root: Option<&Path>,
) -> anyhow::Result<()> {
    match command {
        ContextCommands::Create {
            name,
            title,
            kind,
            mode,
            description,
            parent,
            refs,
        } => {
            let options = NewContextOptions {
                title: title.as_deref(),
                kind: kind.as_str(),
                mode: mode.map(|mode| mode.as_str()),
                description: description.as_deref(),
                parent: parent.as_deref(),
                refs: &refs,
            };
            handle_create(cwd, &name, options, store_root)
        }
        ContextCommands::List { json } => handle_list(cwd, json, store_root),
        ContextCommands::Render { entries, context } => {
            handle_render(cwd, entries, context, store_root)
        }
    }
}

fn handle_render(
    cwd: &Path,
    entries: Vec<String>,
    context: Option<String>,
    store_root: Option<&Path>,
) -> anyhow::Result<()> {
    let rendered = crate::render::render(
        cwd,
        crate::render::RenderOptions {
            entries,
            scope_name: context,
            store_root: store_root.map(Path::to_path_buf),
        },
    )?;
    print!("{rendered}");
    Ok(())
}

fn handle_list(cwd: &Path, json: bool, store_root: Option<&Path>) -> anyhow::Result<()> {
    let scope = store::repository_scope(cwd)?;
    let repository_dir = store::root(store_root)?.join(&scope);
    if !repository_dir.is_dir() {
        anyhow::bail!(
            "no cue store at {}; run `cue init` to create it",
            repository_dir.display()
        );
    }

    let mut contexts = Vec::new();
    for entry in std::fs::read_dir(&repository_dir)? {
        let context_dir = entry?.path();
        let context_path = context_dir.join("context.md");
        if !context_path.is_file() {
            continue;
        }
        let context = context_dir
            .file_name()
            .and_then(|name| name.to_str())
            .context("context directory name is not valid UTF-8")?
            .to_string();
        let frontmatter = extract_frontmatter_yaml(&context_path).with_context(|| {
            format!(
                "could not read context metadata at {}",
                context_path.display()
            )
        })?;
        let metadata: StoredContextMetadata = serde_yaml::from_str(&frontmatter)
            .with_context(|| format!("invalid context metadata at {}", context_path.display()))?;
        contexts.push(ContextListEntry {
            context,
            scope: scope.display().to_string(),
            title: metadata.title,
            kind: metadata.kind,
            mode: metadata.mode,
            description: metadata.description,
            created_at: metadata.created_at,
            parent: metadata.parent,
            refs: metadata.refs,
            path: context_path,
        });
    }
    contexts.sort_by(|left, right| left.context.cmp(&right.context));

    if json {
        println!("{}", serde_json::to_string_pretty(&contexts)?);
    } else {
        for context in contexts {
            println!("{}", context.context);
        }
    }
    Ok(())
}

fn handle_create(
    cwd: &Path,
    name: &str,
    options: NewContextOptions<'_>,
    store_root: Option<&Path>,
) -> anyhow::Result<()> {
    cuelib::head::validate_slug(name)?;

    let repository_scope = store::repository_scope(cwd)?;
    let repository_dir = store::root(store_root)?.join(&repository_scope);
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
        title: options.title,
        kind: options.kind,
        mode: options.mode,
        description: options.description,
        created_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        parent: options.parent,
        refs: (!options.refs.is_empty()).then_some(options.refs),
    };
    let frontmatter = serde_yaml::to_string(&metadata)?;
    std::fs::create_dir(&context_dir)?;
    std::fs::write(context_path, format!("---\n{frontmatter}---\n"))?;

    println!("Created {}/{}", repository_scope.display(), name);
    Ok(())
}
