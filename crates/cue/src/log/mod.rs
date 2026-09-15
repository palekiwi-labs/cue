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
    pub title: String,
    pub trace: Option<String>,
    pub found: Vec<String>,
    pub decided: Vec<String>,
    pub open: Vec<String>,
}

pub struct LogAddOptions {
    pub entry: LogEntry,
    pub context: Option<String>,
    pub store_root: Option<PathBuf>,
}

pub fn add_entry(root: &Path, opts: LogAddOptions) -> Result<PathBuf> {
    let LogAddOptions {
        mut entry,
        context,
        store_root,
    } = opts;

    // 1. Validate
    if entry.title.trim().is_empty() {
        bail!("Title cannot be empty.");
    }
    if entry.title.chars().count() > 120 {
        bail!("Title must be 120 characters or fewer.");
    }

    // 2. Resolve the context in the central store. A log entry is a memory
    // and communication event, not a revision-correlated artifact, so no
    // repository revision is read or stamped here.
    let context = cuelib::head::resolve_active_context(root, context.as_deref())?
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

    // 3. Create one collision-safe JSON file for the entry. Nanosecond
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
    context: Option<&str>,
    store_root: Option<&Path>,
) -> Result<Vec<StoredLogEntry>> {
    let context = cuelib::head::resolve_active_context(root, context)?
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

/// The newest entry timestamp in a context's log, or `None` when the log
/// holds no entry.
///
/// Read from entry filenames rather than entry contents: an entry is named
/// for the timestamp it records, so the newest is known from one directory
/// listing without opening a file. The scan visits every directory entry,
/// but its cost does not depend on log body sizes. Corrupt JSON contents do
/// not affect ordering.
///
/// Anything that is not an entry is not activity: a name that is not exactly
/// a stamp is ignored, as is a directory wearing an entry's name, and a
/// missing log directory is a log with no entries. Any other failure to read
/// the directory is reported, so a log that cannot be listed is never
/// mistaken for a context with nothing in it.
pub fn latest_timestamp(context_dir: &Path) -> Result<Option<u64>> {
    let log_dir = context_dir.join("log");
    let entries = match fs::read_dir(&log_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Failed to read log directory {}", log_dir.display()));
        }
    };

    let mut latest = None;
    for entry in entries {
        let entry =
            entry.with_context(|| format!("Failed to read log directory {}", log_dir.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("Failed to read log entry {}", entry.path().display()))?;
        if !file_type.is_file() {
            continue;
        }
        if let Some(timestamp) = entry_timestamp(&entry.file_name()) {
            latest = latest.max(Some(timestamp));
        }
    }

    Ok(latest)
}

/// The timestamp a log entry's filename records, or `None` when the name is
/// not one an entry is written under.
///
/// `add_entry` writes `{timestamp:020}.json`, so only that exact shape is an
/// entry. Twenty digits is the width of `u64::MAX`, and a name that is wider,
/// shorter, or not all ASCII digits is something else that happens to share
/// the directory.
fn entry_timestamp(file_name: &std::ffi::OsStr) -> Option<u64> {
    let stamp = file_name.to_str()?.strip_suffix(".json")?;
    if stamp.len() != 20 || !stamp.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    stamp.parse().ok()
}

pub fn render_markdown(entries: &[StoredLogEntry]) -> String {
    let mut markdown = String::new();

    for entry in entries {
        writeln!(&mut markdown, "## {}", entry.title.trim()).unwrap();

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

/// Verify that `trace` is a canonical address naming an existing trace
/// artifact in this context, and return it unchanged.
///
/// The returned value is the address itself, never a path and never a
/// shortened tail: a log entry is an artifact, so the reference it records is
/// subject to the same canonical-address rule as any other.
fn resolve_trace_reference(
    trace: &str,
    store_root: &Path,
    repository_dir: &Path,
    context: &str,
) -> Result<String> {
    let trace = trace.trim();
    crate::address::validate_reference("--trace", trace, store_root)?;

    let candidate = store_root.join(trace);
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
    if !target.starts_with(&canonical_trace_root) {
        bail!("Trace reference must target a trace artifact in context '{context}': {trace}");
    }

    Ok(trace.to_string())
}
