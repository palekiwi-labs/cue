use anyhow::{bail, Context, Result};
use cuelib::{head, store};
use std::fs;
use std::path::{Path, PathBuf};

pub fn handle(cwd: &Path, store_path: PathBuf, task: Option<String>) -> Result<()> {
    // 1. Validate store_path: exists, contains master/, not a proxy (chaining).
    store::validate_store_target(&store_path)?;

    // 2. Canonicalize the store path.
    let canonical_store = store_path
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize: {}", store_path.display()))?;

    // 4. Ensure .cue/ does not already exist in cwd.
    let proxy_cue = cwd.join(".cue");
    if proxy_cue.exists() {
        bail!(
            ".cue/ already exists in {}: remove it first to re-link",
            cwd.display()
        );
    }

    // 5. Create the proxy .cue/ directory.
    fs::create_dir_all(&proxy_cue)
        .with_context(|| format!("Failed to create proxy .cue/ at {}", proxy_cue.display()))?;

    // 6. Write STORE file.
    let store_file = proxy_cue.join("STORE");
    fs::write(&store_file, canonical_store.to_str().unwrap_or(""))
        .with_context(|| format!("Failed to write STORE file at {}", store_file.display()))?;

    // 7. If --task given, validate slug and write HEAD.
    if let Some(slug) = task {
        head::validate_slug(&slug)?;
        head::write_head(&proxy_cue, &slug)?;

        // Warn if the task card does not exist in the store.
        let card = canonical_store
            .join("master")
            .join("task")
            .join(format!("{}.md", slug));
        if slug != "master" && !card.exists() {
            eprintln!(
                "warning: no task card found for '{}' at {}",
                slug,
                card.display()
            );
        }
    }

    Ok(())
}
