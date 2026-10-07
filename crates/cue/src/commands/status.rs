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
    let Some(resolved) = head::resolve_active_context(cwd, context.as_deref())? else {
        // Nothing is selected, so no selection names a scope: only the cwd
        // repository's scope can be reported.
        let cwd_scope = store::repository_scope(cwd)?;
        if json_output {
            println!(
                "{}",
                json!({
                    "context": null,
                    "address": null,
                    "store": store_root.display().to_string(),
                    "scope": cwd_scope.display().to_string(),
                })
            );
        } else {
            println!("active context: unset");
            println!("  store: {}", store_root.display());
            println!("  scope: {}", cwd_scope.display());
        }
        return Ok(());
    };
    // A bare slug identifies a context only to someone who already knows the
    // repository. The canonical address is the form that survives being
    // copied out of this repository, so it is emitted alongside the slug
    // rather than left for a caller to concatenate. A canonical selector
    // addresses its own scope, so the cwd repository is consulted only for
    // the bare-slug form: answering an address needs nothing from it.
    let scope = match &resolved.scope {
        Some(addressed) => addressed.clone(),
        None => store::repository_scope(cwd)?.display().to_string(),
    };
    let context = resolved.slug;
    let address = format!("{scope}/{context}");
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
                "address": address,
                "title": metadata.title,
                "kind": metadata.kind,
                "mode": metadata.mode,
                "parent": metadata.parent,
                "store": store_root.display().to_string(),
                "scope": scope,
            })
        );
    } else {
        println!("active context: {context}");
        println!("  address: {address}");
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
        println!("  scope: {scope}");
    }

    Ok(())
}
