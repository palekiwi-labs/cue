//! Plane 1: the trace artifact in the caller's context.
//!
//! The body is the agent's final message verbatim, with nothing added, so the
//! trace and the response extracted from `events.jsonl` are byte-identical and
//! the trace can be verified or regenerated. Writing goes through the `cue`
//! CLI rather than a reimplementation, because `cue add` owns frontmatter
//! encoding and the `repo_id` and `commit_hash` stamps.

use crate::engine;
use crate::receipt::RunReceipt;
use anyhow::{Context, Result, bail};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(15);
/// How much of a failed helper's stderr is reported.
const STDERR_LIMIT_BYTES: u64 = 4096;

/// Resolve the `cue` binary: `$CUE_AGENT_CUE_BIN`, then `cue` on `PATH`.
fn cue_binary() -> std::path::PathBuf {
    std::env::var_os("CUE_AGENT_CUE_BIN")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("cue"))
}

/// The artifact name:
/// `agent/<batch-label-or-"batch">-<batch-id>/<position>-<agent>.md`.
///
/// Nested so agent runs do not crowd hand-off traces. Every task of a batch
/// shares the directory in whichever context it captures to; the batch id
/// makes it unique to the batch, and the one-based position in the original
/// tasks array, zero-padded to three digits, keeps repeated agents apart and
/// reflects specification order rather than completion order.
pub fn artifact_name(
    batch_label: Option<&str>,
    batch_id: &str,
    position: usize,
    agent: &str,
) -> String {
    // Labels are for humans: one with nothing path-safe in it falls back to
    // the neutral prefix rather than to the agent placeholder.
    let prefix = batch_label
        .filter(|label| label.chars().any(|ch| ch.is_ascii_alphanumeric()))
        .map(crate::ids::sanitize)
        .unwrap_or_else(|| "batch".to_string());
    format!(
        "agent/{prefix}-{}/{position:03}-{}.md",
        crate::ids::sanitize(batch_id),
        crate::ids::sanitize(agent),
    )
}

/// The repository and revision a run actually executed, read from its
/// directory before anything there may be removed. `None` where the
/// directory has no repository, origin or commit.
#[derive(Debug, Clone, Default)]
pub struct Provenance {
    /// The `<org>/<repo>` scope derived from the origin, as cue derives it.
    pub repo_id: Option<String>,
    pub commit_hash: Option<String>,
}

/// Snapshot `dir`'s provenance with bounded Git commands that ignore any
/// inherited repository-routing variables, so only `dir` selects the
/// repository.
pub fn provenance(dir: &Path) -> Provenance {
    let repo_id = crate::worktree::inspect(dir, ["remote", "get-url", "origin"])
        .ok()
        .and_then(|origin| cuelib::store::parse_origin_scope(&origin))
        .and_then(|scope| scope.to_str().map(str::to_string));
    let commit_hash = crate::worktree::inspect(dir, ["rev-parse", "--short", "HEAD"]).ok();
    Provenance {
        repo_id,
        commit_hash,
    }
}

/// Frontmatter for one agent run.
///
/// The prompt is deliberately absent: prompts are long and multi-line, which
/// YAML handles badly and which would bury the artifact head. It is written
/// verbatim to plane 2 as `prompt.md` and reached through `run_path`. What
/// survives independently of plane 2 is `description`, the caller's label.
pub fn frontmatter(
    run: &RunReceipt,
    harness: &str,
    harness_version: Option<&str>,
    description: Option<&str>,
    batch_label: Option<&str>,
    source: &Provenance,
) -> Vec<(String, String)> {
    let mut fields = vec![
        ("kind".to_string(), "agent-run".to_string()),
        ("agent".to_string(), run.agent.clone()),
        ("harness".to_string(), harness.to_string()),
        ("outcome".to_string(), run.outcome.as_str().to_string()),
        ("duration_ms".to_string(), run.duration_ms.to_string()),
        ("turns".to_string(), run.turns.to_string()),
        ("tokens_input".to_string(), run.tokens_input.to_string()),
        ("tokens_output".to_string(), run.tokens_output.to_string()),
        ("cost_usd".to_string(), format!("{:.6}", run.cost_usd)),
        ("run_id".to_string(), run.run_id.clone()),
        ("batch_id".to_string(), run.batch_id.clone()),
        (
            "run_path".to_string(),
            run.run_path.to_string_lossy().into_owned(),
        ),
    ];
    if let Some(model) = &run.model {
        fields.push(("model".to_string(), model.clone()));
    }
    if let Some(version) = harness_version {
        fields.push(("harness_version".to_string(), version.to_string()));
    }
    if let Some(description) = description {
        fields.push(("description".to_string(), description.to_string()));
    }
    if let Some(label) = batch_label {
        fields.push(("batch_label".to_string(), label.to_string()));
    }
    if let Some(code) = run.exit_code {
        fields.push(("exit_code".to_string(), code.to_string()));
    }
    // Explicit stamps win over cue's own, which would describe the directory
    // cue was started in rather than the revision the run executed.
    if let Some(repo_id) = &source.repo_id {
        fields.push(("repo_id".to_string(), repo_id.clone()));
    }
    if let Some(commit) = &source.commit_hash {
        fields.push(("commit_hash".to_string(), commit.clone()));
    }
    fields
}

/// Write the trace by invoking `cue add`, returning the canonical address.
///
/// `context` is the canonical `<org>/<repo>/<context>` destination; `repo` is
/// the directory the run executed in. Its revision is stamped explicitly by
/// the caller where known, otherwise by cue from `repo`. Cross-scope writes
/// require both source revision fields explicitly; the destination address
/// selects the scope independently of the execution directory.
pub fn write(
    repo: &Path,
    context: &str,
    name: &str,
    body_path: &Path,
    fields: &[(String, String)],
) -> Result<String> {
    let mut command = Command::new(cue_binary());
    // Inherited repository-routing variables would make cue resolve the
    // scope of another repository than `repo`.
    for name in crate::worktree::REPOSITORY_ENV {
        command.env_remove(name);
    }
    command
        .arg("-C")
        .arg(repo)
        .arg("add")
        .arg(name)
        .arg("--file")
        .arg(body_path)
        .args(["--type", "trace", "--context", context]);
    for (key, value) in fields {
        command.arg("-f").arg(format!("{key}={value}"));
    }

    run_bounded(command, crate::worktree::HELPER_LIMIT)?;
    Ok(format!("{context}/trace/{name}"))
}

/// Run the helper `command` to its exit, like the batch's Git commands:
/// with no input, output in files rather than pipes, and in its own process
/// group, so a terminal interrupt does not reach it and a descendant holding
/// its output cannot cause a pipe-EOF wait.
///
/// It is capped at `limit`, or at the grace window from when it observes an
/// interrupt if that is sooner, and its whole group is killed when the cap
/// passes: a stalled helper delays finalization but cannot stop it. Members
/// of its group still alive once it has exited are killed as well.
///
/// The cap is per helper, not a budget for the batch: a helper started after
/// an interrupt still runs, with a grace window of its own, so a response the
/// harness produced is captured, and finalization of several runs can take
/// up to one window each.
fn run_bounded(mut command: Command, limit: Duration) -> Result<()> {
    use std::os::unix::process::CommandExt;
    let mut stderr = tempfile::tempfile().context("could not create a cue output file")?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr.try_clone()?))
        .process_group(0)
        .spawn()
        .context("could not run cue add")?;
    let group = child.id() as i32;
    let kill_group = || {
        // SAFETY: `kill` is always safe; the child leads its own group.
        unsafe { libc::kill(-group, libc::SIGKILL) };
    };
    // The group is only signalled while the leader is unreaped: once it is
    // waited for, its id may already belong to an unrelated process.
    let mut kill_at = Instant::now() + limit;
    let mut interrupted = false;
    loop {
        match exited(&child) {
            Ok(true) => break,
            Ok(false) => {}
            // Whether the leader is still unreaped is unknown, so its group
            // is left alone.
            Err(err) => bail!("could not wait for cue add: {err}"),
        }
        if !interrupted && engine::abort_requested().is_some() {
            interrupted = true;
            kill_at = kill_at.min(Instant::now() + engine::grace_window());
        }
        if Instant::now() >= kill_at {
            kill_group();
            let _ = child.wait();
            if interrupted {
                bail!("cue add was stopped after the batch was interrupted");
            }
            bail!("cue add did not finish within {limit:?} and was stopped");
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    kill_group();
    let status = child.wait().context("could not wait for cue add")?;
    if !status.success() {
        let mut bytes = Vec::new();
        let _ = stderr.seek(SeekFrom::Start(0));
        let _ = (&mut stderr)
            .take(STDERR_LIMIT_BYTES)
            .read_to_end(&mut bytes);
        let stderr = String::from_utf8_lossy(&bytes).trim().to_string();
        if stderr.is_empty() {
            bail!("cue add failed: {status}");
        }
        bail!("cue add failed: {stderr}");
    }
    Ok(())
}

/// Whether `child` has exited, observed without reaping it: until it is
/// waited for, its pid, and so the id of the group it leads, cannot be
/// reused, which keeps signalling that group safe.
fn exited(child: &Child) -> std::io::Result<bool> {
    loop {
        // SAFETY: an all-zero `siginfo_t` is valid; `waitid` writes into it.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // SAFETY: `info` is a valid, exclusively borrowed `siginfo_t`.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result == 0 {
            // SAFETY: `si_pid` is set by `waitid` and zero when nothing exited.
            return Ok(unsafe { info.si_pid() } != 0);
        }
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::Interrupted {
            return Err(err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::CommandExt;

    fn alive(pid: i32) -> bool {
        std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
            !stat
                .rsplit(')')
                .next()
                .unwrap_or_default()
                .trim_start()
                .starts_with('Z')
        })
    }

    #[test]
    fn a_stalled_helper_and_its_group_are_stopped_at_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("descendant.pid");
        let mut command = Command::new("bash");
        command.arg("-c").arg(format!(
            "sleep 30 & printf '%s\\n' $! >{}; sleep 30",
            pid_file.display()
        ));

        let started = Instant::now();
        let err = run_bounded(command, Duration::from_millis(300)).unwrap_err();
        let elapsed = started.elapsed();

        assert!(
            format!("{err:#}").contains("did not finish within 300ms"),
            "{err:#}"
        );
        assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
        let pid: i32 = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while alive(pid) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!alive(pid), "the helper's group is killed with it");
    }

    #[test]
    fn observing_an_exit_keeps_the_leader_unreaped_until_the_wait() {
        let mut child = Command::new("true").process_group(0).spawn().unwrap();
        let pid = child.id() as i32;
        let deadline = Instant::now() + Duration::from_secs(5);
        while !exited(&child).unwrap() {
            assert!(Instant::now() < deadline, "the helper never exited");
            std::thread::sleep(Duration::from_millis(5));
        }

        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
        let state = stat.rsplit(')').next().unwrap().trim_start();
        assert!(
            state.starts_with('Z'),
            "the exited leader still holds its pid and group id: {state}"
        );
        // SAFETY: probes, with no signal, the group of a child this test owns.
        assert_eq!(unsafe { libc::kill(-pid, 0) }, 0, "the group is still ours");
        assert!(child.wait().unwrap().success());
    }

    #[test]
    fn a_failed_helper_reports_its_stderr() {
        let mut command = Command::new("bash");
        command
            .arg("-c")
            .arg("printf 'no such context\\n' >&2; exit 3");
        let err = run_bounded(command, Duration::from_secs(10)).unwrap_err();
        assert_eq!(format!("{err:#}"), "cue add failed: no such context");
    }

    #[test]
    fn artifact_names_group_a_batch_and_number_tasks_by_position() {
        assert_eq!(
            artifact_name(
                Some("Review the diff"),
                "20260918-120301-3f9a2b",
                1,
                "consultant-opus"
            ),
            "agent/review-the-diff-20260918-120301-3f9a2b/001-consultant-opus.md"
        );
        assert_eq!(
            artifact_name(None, "20260918-120301-3f9a2b", 12, "explore"),
            "agent/batch-20260918-120301-3f9a2b/012-explore.md"
        );
        assert_eq!(
            artifact_name(Some("../.."), "b", 1000, "a/../b"),
            "agent/batch-b/1000-a-b.md"
        );
    }
}
