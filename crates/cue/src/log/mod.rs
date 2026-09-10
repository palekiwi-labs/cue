use crate::git;
use anyhow::{Context, Result, bail};
use cuelib::store;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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

#[derive(Deserialize, Serialize)]
pub struct StoredLogEntry {
    pub timestamp: u64,
    pub commit_hash: String,
    pub title: String,
    pub trace: Option<String>,
    pub found: Vec<String>,
    pub decided: Vec<String>,
    pub open: Vec<String>,
}

pub struct LogAddOptions {
    pub entry: LogEntry,
    pub scope_name: Option<String>,
    pub store_root: Option<PathBuf>,
}

pub fn add_entry(root: &Path, opts: LogAddOptions) -> Result<PathBuf> {
    let LogAddOptions {
        mut entry,
        scope_name,
        store_root,
    } = opts;

    // 1. Validate
    if entry.title.trim().is_empty() {
        bail!("Title cannot be empty.");
    }
    if entry.title.chars().count() > 120 {
        bail!("Title must be 120 characters or fewer.");
    }

    // 2. Gather Git context
    let mut hash =
        git::get_short_head_hash(root).context("Failed to resolve current commit for log entry")?;
    if git::is_working_tree_dirty(root).unwrap_or(false) {
        hash.push_str("-dirty");
    }

    // 3. Resolve the context in the central store.
    let context = cuelib::head::resolve_active_context(root, scope_name.as_deref())?
        .context("No context selected; pass --context <context>")?;
    let store_root = store::root(store_root.as_deref())?;
    let repository_dir = store_root.join(store::repository_scope(root)?);
    let context_dir = repository_dir.join(&context);
    if !context_dir.join("context.md").is_file() {
        bail!("Context does not exist: {context}");
    }

    if let Some(trace) = &entry.trace {
        entry.trace = Some(resolve_trace_reference(
            trace,
            &store_root,
            &repository_dir,
            &context,
        )?);
    }

    // 4. Create one collision-safe JSON file for the entry. Nanosecond
    // timestamps sort chronologically by filename.
    let log_dir = context_dir.join("log");
    fs::create_dir_all(&log_dir)?;
    loop {
        let timestamp = u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos())
            .context("Log timestamp exceeds supported range")?;
        let path = log_dir.join(format!("{timestamp:020}.json"));
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to create log entry in {}", log_dir.display())
                });
            }
        };
        let stored = StoredLogEntry {
            timestamp,
            commit_hash: hash,
            title: entry.title.trim().to_owned(),
            trace: entry.trace,
            found: entry.found,
            decided: entry.decided,
            open: entry.open,
        };
        serde_json::to_writer_pretty(&mut file, &stored)
            .with_context(|| format!("Failed to write to {}", path.display()))?;
        file.write_all(b"\n")
            .with_context(|| format!("Failed to write to {}", path.display()))?;
        return Ok(path);
    }
}

pub fn list_entries(
    root: &Path,
    scope_name: Option<&str>,
    store_root: Option<&Path>,
) -> Result<Vec<StoredLogEntry>> {
    let context = cuelib::head::resolve_active_context(root, scope_name)?
        .context("No context selected; pass --context <context>")?;
    let context_dir = store::root(store_root)?
        .join(store::repository_scope(root)?)
        .join(&context);
    if !context_dir.join("context.md").is_file() {
        bail!("Context does not exist: {context}");
    }

    let log_dir = context_dir.join("log");
    let entries = match fs::read_dir(&log_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Failed to read log directory {}", log_dir.display()));
        }
    };
    let mut paths = entries
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();

    paths
        .into_iter()
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .map(|path| {
            let file = fs::File::open(&path)
                .with_context(|| format!("Failed to read log entry {}", path.display()))?;
            serde_json::from_reader(file)
                .with_context(|| format!("Failed to parse log entry {}", path.display()))
        })
        .collect()
}

pub fn render_markdown(entries: &[StoredLogEntry]) -> String {
    let mut markdown = String::new();

    for entry in entries {
        writeln!(
            &mut markdown,
            "## [{}] {}",
            entry.commit_hash,
            entry.title.trim()
        )
        .unwrap();

        if let Some(trace) = &entry.trace {
            writeln!(&mut markdown, "\n[trace]({})", encode_markdown_path(trace)).unwrap();
        }

        let has_bullets = entry
            .found
            .iter()
            .chain(entry.decided.iter())
            .chain(entry.open.iter())
            .any(|item| !item.trim().is_empty());
        if has_bullets {
            writeln!(&mut markdown).unwrap();
            push_bullets("Found", &entry.found, &mut markdown);
            push_bullets("Decided", &entry.decided, &mut markdown);
            push_bullets("Open", &entry.open, &mut markdown);
        }

        writeln!(&mut markdown).unwrap();
    }

    markdown
}

fn push_bullets(label: &str, items: &[String], markdown: &mut String) {
    for item in items {
        let item = item.trim();
        if !item.is_empty() {
            writeln!(markdown, "- **{label}:** {item}").unwrap();
        }
    }
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

fn resolve_trace_reference(
    trace: &str,
    store_root: &Path,
    repository_dir: &Path,
    context: &str,
) -> Result<String> {
    let trace = trace.trim();
    if trace.is_empty() {
        bail!("Trace reference cannot be empty.");
    }

    let reference = Path::new(trace);
    let candidate = if reference.is_absolute() {
        reference.to_path_buf()
    } else {
        store_root.join(reference)
    };
    let target = fs::canonicalize(&candidate)
        .with_context(|| format!("Trace reference does not exist: {trace}"))?;
    if !target.is_file() {
        bail!("Trace reference must target a file: {trace}");
    }

    let canonical_repository_dir = fs::canonicalize(repository_dir).with_context(|| {
        format!(
            "Failed to resolve cue repository directory {}",
            repository_dir.display()
        )
    })?;
    if !target.starts_with(&canonical_repository_dir) {
        bail!("Trace reference resolves outside this repository's scope: {trace}");
    }

    let trace_root = canonical_repository_dir.join(context).join("trace");
    let canonical_trace_root = fs::canonicalize(&trace_root).with_context(|| {
        format!("Trace reference must target a trace artifact in context '{context}': {trace}")
    })?;
    let relative = target.strip_prefix(&canonical_trace_root).map_err(|_| {
        anyhow::anyhow!(
            "Trace reference must target a trace artifact in context '{context}': {trace}"
        )
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
