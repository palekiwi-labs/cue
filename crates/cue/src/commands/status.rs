use anyhow::{Context, Result};
use cuelib::artifact::extract_frontmatter_yaml;
use cuelib::{head, store};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;

#[derive(Deserialize)]
struct ContextMetadata {
    title: Option<String>,
    kind: String,
    mode: Option<String>,
    parent: Option<String>,
}

pub fn handle(
    cwd: &Path,
    context: Option<String>,
    json_output: bool,
    store_root: Option<&Path>,
) -> Result<()> {
    let store_root = store::root(store_root)?;
    let scope = store::repository_scope(cwd)?;
    let Some(context) = head::resolve_active_context(cwd, context.as_deref())? else {
        if json_output {
            println!(
                "{}",
                json!({
                    "context": null,
                    "store": store_root.display().to_string(),
                    "scope": scope.display().to_string(),
                })
            );
        } else {
            println!("active context: unset");
            println!("  store: {}", store_root.display());
            println!("  scope: {}", scope.display());
        }
        return Ok(());
    };
    let context_path = store_root.join(&scope).join(&context).join("context.md");
    let frontmatter = extract_frontmatter_yaml(&context_path).with_context(|| {
        format!(
            "could not read context metadata at {}",
            context_path.display()
        )
    })?;
    let metadata: ContextMetadata = serde_yaml::from_str(&frontmatter)
        .with_context(|| format!("invalid context metadata at {}", context_path.display()))?;

    if json_output {
        println!(
            "{}",
            json!({
                "context": context,
                "title": metadata.title,
                "kind": metadata.kind,
                "mode": metadata.mode,
                "parent": metadata.parent,
                "store": store_root.display().to_string(),
                "scope": scope.display().to_string(),
            })
        );
    } else {
        println!("active context: {context}");
        if let Some(title) = metadata.title {
            println!("  title: {title}");
        }
        println!("  kind: {}", metadata.kind);
        if let Some(mode) = metadata.mode {
            println!("  mode: {mode}");
        }
        if let Some(parent) = metadata.parent {
            println!("  parent: {parent}");
        }
        println!("  store: {}", store_root.display());
        println!("  scope: {}", scope.display());
    }

    Ok(())
}
