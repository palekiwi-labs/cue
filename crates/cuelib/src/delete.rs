//! Removal of one artifact, refused while other artifacts link to it.

use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use std::path::Path;

use crate::artifact::{collect_files, extract_frontmatter_yaml};
use crate::update::{Address, lock};

const TYPES: &[&str] = &[
    "task", "plan", "note", "spec", "trace", "review", "bin", "tmp",
];

/// The outcome of a delete.
#[derive(Debug, Serialize)]
pub struct Deletion {
    pub mutation_id: String,
    pub address: String,
    /// Log entries whose `trace` names the deleted artifact.
    pub warnings: Vec<String>,
}

/// A link from another artifact or context to the target.
#[derive(Debug, PartialEq, Eq)]
pub struct Link {
    pub from: String,
    pub field: &'static str,
}

/// Delete the artifact at `address` from the store at `store`.
///
/// Refused when the address is not an artifact file, or while any context
/// or artifact in the store names it as `parent` or lists it in `refs`.
/// Log entries naming it as `trace` are reported as warnings. With
/// `dry_run` nothing is removed.
pub fn delete(store: &Path, address: &str, dry_run: bool) -> Result<Deletion> {
    let address = Address::parse(address)?;
    match address.artifact_type() {
        None => bail!("contexts cannot be deleted; use cue context archive"),
        Some("log") => bail!("log entries cannot be deleted"),
        Some(kind) if !TYPES.contains(&kind) => bail!("unknown artifact type '{kind}'"),
        Some(_) => {}
    }
    let _lock = if dry_run { None } else { Some(lock(store)?) };
    let path = address.path(store);
    if path.is_dir() {
        bail!("not a file: {}", address.as_str());
    }
    if !path.is_file() {
        bail!(
            "no {} at {}",
            address.artifact_type().unwrap_or_default(),
            address.as_str()
        );
    }

    let (links, warnings) = inbound(store, address.as_str())?;
    if !links.is_empty() {
        let lines: Vec<String> = links
            .iter()
            .map(|link| format!("  {} ({})", link.from, link.field))
            .collect();
        bail!("{} is linked from:\n{}", address.as_str(), lines.join("\n"));
    }

    if !dry_run {
        std::fs::remove_file(&path)
            .with_context(|| format!("could not delete {}", path.display()))?;
        if address.artifact_type() == Some("tmp")
            && let Some(dir) = path.parent()
        {
            let _ = std::fs::remove_dir(dir);
        }
    }
    Ok(Deletion {
        mutation_id: uuid::Uuid::new_v4().to_string(),
        address: address.as_str().to_string(),
        warnings,
    })
}

fn inbound(store: &Path, target: &str) -> Result<(Vec<Link>, Vec<String>)> {
    let mut links = Vec::new();
    let mut warnings = Vec::new();
    let mut files = Vec::new();
    for entry in std::fs::read_dir(store)? {
        let path = entry?.path();
        let hidden = path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with('.'));
        if path.is_dir() && !hidden {
            files.extend(collect_files(&path)?);
        }
    }
    files.sort();
    for file in files {
        let Ok(relative) = file.strip_prefix(store) else {
            continue;
        };
        let from = relative.to_string_lossy().to_string();
        if from == target {
            continue;
        }
        let segments: Vec<&str> = from.split('/').collect();
        let kind = segments.get(3).copied();
        let json = from.ends_with(".json");
        match kind {
            Some("log") if json => {
                if read_json(&file).and_then(|value| value.get("trace").cloned())
                    == Some(serde_json::Value::from(target))
                {
                    warnings.push(from);
                }
            }
            Some("review") if json => {
                if let Some(value) = read_json(&file) {
                    let refs = value
                        .get("refs")
                        .and_then(|refs| refs.as_array())
                        .is_some_and(|refs| refs.iter().any(|item| item == target));
                    push(
                        &mut links,
                        &from,
                        value.get("parent") == Some(&target.into()),
                        refs,
                    );
                }
            }
            Some("bin" | "tmp") => {}
            _ if from.ends_with(".md") => {
                let mapping = extract_frontmatter_yaml(&file)
                    .and_then(|yaml| serde_yaml::from_str::<serde_yaml::Mapping>(&yaml).ok());
                if let Some(mapping) = mapping {
                    let parent = mapping.get("parent").and_then(|v| v.as_str()) == Some(target);
                    let refs = match mapping.get("refs") {
                        Some(serde_yaml::Value::Sequence(items)) => {
                            items.iter().any(|item| item.as_str() == Some(target))
                        }
                        Some(serde_yaml::Value::String(item)) => item == target,
                        _ => false,
                    };
                    let from = from.strip_suffix("/context.md").unwrap_or(&from);
                    push(&mut links, from, parent, refs);
                }
            }
            _ => {}
        }
    }
    Ok((links, warnings))
}

fn push(links: &mut Vec<Link>, from: &str, parent: bool, refs: bool) {
    if parent {
        links.push(Link {
            from: from.to_string(),
            field: "parent",
        });
    }
    if refs {
        links.push(Link {
            from: from.to_string(),
            field: "refs",
        });
    }
}

fn read_json(path: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}
