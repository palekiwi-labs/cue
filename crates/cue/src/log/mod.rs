use crate::config::Config;
use crate::git;
use anyhow::{Context, Result, bail};
use cuelib::store;
use serde::Deserialize;
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Default)]
pub struct LogEntry {
    pub title: String,
    pub trace: Option<String>,
    #[serde(default)]
    pub found: Vec<String>,
    #[serde(default)]
    pub decided: Vec<String>,
    #[serde(default)]
    pub open: Vec<String>,
}

pub struct LogAddOptions {
    pub entry: LogEntry,
    pub scope_name: Option<String>,
}

pub fn add_entry(root: &Path, config: &Config, opts: LogAddOptions) -> Result<PathBuf> {
    let LogAddOptions {
        mut entry,
        scope_name,
    } = opts;

    // 1. Validate
    if entry.title.trim().is_empty() {
        bail!("Title cannot be empty.");
    }
    if entry.title.chars().count() > 120 {
        bail!("Title must be 120 characters or fewer.");
    }

    // 2. Gather Git context
    let mut hash = git::get_short_head_hash(root).unwrap_or_else(|_| "initial".to_string());
    if git::is_working_tree_dirty(root).unwrap_or(false) {
        hash.push_str("-dirty");
    }

    // 3. Open store
    let repository_root = store::main_worktree_root(root)?;
    let resolved = store::open(root, config)?;

    let scope = cuelib::head::resolve_scope(&resolved.head_dir, scope_name.as_deref())?;
    if scope.trim().is_empty() {
        bail!("Scope name cannot be empty.");
    }

    if let Some(trace) = &entry.trace {
        entry.trace = Some(resolve_trace_reference(
            trace,
            &repository_root,
            &resolved.store_dir,
            &scope,
        )?);
    }

    let log_file_path = resolved.store_dir.join(&scope).join("log.md");

    // 4. Open file and get metadata (to check if it's new) before building markdown
    if let Some(parent) = log_file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
        .with_context(|| format!("Failed to open {}", log_file_path.display()))?;

    let is_new = file.metadata()?.len() == 0;

    // 5. Build Markdown
    let md = build_log_markdown(&entry, &hash, is_new);

    // 6. Append to file
    file.write_all(md.as_bytes())
        .with_context(|| format!("Failed to write to {}", log_file_path.display()))?;

    Ok(log_file_path)
}

fn build_log_markdown(entry: &LogEntry, hash: &str, is_new: bool) -> String {
    let mut md = String::new();

    if is_new {
        md.push_str("# Project Log\n\n");
    }

    writeln!(&mut md, "## [{}] {}", hash, entry.title.trim()).unwrap();

    if let Some(trace) = &entry.trace {
        writeln!(&mut md, "\n[trace]({})", encode_markdown_path(trace)).unwrap();
    }

    let push_bullets = |label: &str, items: &[String], md: &mut String| {
        for item in items {
            let item = item.trim();
            if !item.is_empty() {
                writeln!(md, "- **{}:** {}", label, item).unwrap();
            }
        }
    };

    let has_bullets = entry
        .found
        .iter()
        .chain(entry.decided.iter())
        .chain(entry.open.iter())
        .any(|i| !i.trim().is_empty());

    if has_bullets {
        writeln!(&mut md).unwrap();
        push_bullets("Found", &entry.found, &mut md);
        push_bullets("Decided", &entry.decided, &mut md);
        push_bullets("Open", &entry.open, &mut md);
    }

    writeln!(&mut md).unwrap();

    md
}

fn resolve_trace_reference(
    trace: &str,
    repository_root: &Path,
    store_dir: &Path,
    scope: &str,
) -> Result<String> {
    let trace = trace.trim();
    if trace.is_empty() {
        bail!("Trace reference cannot be empty.");
    }

    let reference = Path::new(trace);
    let candidate = if reference.is_absolute() {
        reference.to_path_buf()
    } else {
        repository_root.join(reference)
    };
    let target = fs::canonicalize(&candidate)
        .with_context(|| format!("Trace reference does not exist: {trace}"))?;
    if !target.is_file() {
        bail!("Trace reference must target a file: {trace}");
    }

    let canonical_store = fs::canonicalize(store_dir).with_context(|| {
        format!(
            "Failed to resolve cue store directory {}",
            store_dir.display()
        )
    })?;
    if !target.starts_with(&canonical_store) {
        bail!("Trace reference resolves outside the cue store: {trace}");
    }

    let trace_root = canonical_store.join(scope).join("trace");
    let canonical_trace_root = fs::canonicalize(&trace_root).with_context(|| {
        format!("Trace reference must target a trace artifact in scope '{scope}': {trace}")
    })?;
    let relative = target.strip_prefix(&canonical_trace_root).map_err(|_| {
        anyhow::anyhow!("Trace reference must target a trace artifact in scope '{scope}': {trace}")
    })?;

    let mut normalized = String::from("trace");
    for component in relative.components() {
        let component = component
            .as_os_str()
            .to_str()
            .context("Trace artifact path must be valid UTF-8")?;
        normalized.push('/');
        normalized.push_str(component);
    }

    Ok(normalized)
}

fn encode_markdown_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'/' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            write!(&mut encoded, "%{byte:02X}").unwrap();
        }
    }
    encoded
}
