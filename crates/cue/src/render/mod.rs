use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

pub struct RenderOptions {
    pub entries: Vec<String>,
    pub context: Option<String>,
    pub store_root: Option<PathBuf>,
}

/// Resolve the active context directory, requiring both a selected context and
/// an existing one, matching `cue log list`.
fn resolve_context_dir(
    root: &Path,
    context: Option<&str>,
    store_root: Option<&Path>,
) -> Result<PathBuf> {
    let resolved = cuelib::head::resolve_active_context(root, context)?
        .context("No context selected; pass --context <context>")?;
    let context_dir = resolved.context_dir(root, store_root)?;
    if !context_dir.join("context.md").is_file() {
        bail!("Context does not exist: {}", resolved.address(root)?);
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
        context,
        store_root,
    } = opts;

    let entries = dedup(entries);
    let context_dir = if entries.iter().any(|entry| Path::new(entry).is_relative()) {
        Some(resolve_context_dir(
            root,
            context.as_deref(),
            store_root.as_deref(),
        )?)
    } else {
        None
    };

    let mut rendered = String::new();
    for entry in entries {
        let path = if Path::new(&entry).is_absolute() {
            PathBuf::from(entry)
        } else {
            context_dir
                .as_ref()
                .expect("relative entries require a context directory")
                .join(entry)
        };
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
