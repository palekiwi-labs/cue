//! Liveness, discovery, observation and stop requests for running batches.
//!
//! The supervisor holds an exclusive `flock` on `<batch>/lock` for as long as
//! it runs. The kernel drops the lock when the process dies however it dies,
//! so "is this batch running" never rests on a process id that may have been
//! reused. `active/<batch-id>` under the agent root is only a hint for finding
//! batches without scanning; the lock is the truth.
//!
//! A stop request is a file the supervisor polls for. The requester never
//! signals a harness: the supervisor stays the only party that signals or
//! reaps one, so a stopped run gets ordinary teardown, capture and cleanup.
//!
//! Locks need a local filesystem for the state directory; `flock` over NFS
//! is not reliable.

use crate::harness;
use crate::run_spec::Job;
use crate::state;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const LOCK: &str = "lock";
const RECORD: &str = "batch.json";
const FINALIZING: &str = "finalizing";
/// Written by the supervisor when a run's harness is reaped.
pub const EXIT: &str = "exit.json";
const BATCH_STOP: &str = "stop";
/// Bytes of a tool call's arguments shown in a status.
const ARGUMENTS_EXCERPT: usize = 400;
const FOLLOW_INTERVAL: Duration = Duration::from_millis(200);

/// Per-run stop request, kept in the batch directory so it can be left
/// before the run's own directory exists.
fn run_stop(batch_path: &Path, run_id: &str) -> PathBuf {
    batch_path.join(format!("stop-{run_id}"))
}

/// The stop requests that end one run.
pub fn stop_paths(batch_path: &Path, run_id: &str) -> Vec<PathBuf> {
    vec![batch_path.join(BATCH_STOP), run_stop(batch_path, run_id)]
}

pub fn stop_requested(batch_path: &Path, run_id: &str) -> bool {
    stop_paths(batch_path, run_id)
        .iter()
        .any(|path| path.exists())
}

fn active_dir(agent_root: &Path) -> PathBuf {
    agent_root.join("active")
}

/// The supervisor's side: held for the lifetime of the batch.
pub struct Liveness {
    /// Never read; holding the descriptor holds the lock. Opened with
    /// `O_CLOEXEC`, so no harness or helper inherits it.
    _lock: File,
    /// The discovery entry, when it could be created.
    active: Option<PathBuf>,
    batch_path: PathBuf,
}

impl Liveness {
    /// Lock, describe and register a batch whose directory exists.
    ///
    /// The lock is taken on a private name and renamed into place, so a
    /// reader never sees `lock` unlocked while the batch is alive.
    pub fn acquire(agent_root: &Path, batch_path: &Path, record: &Value) -> Result<Self> {
        let pending = batch_path.join(format!(".lock.{}", std::process::id()));
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&pending)
            .with_context(|| format!("could not create {}", pending.display()))?;
        // SAFETY: flock on a descriptor we own.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            bail!(
                "could not lock {}: {}",
                pending.display(),
                std::io::Error::last_os_error()
            );
        }
        fs::rename(&pending, batch_path.join(LOCK))?;

        let temporary = batch_path.join(format!(".{RECORD}.tmp"));
        state::write_private(&temporary, serde_json::to_vec_pretty(record)?)?;
        fs::rename(&temporary, batch_path.join(RECORD))?;

        let batch_id = record["batch_id"].as_str().unwrap_or_default();
        // Past this point the batch is locked and described, so it is
        // reliably alive to `status` and `stop`; failing to register it only
        // hides it from `ps`, and must not release the lock.
        let entry = active_dir(agent_root).join(batch_id);
        let registered = state::private_dir(&active_dir(agent_root))
            .and_then(|()| std::os::unix::fs::symlink(batch_path, &entry));
        let active = match registered {
            Ok(()) => Some(entry),
            Err(err) => {
                eprintln!(
                    "cue-agent: `ps` will not list this batch: could not create {}: {err}",
                    entry.display()
                );
                None
            }
        };
        Ok(Self {
            _lock: lock,
            active,
            batch_path: batch_path.to_path_buf(),
        })
    }

    /// Supervision is over; results are being captured and cleaned up.
    pub fn finalizing(&self) {
        let _ = state::write_private(self.batch_path.join(FINALIZING), "");
        if let Some(active) = &self.active {
            let _ = fs::remove_file(active);
        }
    }
}

impl Drop for Liveness {
    fn drop(&mut self) {
        // The lock file stays: unlinking it would let a late reader lock a
        // fresh inode and miss a holder. Dropping the descriptor frees it.
        if let Some(active) = &self.active {
            let _ = fs::remove_file(active);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchState {
    Running,
    Finalizing,
    Finished,
    Crashed,
}

impl BatchState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Finalizing => "finalizing",
            Self::Finished => "finished",
            Self::Crashed => "crashed",
        }
    }

    fn alive(self) -> bool {
        matches!(self, Self::Running | Self::Finalizing)
    }
}

/// Whether some process holds the batch lock. A missing lock file is a batch
/// that never ran under monitoring, which is not alive either.
fn lock_held(batch_path: &Path) -> bool {
    let Ok(file) = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(batch_path.join(LOCK))
    else {
        return false;
    };
    // SAFETY: flock on a descriptor we own; closing it releases the probe.
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB) };
    result != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EWOULDBLOCK)
}

/// A batch's monitoring record and where it lives.
struct Batch {
    path: PathBuf,
    record: Value,
}

impl Batch {
    fn open(path: PathBuf) -> Option<Self> {
        let bytes = fs::read(path.join(RECORD)).ok()?;
        let record = serde_json::from_slice(&bytes).ok()?;
        Some(Self { path, record })
    }

    fn id(&self) -> &str {
        self.record["batch_id"].as_str().unwrap_or_default()
    }

    fn runs(&self) -> &[Value] {
        self.record["runs"].as_array().map_or(&[], Vec::as_slice)
    }

    fn run_path(&self, run: &Value) -> PathBuf {
        self.path.join(run["dir"].as_str().unwrap_or_default())
    }

    fn state(&self) -> BatchState {
        if lock_held(&self.path) {
            if self.path.join(FINALIZING).exists() {
                BatchState::Finalizing
            } else {
                BatchState::Running
            }
        } else if self
            .runs()
            .iter()
            .all(|run| self.run_path(run).join("receipt.json").exists())
        {
            BatchState::Finished
        } else {
            BatchState::Crashed
        }
    }

    fn report(&self) -> Value {
        let state = self.state();
        let runs: Vec<Value> = self
            .runs()
            .iter()
            .map(|run| self.run_report(run, state))
            .collect();
        json!({
            "batch_id": self.id(),
            "label": self.record["label"],
            "job": self.record["job"],
            "state": state.as_str(),
            "pid": self.record["pid"],
            "started_at": self.record["started_at"],
            "batch_path": self.path,
            "runs": runs,
        })
    }

    fn run_report(&self, run: &Value, state: BatchState) -> Value {
        let path = self.run_path(run);
        let events = path.join("events.jsonl");
        let receipt: Option<Value> = fs::read(path.join("receipt.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        let launch_error = fs::read(path.join("manifest.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .is_some_and(|manifest| !manifest["launch_error"].is_null());
        let exit: Option<Value> = fs::read(path.join(EXIT))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        let exited_at = exit.as_ref().and_then(|_| modified(&path.join(EXIT)));
        let phase = if receipt.is_some() {
            "finished"
        } else if state == BatchState::Crashed {
            "crashed"
        } else if state == BatchState::Finalizing || launch_error {
            "finishing"
        } else if exit.is_some() {
            "exited"
        } else if events.exists() {
            "running"
        } else {
            "preparing"
        };

        let pgid: Option<i32> = fs::read_to_string(path.join("harness.pgid"))
            .ok()
            .and_then(|text| text.trim().parse().ok());
        let launched_at = modified(&path.join("harness.pgid"));
        // An ended run's clock stops at its exit, so it never looks stalled.
        let now = exited_at.unwrap_or_else(SystemTime::now);
        let elapsed = |since: Option<SystemTime>| {
            since.and_then(|at| now.duration_since(at).ok().map(|d| d.as_secs()))
        };

        let mut report = json!({
            "run_id": run["run_id"],
            "agent": run["agent"],
            "model": run["model"],
            "position": run["position"],
            "label": run["label"],
            "context": run["context"],
            "phase": phase,
            "run_path": path,
            "harness_pgid": pgid,
        });
        if let Some(receipt) = receipt {
            report["elapsed_secs"] = json!(receipt["duration_ms"].as_u64().map(|ms| ms / 1000));
            report["result"] = json!({
                "outcome": receipt["outcome"],
                "error": receipt["error"],
                "response": receipt["response"],
                "trace": receipt["trace"],
                "trace_error": receipt["trace_error"],
            });
        } else if let Some(exit) = &exit {
            report["elapsed_secs"] = json!(exit["duration_ms"].as_u64().map(|ms| ms / 1000));
            report["exit"] = json!({
                "reason": exit["reason"],
                "code": exit["code"],
                "signal": exit["signal"],
            });
        } else {
            report["elapsed_secs"] = json!(elapsed(launched_at));
        }
        if events.exists() {
            let observed = harness::capture(&events);
            report["idle_secs"] = json!(elapsed(modified(&events)));
            report["turns"] = json!(observed.turns);
            report["tokens_input"] = json!(observed.tokens_input);
            report["tokens_cache_read"] = json!(observed.cache_read);
            report["tokens_cache_write"] = json!(observed.cache_write);
            report["tokens_output"] = json!(observed.tokens_output);
            report["cost_usd"] = json!(observed.cost_usd);
            report["last_tool"] = match observed.last_tool {
                Some(tool) => json!({
                    "name": tool.name,
                    "arguments": excerpt(&tool.arguments, ARGUMENTS_EXCERPT),
                    // A tool cannot still be running once its harness exited.
                    "pending": tool.pending && exit.is_none(),
                }),
                None => Value::Null,
            };
        }
        if state == BatchState::Crashed {
            // The group id may since have been reused; this is a hint that
            // something may survive, not proof of what it is.
            report["harness_group_alive"] =
                json!(pgid.is_some_and(|pgid| unsafe { libc::kill(-pgid, 0) } == 0));
        }
        report
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|meta| meta.modified()).ok()
}

fn excerpt(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
}

/// What a run or batch identifier names.
enum Target {
    Batch(Batch),
    Run(Batch, Value),
}

/// Length of `<yyyymmdd>-<hhmmss>-<6 hex>`.
const BATCH_ID_LEN: usize = 22;

/// The batch directory is derived from the date inside the identifier, so no
/// scan is needed. A run identifier starts with its batch's.
fn resolve(agent_root: &Path, id: &str) -> Result<Target> {
    let unknown = || anyhow::anyhow!("no batch or run '{id}'");
    if id.len() < BATCH_ID_LEN || !id.is_char_boundary(BATCH_ID_LEN) {
        return Err(unknown());
    }
    let batch_id = &id[..BATCH_ID_LEN];
    let date = batch_id.get(..8).ok_or_else(unknown)?;
    if !date.bytes().all(|byte| byte.is_ascii_digit()) || !crate::ids::is_valid(batch_id) {
        return Err(unknown());
    }
    let dir = format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..8]);
    let path = agent_root.join("runs").join(dir).join(batch_id);
    let batch = Batch::open(path).ok_or_else(unknown)?;
    if id == batch_id {
        return Ok(Target::Batch(batch));
    }
    let run = batch
        .runs()
        .iter()
        .find(|run| run["run_id"] == id)
        .cloned()
        .ok_or_else(unknown)?;
    Ok(Target::Run(batch, run))
}

/// Which batches `ps` reports.
#[derive(Debug, Default)]
pub struct Filter {
    /// Include batches without an active entry.
    pub all: bool,
    /// Unix seconds; batches started earlier are skipped.
    pub since: Option<i64>,
    /// At most this many, newest first.
    pub limit: Option<usize>,
    pub job: Option<String>,
}

impl Filter {
    fn matches(&self, batch: &Batch) -> bool {
        self.job
            .as_ref()
            .is_none_or(|job| batch.record["job"]["id"].as_str() == Some(job))
    }
}

/// The last second whose date has four year digits (9999-12-31T23:59:59Z).
/// Date names and batch ids compare as text, so later bounds would sort
/// below them; nothing can start that late anyway.
const LAST_FOUR_DIGIT_YEAR: i64 = 253_402_300_799;

/// The date directory name and batch-id prefix of a moment; both sort by
/// time, so older entries compare below them.
fn since_bounds(since: i64) -> (String, String) {
    let (y, m, d, hh, mm, ss) = state::civil_from_unix(since);
    (
        format!("{y:04}-{m:02}-{d:02}"),
        format!("{y:04}{m:02}{d:02}-{hh:02}{mm:02}{ss:02}"),
    )
}

/// Parse `--since`: Unix seconds, or a duration back from now such as
/// `90s`, `30m`, `2h` or `7d`.
pub fn parse_since(text: &str, now: i64) -> Result<i64, String> {
    if let Ok(secs) = text.parse::<i64>() {
        return Ok(secs);
    }
    let invalid = || format!("'{text}' is neither Unix seconds nor a duration like 30m, 2h or 7d");
    let Some((last, _)) = text.char_indices().next_back() else {
        return Err(invalid());
    };
    let (number, unit) = text.split_at(last);
    let scale = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86400,
        _ => return Err(invalid()),
    };
    let count: i64 = number.parse().map_err(|_| invalid())?;
    count
        .checked_mul(scale)
        .filter(|secs| *secs >= 0)
        .map(|secs| now - secs)
        .ok_or_else(invalid)
}

/// Batches with an active entry, newest first. Entries whose lock is free
/// belong to supervisors that died while running: they are reported this
/// once, then removed. With `all`, the history is scanned newest first and
/// the scan stops at the `since` bound or once `limit` batches are found.
pub fn ps(filter: &Filter) -> Result<Value> {
    let agent_root = state::agent_root()?;
    if filter
        .since
        .is_some_and(|since| since > LAST_FOUR_DIGIT_YEAR)
    {
        return Ok(json!({ "batches": [] }));
    }
    let bounds = filter.since.map(since_bounds);
    let mut batches: Vec<Batch> = Vec::new();
    // Active entries of dead batches, removed only once reported.
    let mut dead_entries: Vec<(String, PathBuf)> = Vec::new();
    if filter.all {
        'days: for day in sorted_dirs(&agent_root.join("runs")).into_iter().rev() {
            let name = day.file_name().unwrap_or_default().to_string_lossy();
            if bounds.as_ref().is_some_and(|(date, _)| *name < **date) {
                break;
            }
            for path in sorted_dirs(&day).into_iter().rev() {
                let id = path.file_name().unwrap_or_default().to_string_lossy();
                if bounds.as_ref().is_some_and(|(_, prefix)| *id < **prefix) {
                    break 'days;
                }
                if filter.limit.is_some_and(|limit| batches.len() >= limit) {
                    break 'days;
                }
                if let Some(batch) = Batch::open(path)
                    && filter.matches(&batch)
                {
                    batches.push(batch);
                }
            }
        }
    } else {
        let active = active_dir(&agent_root);
        for entry in sorted_dirs(&active) {
            let Ok(target) = fs::read_link(&entry) else {
                continue;
            };
            let Some(batch) = Batch::open(target) else {
                continue;
            };
            let recent = bounds
                .as_ref()
                .is_none_or(|(_, prefix)| batch.id() >= prefix.as_str());
            if recent && filter.matches(&batch) {
                if !batch.state().alive() {
                    dead_entries.push((batch.id().to_string(), entry));
                }
                batches.push(batch);
            }
        }
    }
    batches.sort_by(|a, b| b.id().cmp(a.id()));
    if let Some(limit) = filter.limit {
        batches.truncate(limit);
    }
    for (id, entry) in dead_entries {
        if batches.iter().any(|batch| batch.id() == id) {
            let _ = fs::remove_file(&entry);
        }
    }
    let reports: Vec<Value> = batches.iter().map(Batch::report).collect();
    Ok(json!({ "batches": reports }))
}

fn sorted_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|entry| Some(entry.ok()?.path()))
                .collect()
        })
        .unwrap_or_default();
    paths.sort();
    paths
}

pub fn status(id: &str) -> Result<Value> {
    let agent_root = state::agent_root()?;
    Ok(match resolve(&agent_root, id)? {
        Target::Batch(batch) => batch.report(),
        Target::Run(batch, run) => {
            let state = batch.state();
            let mut report = batch.run_report(&run, state);
            report["batch_id"] = json!(batch.id());
            report["batch_state"] = json!(state.as_str());
            report
        }
    })
}

/// Leave a stop request for a run or a batch.
pub fn stop(id: &str, wait: bool) -> Result<Value> {
    let agent_root = state::agent_root()?;
    let target = resolve(&agent_root, id)?;
    let (batch, run) = match &target {
        Target::Batch(batch) => (batch, None),
        Target::Run(batch, run) => (batch, Some(run)),
    };
    let state = batch.state();
    let finished = match run {
        Some(run) => batch.run_path(run).join("receipt.json").exists(),
        None => false,
    };
    let result = if finished {
        "already_finished"
    } else if !state.alive() {
        "not_running"
    } else {
        let path = match run {
            Some(_) => run_stop(&batch.path, id),
            None => batch.path.join(BATCH_STOP),
        };
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
        {
            Ok(_) => "requested",
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => "already_requested",
            Err(err) => {
                return Err(err).with_context(|| format!("could not create {}", path.display()));
            }
        }
    };

    let mut report = json!({
        "target": id,
        "kind": if run.is_some() { "run" } else { "batch" },
        "result": result,
        "batch_state": state.as_str(),
    });
    if wait && matches!(result, "requested" | "already_requested") {
        let done = || match run {
            Some(run) => {
                batch.run_path(run).join("receipt.json").exists() || !batch.state().alive()
            }
            None => !batch.state().alive(),
        };
        while !done() {
            std::thread::sleep(FOLLOW_INTERVAL);
        }
        report["status"] = status(id)?;
    }
    Ok(report)
}

/// Copy a run's raw events or stderr to stdout, optionally following it
/// until the run's result exists or its supervisor is gone.
pub fn logs(id: &str, stderr: bool, follow: bool) -> Result<()> {
    let agent_root = state::agent_root()?;
    let Target::Run(batch, run) = resolve(&agent_root, id)? else {
        bail!("'{id}' is a batch; logs takes a run identifier");
    };
    let run_path = batch.run_path(&run);
    let path = run_path.join(if stderr { "stderr.log" } else { "events.jsonl" });
    let mut out = std::io::stdout().lock();
    let mut offset = 0u64;
    loop {
        // Decide before reading, so the last read sees everything written.
        let done = !follow || run_path.join("receipt.json").exists() || !batch.state().alive();
        if let Ok(mut file) = File::open(&path) {
            file.seek(SeekFrom::Start(offset))?;
            let mut buffer = Vec::new();
            offset += file.read_to_end(&mut buffer)? as u64;
            out.write_all(&buffer)?;
            out.flush()?;
        }
        if done {
            return Ok(());
        }
        std::thread::sleep(FOLLOW_INTERVAL);
    }
}

/// The record a supervisor writes beside its lock.
pub fn record(
    batch_id: &str,
    label: Option<&str>,
    job: Option<&Job>,
    started_at: i64,
    runs: Vec<Value>,
) -> Value {
    json!({
        "batch_id": batch_id,
        "label": label,
        "job": job,
        "pid": std::process::id(),
        "started_at": started_at,
        "runs": runs,
    })
}

/// One-line human rendering of a run report.
pub fn human_run(run: &Value) -> String {
    let mut line = format!(
        "  {}  {}  {}",
        run["run_id"].as_str().unwrap_or_default(),
        run["agent"].as_str().unwrap_or_default(),
        run["phase"].as_str().unwrap_or_default(),
    );
    if let Some(secs) = run["elapsed_secs"].as_u64() {
        line.push_str(&format!("  {secs}s"));
    }
    if let Some(idle) = run["idle_secs"].as_u64() {
        line.push_str(&format!("  idle {idle}s"));
    }
    if let Some(turns) = run["turns"].as_u64() {
        line.push_str(&format!("  turns {turns}"));
    }
    if let Some(name) = run["last_tool"]["name"].as_str() {
        let pending = if run["last_tool"]["pending"] == true {
            " (pending)"
        } else {
            ""
        };
        line.push_str(&format!("  tool {name}{pending}"));
    }
    if let Some(outcome) = run["result"]["outcome"].as_str() {
        line.push_str(&format!("  {outcome}"));
    } else if let Some(reason) = run["exit"]["reason"].as_str() {
        // The phase already says exited; repeat only a different reason.
        if reason != "exited" {
            line.push_str(&format!("  {reason}"));
        }
        if let Some(code) = run["exit"]["code"].as_i64() {
            line.push_str(&format!("  code {code}"));
        }
        if let Some(signal) = run["exit"]["signal"].as_i64() {
            line.push_str(&format!("  signal {signal}"));
        }
    }
    line.push('\n');
    line
}

pub fn human_batch(batch: &Value) -> String {
    let mut text = format!(
        "{}  {}  pid {}\n",
        batch["batch_id"].as_str().unwrap_or_default(),
        batch["state"].as_str().unwrap_or_default(),
        batch["pid"],
    );
    for run in batch["runs"].as_array().into_iter().flatten() {
        text.push_str(&human_run(run));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exited(reason: &str, code: Value, signal: Value) -> String {
        human_run(&json!({
            "run_id": "r", "agent": "a", "phase": "exited",
            "exit": { "reason": reason, "code": code, "signal": signal },
        }))
    }

    #[test]
    fn parse_since_accepts_seconds_and_durations() {
        assert_eq!(parse_since("1700000000", 0), Ok(1700000000));
        assert_eq!(parse_since("90s", 1000), Ok(910));
        assert_eq!(parse_since("2h", 10000), Ok(2800));
        assert_eq!(parse_since("1d", 100000), Ok(13600));
    }

    #[test]
    fn parse_since_rejects_malformed_text_without_panicking() {
        for text in ["", "h", "-5m", "5x", "\u{e9}", "5\u{b5}", "\u{1f600}"] {
            assert!(parse_since(text, 1000).is_err(), "{text:?}");
        }
    }

    #[test]
    fn an_exited_line_names_the_reason_once_with_its_status() {
        assert_eq!(
            exited("exited", json!(0), Value::Null),
            "  r  a  exited  code 0\n"
        );
        assert_eq!(
            exited("stopped", Value::Null, json!(15)),
            "  r  a  exited  stopped  signal 15\n"
        );
    }
}
