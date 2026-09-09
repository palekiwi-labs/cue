use anyhow::{Context, Result, bail};
use cuelib::store;
use std::path::{Path, PathBuf};

pub struct RenderOptions {
    pub entries: Vec<String>,
    pub scope_name: Option<String>,
    pub store_root: Option<PathBuf>,
}

/// Resolve the active context directory, requiring both a selected context and
/// an existing one, matching `cue log list`.
fn resolve_context_dir(
    root: &Path,
    scope_name: Option<&str>,
    store_root: Option<&Path>,
) -> Result<PathBuf> {
    let context = cuelib::head::resolve_active_context(root, scope_name)?
        .context("No context selected; pass --context <context>")?;
    let context_dir = store::root(store_root)?
        .join(store::repository_scope(root)?)
        .join(&context);
    if !context_dir.join("context.md").is_file() {
        bail!("Context does not exist: {context}");
    }
    Ok(context_dir)
}

/// Render the requested entries as `<artifact>` blocks.
///
/// Entry paths are joined to the context directory without validation: the
/// operator supplies them directly, so escaping the context is permitted.
/// Anything that does not resolve to a readable file is skipped silently.
pub fn render(root: &Path, opts: RenderOptions) -> Result<String> {
    let RenderOptions {
        entries,
        scope_name,
        store_root,
    } = opts;

    let context_dir = resolve_context_dir(root, scope_name.as_deref(), store_root.as_deref())?;

    let mut rendered = String::new();
    for entry in dedup(entries) {
        let path = context_dir.join(&entry);
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        rendered.push_str(&format!(
            "<artifact path=\"{}\">\n{content}\n</artifact>\n\n",
            path.display()
        ));
    }

    Ok(rendered)
}

/// Remove repeated entries, keeping the first occurrence so argument order is
/// preserved.
fn dedup(entries: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    entries
        .into_iter()
        .filter(|entry| seen.insert(entry.clone()))
        .collect()
}
