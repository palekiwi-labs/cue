//! Global lifecycle hooks: executables run after a successful store write.
//!
//! Hooks live under `<config>/hooks/all.d/` (every event) and
//! `<config>/hooks/<event>.d/` (one event). Both directories are merged and
//! run in lexical order of file name; on equal names the `all.d` hook runs
//! first. Each hook receives a JSON payload on stdin and the dispatch context
//! in `CUE_HOOK_*` variables. Hook failures never fail the write.

use serde::Serialize;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const DEFAULT_MAX_DEPTH: u32 = 4;
const DEFAULT_TIMEOUT_MS: u64 = 3_000;
const DEFAULT_BUDGET_MS: u64 = 10_000;
const STDERR_TAIL_BYTES: usize = 2_048;
const STDERR_DRAIN_MS: u64 = 100;
const IGNORED_SUFFIXES: [&str; 5] = ["~", ".bak", ".swp", ".orig", ".tmp"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    ArtifactCreate,
    ArtifactUpdate,
    ArtifactDelete,
    ContextUpdate,
    ContextCreate,
    LogEntryAdd,
}

impl Event {
    pub fn name(self) -> &'static str {
        match self {
            Event::ArtifactCreate => "artifact_create",
            Event::ArtifactUpdate => "artifact_update",
            Event::ArtifactDelete => "artifact_delete",
            Event::ContextUpdate => "context_update",
            Event::ContextCreate => "context_create",
            Event::LogEntryAdd => "log_entry_add",
        }
    }
}

/// JSON object written to a hook's stdin.
///
/// This is the contract with hook authors: incompatible changes bump `v`,
/// new keys may be added without a bump.
#[derive(Serialize)]
struct Payload<'a> {
    /// Payload schema version, currently 1.
    v: u32,
    /// Event name, e.g. `artifact_create`.
    event: &'a str,
    /// Unique id of this event, also exported as `CUE_HOOK_EVENT_ID`.
    event_id: String,
    /// Canonical store address of what was written.
    address: &'a str,
    /// UTC RFC 3339 time of the write.
    occurred_at: String,
    session_id: Option<String>,
    #[serde(flatten)]
    mutation: MutationPayload<'a>,
}

#[derive(Default, Serialize)]
struct MutationPayload<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    mutation_id: Option<&'a str>,
    #[serde(flatten)]
    change: Option<&'a cuelib::update::Change>,
}

struct Lineage {
    event_id: String,
    parent_event_id: Option<String>,
    root_event_id: String,
    depth: u32,
}

impl Lineage {
    fn from_env() -> Self {
        let event_id = Uuid::new_v4().to_string();
        // Inside a hook, CUE_HOOK_EVENT_ID names the event that ran it, which
        // is the parent of this write; CUE_HOOK_PARENT_EVENT_ID is one level
        // further up and is only informational for hooks.
        let parent_event_id = non_empty_var("CUE_HOOK_EVENT_ID");
        let root_event_id = non_empty_var("CUE_HOOK_ROOT_EVENT_ID")
            .or_else(|| parent_event_id.clone())
            .unwrap_or_else(|| event_id.clone());
        let incoming = non_empty_var("CUE_HOOK_DEPTH")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        Self {
            event_id,
            parent_event_id,
            root_event_id,
            depth: incoming.saturating_add(1),
        }
    }
}

#[derive(Serialize)]
struct LogRecord<'a> {
    at: String,
    event: &'a str,
    event_id: &'a str,
    parent_event_id: Option<&'a str>,
    root_event_id: &'a str,
    depth: u32,
    address: &'a str,
    session_id: Option<String>,
    outcome: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    hook: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stderr_tail: Option<String>,
}

/// Run every hook registered for `event` after a successful write of
/// `address` in the store at `store_root`. Never returns an error.
pub fn dispatch(event: Event, address: &str, store_root: &Path) {
    dispatch_payload(event, address, store_root, MutationPayload::default());
}

pub fn dispatch_update(mutation: &cuelib::update::Mutation, store_root: &Path) {
    let event = if mutation.address.split('/').count() == 3 {
        Event::ContextUpdate
    } else {
        Event::ArtifactUpdate
    };
    for change in &mutation.changes {
        dispatch_payload(
            event,
            &mutation.address,
            store_root,
            MutationPayload {
                mutation_id: Some(&mutation.mutation_id),
                change: Some(change),
            },
        );
    }
}

pub fn dispatch_delete(address: &str, mutation_id: &str, store_root: &Path) {
    dispatch_payload(
        Event::ArtifactDelete,
        address,
        store_root,
        MutationPayload {
            mutation_id: Some(mutation_id),
            change: None,
        },
    );
}

fn dispatch_payload(event: Event, address: &str, store_root: &Path, mutation: MutationPayload<'_>) {
    let lineage = Lineage::from_env();
    let occurred_at = rfc3339_now();
    let Some(config) = hooks_dir() else {
        return;
    };
    let hooks = discover(&config, event);
    if hooks.is_empty() {
        return;
    }

    let record = |outcome: &str,
                  hook: Option<&Path>,
                  exit_code: Option<i32>,
                  duration: Option<Duration>,
                  stderr_tail: Option<String>| {
        append_log(&LogRecord {
            at: rfc3339_now(),
            event: event.name(),
            event_id: &lineage.event_id,
            parent_event_id: lineage.parent_event_id.as_deref(),
            root_event_id: &lineage.root_event_id,
            depth: lineage.depth,
            address,
            session_id: non_empty_var("PI_SESSION_ID"),
            outcome,
            hook: hook.map(|path| path.display().to_string()),
            exit_code,
            duration_ms: duration.map(|duration| duration.as_millis()),
            stderr_tail,
        });
    };

    let max_depth = env_number("CUE_HOOK_MAX_DEPTH", DEFAULT_MAX_DEPTH as u64) as u32;
    if lineage.depth > max_depth {
        eprintln!(
            "Warning: cue hooks not run for {} (depth {} exceeds CUE_HOOK_MAX_DEPTH {max_depth})",
            event.name(),
            lineage.depth
        );
        record("depth_exceeded", None, None, None, None);
        return;
    }

    let payload = serde_json::to_vec(&Payload {
        v: 1,
        event: event.name(),
        event_id: lineage.event_id.clone(),
        address,
        occurred_at,
        session_id: non_empty_var("PI_SESSION_ID"),
        mutation,
    })
    .expect("hook payload serializes");
    let timeout = Duration::from_millis(env_number("CUE_HOOK_TIMEOUT_MS", DEFAULT_TIMEOUT_MS));
    let budget = Duration::from_millis(env_number("CUE_HOOK_BUDGET_MS", DEFAULT_BUDGET_MS));
    let started = Instant::now();

    for hook in hooks {
        let remaining = budget.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            eprintln!(
                "Warning: cue hook budget exhausted; skipped {}",
                hook.display()
            );
            record("budget_exhausted", Some(&hook), None, None, None);
            continue;
        }
        let limit = timeout.min(remaining);
        let mut command = Command::new(&hook);
        command
            .env("CUE_HOOK_EVENT", event.name())
            .env("CUE_HOOK_EVENT_ID", &lineage.event_id)
            .env("CUE_HOOK_ROOT_EVENT_ID", &lineage.root_event_id)
            .env("CUE_HOOK_DEPTH", lineage.depth.to_string())
            .env("CUE_STORE", store_root);
        match &lineage.parent_event_id {
            Some(parent) => command.env("CUE_HOOK_PARENT_EVENT_ID", parent),
            None => command.env_remove("CUE_HOOK_PARENT_EVENT_ID"),
        };
        let own_group = lineage.depth == 1;
        let outcome = run(command, &payload, limit, own_group);
        match outcome {
            Ok(run) => {
                let name = if run.timed_out {
                    "timeout"
                } else if run.exit_code == Some(0) {
                    "ok"
                } else {
                    "failed"
                };
                if name != "ok" {
                    let detail = if run.timed_out {
                        format!("timed out after {} ms", limit.as_millis())
                    } else {
                        match run.exit_code {
                            Some(code) => format!("exited with {code}"),
                            None => "killed by a signal".into(),
                        }
                    };
                    eprintln!("Warning: cue hook {} {detail}", hook.display());
                }
                record(
                    name,
                    Some(&hook),
                    run.exit_code,
                    Some(run.duration),
                    Some(run.stderr_tail).filter(|tail| !tail.is_empty()),
                );
            }
            Err(RunError::Spawn(error)) => {
                eprintln!(
                    "Warning: cue hook {} could not run: {error}",
                    hook.display()
                );
                record(
                    "spawn_failed",
                    Some(&hook),
                    None,
                    None,
                    Some(error.to_string()),
                );
            }
            Err(RunError::Wait(error)) => {
                eprintln!(
                    "Warning: cue hook {} killed after a wait error: {error}",
                    hook.display()
                );
                record(
                    "wait_failed",
                    Some(&hook),
                    None,
                    None,
                    Some(error.to_string()),
                );
            }
        }
    }
}

struct RunResult {
    exit_code: Option<i32>,
    timed_out: bool,
    duration: Duration,
    stderr_tail: String,
}

enum RunError {
    Spawn(std::io::Error),
    Wait(std::io::Error),
}

/// Run one hook with `payload` on stdin, killing it after `limit`.
///
/// With `own_group` the hook leads a new process group and a timeout kills
/// the whole group. Without it (a hook of a write made by another hook) the
/// hook stays in the enclosing hook's group, so the outer timeout reaches it
/// too; its own timeout then kills only the hook process.
fn run(
    mut command: Command,
    payload: &[u8],
    limit: Duration,
    own_group: bool,
) -> Result<RunResult, RunError> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if own_group {
        command.process_group(0);
    }
    let started = Instant::now();
    let mut child = command.spawn().map_err(RunError::Spawn)?;

    if let Some(mut stdin) = child.stdin.take() {
        let payload = payload.to_vec();
        std::thread::spawn(move || {
            let _ = stdin.write_all(&payload);
        });
    }
    let tail = Arc::new(Mutex::new(Vec::new()));
    let (done_tx, done_rx) = mpsc::channel::<()>();
    if let Some(mut stderr) = child.stderr.take() {
        let tail = Arc::clone(&tail);
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(read) = stderr.read(&mut buf) {
                if read == 0 {
                    break;
                }
                let mut tail = tail.lock().unwrap_or_else(|poison| poison.into_inner());
                tail.extend_from_slice(&buf[..read]);
                if tail.len() > STDERR_TAIL_BYTES {
                    let excess = tail.len() - STDERR_TAIL_BYTES;
                    tail.drain(..excess);
                }
            }
            let _ = done_tx.send(());
        });
    }

    let pid = child.id() as i32;
    let kill = || {
        let target = if own_group { -pid } else { pid };
        // SAFETY: kill(2) on the hook process, or on the process group it
        // leads when spawned with process_group(0); no memory is touched.
        unsafe {
            libc::kill(target, libc::SIGKILL);
        }
    };
    let (status, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (status, false),
            Ok(None) => {}
            Err(error) => {
                kill();
                let _ = child.wait();
                return Err(RunError::Wait(error));
            }
        }
        if started.elapsed() >= limit {
            kill();
            match child.wait() {
                Ok(status) => break (status, true),
                Err(error) => return Err(RunError::Wait(error)),
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let duration = started.elapsed();
    let _ = done_rx.recv_timeout(Duration::from_millis(STDERR_DRAIN_MS));
    let tail = tail
        .lock()
        .map(|tail| tail.clone())
        .unwrap_or_else(|poison| poison.into_inner().clone());

    Ok(RunResult {
        exit_code: status.code(),
        timed_out,
        duration,
        stderr_tail: String::from_utf8_lossy(&tail).trim_end().to_string(),
    })
}

fn discover(hooks_dir: &Path, event: Event) -> Vec<PathBuf> {
    let mut hooks: Vec<(Vec<u8>, u8, PathBuf)> = Vec::new();
    for (rank, dir) in [
        (0u8, "all.d".to_string()),
        (1, format!("{}.d", event.name())),
    ] {
        let Ok(entries) = fs::read_dir(hooks_dir.join(dir)) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            if !eligible_name(&name) {
                continue;
            }
            let path = entry.path();
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };
            if metadata.is_file() && metadata.permissions().mode() & 0o111 != 0 {
                hooks.push((name.as_encoded_bytes().to_vec(), rank, path));
            }
        }
    }
    hooks.sort();
    hooks.into_iter().map(|(_, _, path)| path).collect()
}

fn eligible_name(name: &OsString) -> bool {
    let name = name.to_string_lossy();
    !name.starts_with('.') && !IGNORED_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

fn hooks_dir() -> Option<PathBuf> {
    let config = match std::env::var_os("CUE_CONFIG_DIR") {
        Some(path) => PathBuf::from(path),
        None => dirs::home_dir()?.join(".config/cue"),
    };
    Some(config.join("hooks"))
}

fn log_path() -> Option<PathBuf> {
    let state = match non_empty_var("XDG_STATE_HOME") {
        Some(path) => PathBuf::from(path),
        None => dirs::home_dir()?.join(".local/state"),
    };
    Some(state.join("cue/hooks.jsonl"))
}

fn append_log(record: &LogRecord<'_>) {
    let Some(path) = log_path() else {
        return;
    };
    let write = || -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut line = serde_json::to_vec(record)?;
        line.push(b'\n');
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?
            .write_all(&line)
    };
    if let Err(error) = write() {
        eprintln!("Warning: cue hook dispatch log not written: {error}");
    }
}

fn non_empty_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn env_number(name: &str, default: u64) -> u64 {
    non_empty_var(name)
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// Canonical address of `path` relative to `store_root`, using `/`.
pub fn address_of(path: &Path, store_root: &Path) -> Option<String> {
    let relative = path.strip_prefix(store_root).ok()?;
    Some(
        relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn rfc3339_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs() as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:09}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60,
        now.subsec_nanos()
    )
}

/// Proleptic Gregorian (year, month, day) for `days` since 1970-01-01.
///
/// Howard Hinnant's `civil_from_days` algorithm
/// (<https://howardhinnant.github.io/date_algorithms.html>): 719 468 shifts
/// the epoch to 0000-03-01, 146 097 is days per 400-year era, and months are
/// counted from March so the leap day falls at the end of the year.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }

    #[test]
    fn ignores_dotfiles_and_backups() {
        for name in [".hidden", "a~", "a.bak", "a.swp", "a.orig", "a.tmp"] {
            assert!(!eligible_name(&OsString::from(name)), "{name}");
        }
        assert!(eligible_name(&OsString::from("10-publish")));
    }
}
