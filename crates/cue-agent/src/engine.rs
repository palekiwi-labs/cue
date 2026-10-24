//! Synchronous, file-backed, poll-all supervision.
//!
//! Each child's stdout and stderr are redirected to files rather than pipes,
//! and one loop calls `try_wait` on every live child while checking the abort
//! flag and each deadline, sleeping briefly only when nothing changed. No
//! channels, no reader threads, no async runtime.
//!
//! The decisive argument is a failure mode, not simplicity. A pipe obliges the
//! parent to keep draining it and signals completion only by EOF, which arrives
//! only once every process holding the write end has exited. Children here are
//! coding agents running arbitrary bash, so a grandchild inheriting stdout is
//! the ordinary case, and with a pipe it hangs the parent's reader forever. A
//! file has neither property: child exit and output completion become
//! independent questions, answered by `try_wait` and by the file respectively.

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::{Duration, Instant};

/// Window between SIGTERM and SIGKILL when tearing a group down.
const DEFAULT_GRACE: Duration = Duration::from_secs(5);
/// Idle sleep between polls: long enough to leave the CPU alone, far below
/// human perception of an interrupt.
const POLL_INTERVAL: Duration = Duration::from_millis(15);
/// How often live runs look for stop requests.
const STOP_CHECK_INTERVAL: Duration = Duration::from_millis(100);

/// Set by the signal handler the host installs. Registration belongs to the
/// caller, not to supervision: one registration covers a whole batch, and a
/// library that installs handlers cannot be embedded.
static ABORTED: AtomicBool = AtomicBool::new(false);
static ABORT_SIGNAL: AtomicI32 = AtomicI32::new(0);

/// Install the batch-level SIGINT/SIGTERM handler.
pub fn install_signal_handlers() {
    // SAFETY: `signal` with a plain function pointer is defined behaviour; the
    // handler only touches atomics, which is async-signal-safe.
    unsafe {
        libc::signal(
            libc::SIGINT,
            handle_signal as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGTERM,
            handle_signal as *const () as libc::sighandler_t,
        );
    }
}

extern "C" fn handle_signal(signal: libc::c_int) {
    ABORT_SIGNAL.store(signal, Ordering::SeqCst);
    ABORTED.store(true, Ordering::SeqCst);
}

/// What ended a run early on the caller's behalf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// The batch received SIGINT or SIGTERM.
    Signal(i32),
    /// A separate process left a stop request for the run or its batch.
    StopRequest,
}

impl Trigger {
    /// The receipt error for a run this trigger ended.
    pub fn message(self) -> String {
        match self {
            Self::Signal(signal) => format!("the batch was interrupted by signal {signal}"),
            Self::StopRequest => "stopped by request".to_string(),
        }
    }
}

/// The signal that interrupted the batch, once one has.
pub fn abort_requested() -> Option<i32> {
    ABORTED
        .load(Ordering::SeqCst)
        .then(|| ABORT_SIGNAL.load(Ordering::SeqCst))
}

/// Everything supervision needs about one run.
#[derive(Debug, Clone)]
pub struct RunPlan {
    pub run_id: String,
    pub program: PathBuf,
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    /// Overlay on the inherited environment: `Some` sets a variable, `None`
    /// removes it. Nothing else, `CUE_CONTEXT` included, is set implicitly.
    pub env: BTreeMap<String, Option<String>>,
    pub events_path: PathBuf,
    pub stderr_path: PathBuf,
    pub deadline: Option<Duration>,
    /// Stop requests that end this run, checked while it is live: its own
    /// and its batch's.
    pub stop_paths: Vec<PathBuf>,
    /// Where the harness process group id is recorded once it starts.
    pub pgid_path: Option<PathBuf>,
    /// Where the harness's exit is recorded once it is reaped, so observers
    /// can tell an ended run from a stalled one before the batch finalizes.
    pub exit_path: Option<PathBuf>,
}

/// How a run ended. Isolated per run: one failure never ends the batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disposition {
    /// The child exited on its own.
    Exited {
        code: Option<i32>,
        signal: Option<i32>,
    },
    /// The child passed its deadline and was torn down.
    TimedOut {
        code: Option<i32>,
        signal: Option<i32>,
    },
    /// The batch was interrupted or the run stopped by request; both the trigger and how the child actually
    /// died are recorded, because they are distinct facts.
    Aborted {
        trigger: Trigger,
        code: Option<i32>,
        signal: Option<i32>,
    },
    /// The harness never started. A run failure that still yields a receipt,
    /// not a usage error.
    SpawnFailed { message: String },
    /// The harness started, but the OS could not report its exit status.
    WaitFailed { message: String },
}

#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub disposition: Disposition,
    pub duration: Duration,
}

/// Run every plan concurrently until all of them finish.
///
/// Admission happened before this call: there is no queue, so every plan is
/// spawned at once.
pub fn supervise(plans: &[RunPlan]) -> Vec<RunOutcome> {
    let grace = grace_window();
    let mut states: Vec<State> = plans.iter().map(spawn).collect();
    for (plan, state) in plans.iter().zip(&states) {
        if let State::Done(outcome) = state {
            record_exit(plan, outcome);
        }
    }
    let mut last_stop_check = Instant::now();

    loop {
        let mut progressed = false;
        let mut live = 0usize;
        // A stop request is a file; looking for it every tick would be wasted
        // system calls, and a tenth of a second is still immediate.
        let check_stops = last_stop_check.elapsed() >= STOP_CHECK_INTERVAL;
        if check_stops {
            last_stop_check = Instant::now();
        }

        for (plan, state) in plans.iter().zip(states.iter_mut()) {
            let State::Live(run) = state else { continue };
            live += 1;

            match run.child.try_wait() {
                Ok(Some(status)) => {
                    let outcome = run.finish(status);
                    record_exit(plan, &outcome);
                    *state = State::Done(outcome);
                    progressed = true;
                    continue;
                }
                Ok(None) => {}
                Err(err) => {
                    run.group.kill_now();
                    let outcome = RunOutcome {
                        disposition: Disposition::WaitFailed {
                            message: format!("could not wait on the harness: {err}"),
                        },
                        duration: run.started.elapsed(),
                    };
                    record_exit(plan, &outcome);
                    *state = State::Done(outcome);
                    progressed = true;
                    continue;
                }
            }

            if let Some(signal) = abort_requested()
                && run.teardown.is_none()
            {
                run.begin_teardown(Reason::Aborted(Trigger::Signal(signal)), grace);
                progressed = true;
            } else if run.teardown.is_none()
                && check_stops
                && plan.stop_paths.iter().any(|path| path.exists())
            {
                run.begin_teardown(Reason::Aborted(Trigger::StopRequest), grace);
                progressed = true;
            } else if run.teardown.is_none()
                && let Some(deadline) = run.deadline
                && run.started.elapsed() >= deadline
            {
                run.begin_teardown(Reason::TimedOut, grace);
                progressed = true;
            }

            if let Some(teardown) = &run.teardown
                && Instant::now() >= teardown.kill_at
                && !run.killed
            {
                run.group.kill_now();
                run.killed = true;
                progressed = true;
            }
        }

        if live == 0 {
            break;
        }
        if !progressed {
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    states
        .into_iter()
        .map(|state| match state {
            State::Done(outcome) => outcome,
            State::Live(_) => unreachable!("the loop exits only when no run is live"),
        })
        .collect()
}

/// Grace window, overridable for tests that need a child to be escalated to
/// SIGKILL without waiting five seconds for it.
pub fn grace_window() -> Duration {
    std::env::var("CUE_AGENT_GRACE_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_GRACE)
}

enum State {
    Live(Run),
    Done(RunOutcome),
}

struct Run {
    child: Child,
    group: ProcessGroup,
    started: Instant,
    deadline: Option<Duration>,
    teardown: Option<Teardown>,
    killed: bool,
}

struct Teardown {
    reason: Reason,
    kill_at: Instant,
}

#[derive(Clone, Copy)]
enum Reason {
    TimedOut,
    Aborted(Trigger),
}

impl Run {
    /// SIGTERM the group and arm the SIGKILL that follows the grace window.
    fn begin_teardown(&mut self, reason: Reason, grace: Duration) {
        self.group.terminate();
        self.teardown = Some(Teardown {
            reason,
            kill_at: Instant::now() + grace,
        });
    }

    fn finish(&mut self, status: std::process::ExitStatus) -> RunOutcome {
        use std::os::unix::process::ExitStatusExt;
        let code = status.code();
        let signal = status.signal();
        // The child is reaped, so the group may be released; anything still
        // alive in it is a descendant that outlived the harness.
        self.group.kill_now();
        let disposition = match self.teardown.as_ref().map(|teardown| teardown.reason) {
            Some(Reason::TimedOut) => Disposition::TimedOut { code, signal },
            Some(Reason::Aborted(trigger)) => Disposition::Aborted {
                trigger,
                code,
                signal,
            },
            None => Disposition::Exited { code, signal },
        };
        RunOutcome {
            disposition,
            duration: self.started.elapsed(),
        }
    }
}

/// Best effort: the record only informs observers. The receipt written at
/// finalization stays authoritative, so a failed write changes no outcome.
fn record_exit(plan: &RunPlan, outcome: &RunOutcome) {
    let Some(path) = &plan.exit_path else { return };
    let _ = crate::state::write_private(path, exit_record(outcome).to_string());
}

fn exit_record(outcome: &RunOutcome) -> serde_json::Value {
    let (reason, code, signal) = match &outcome.disposition {
        Disposition::Exited { code, signal } => ("exited", *code, *signal),
        Disposition::TimedOut { code, signal } => ("timed_out", *code, *signal),
        Disposition::Aborted {
            trigger: Trigger::StopRequest,
            code,
            signal,
        } => ("stopped", *code, *signal),
        Disposition::Aborted {
            trigger: Trigger::Signal(_),
            code,
            signal,
        } => ("interrupted", *code, *signal),
        Disposition::SpawnFailed { .. } => ("spawn_failed", None, None),
        Disposition::WaitFailed { .. } => ("wait_failed", None, None),
    };
    serde_json::json!({
        "reason": reason,
        "code": code,
        "signal": signal,
        "duration_ms": outcome.duration.as_millis() as u64,
    })
}

fn spawn(plan: &RunPlan) -> State {
    match try_spawn(plan) {
        Ok(state) => state,
        Err(err) => State::Done(RunOutcome {
            disposition: Disposition::SpawnFailed {
                message: format!("{err:#}"),
            },
            duration: Duration::ZERO,
        }),
    }
}

fn try_spawn(plan: &RunPlan) -> Result<State> {
    use std::os::unix::process::CommandExt;

    let events = crate::state::private_file(&plan.events_path)
        .with_context(|| format!("Could not create {}", plan.events_path.display()))?;
    let stderr = crate::state::private_file(&plan.stderr_path)
        .with_context(|| format!("Could not create {}", plan.stderr_path.display()))?;

    let mut command = Command::new(&plan.program);
    command
        .args(&plan.argv)
        .current_dir(&plan.cwd)
        // stdin is /dev/null: the harness must never wait on input, and an
        // immediate EOF is what tells it so.
        .stdin(Stdio::null())
        .stdout(Stdio::from(events))
        .stderr(Stdio::from(stderr))
        // Teardown signals the harness and descendants that stay in its group.
        // A descendant that detaches with setsid/setpgid is not contained.
        .process_group(0);
    for (key, value) in &plan.env {
        match value {
            Some(value) => command.env(key, value),
            None => command.env_remove(key),
        };
    }

    let started = Instant::now();
    let child = command
        .spawn()
        .with_context(|| format!("Could not start the harness {}", plan.program.display()))?;
    // Capture the group id once, immediately: `child.id()` is unusable after
    // the child is reaped, and reusing a reaped pid would signal a stranger.
    let group = ProcessGroup::new(child.id() as i32);
    // For monitors: a supervisor killed outright leaves this group running,
    // and the record is how one finds it. Monitoring is best effort.
    if let Some(path) = &plan.pgid_path {
        let _ = crate::state::write_private(path, format!("{}\n", child.id()));
    }

    Ok(State::Live(Run {
        child,
        group,
        started,
        deadline: plan.deadline,
        teardown: None,
        killed: false,
    }))
}

/// Owns the teardown of one process group. `terminate` stays armed so the
/// SIGKILL escalation can still fire; `kill_now` disarms so `Drop` is a no-op.
struct ProcessGroup(Option<i32>);

impl ProcessGroup {
    fn new(pgid: i32) -> Self {
        Self(Some(pgid))
    }

    fn terminate(&self) {
        if let Some(pgid) = self.0 {
            signal_group(pgid, libc::SIGTERM);
        }
    }

    fn kill_now(&mut self) {
        if let Some(pgid) = self.0.take() {
            signal_group(pgid, libc::SIGKILL);
        }
    }
}

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        self.kill_now();
    }
}

/// `kill(-pgid, sig)`, the POSIX "signal the whole group" form.
///
/// `pgid == 0` is refused: it would signal our own group. `ESRCH` is ignored:
/// the group being gone is the outcome we wanted.
fn signal_group(pgid: i32, signal: libc::c_int) {
    if pgid == 0 {
        return;
    }
    // SAFETY: `kill` is always safe to call; a negative pid targets a group.
    let result = unsafe { libc::kill(-(pgid as libc::pid_t), signal) };
    if result != 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::ESRCH) {
            eprintln!("cue-agent: kill(-{pgid}, {signal}): {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(dir: &std::path::Path, program: &str, argv: &[&str]) -> RunPlan {
        RunPlan {
            run_id: "run".to_string(),
            program: PathBuf::from(program),
            argv: argv.iter().map(|arg| arg.to_string()).collect(),
            cwd: dir.to_path_buf(),
            env: BTreeMap::new(),
            events_path: dir.join("events.jsonl"),
            stderr_path: dir.join("stderr.log"),
            deadline: None,
            stop_paths: Vec::new(),
            pgid_path: None,
            exit_path: Some(dir.join("exit.json")),
        }
    }

    fn recorded(dir: &std::path::Path) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(dir.join("exit.json")).expect("exit.json"))
            .expect("exit.json is JSON")
    }

    #[test]
    fn a_harness_that_never_starts_records_spawn_failed() {
        let dir = tempfile::tempdir().unwrap();
        supervise(&[plan(dir.path(), "/nonexistent/pi", &[])]);
        assert_eq!(recorded(dir.path())["reason"], "spawn_failed");
    }

    #[test]
    fn a_run_past_its_deadline_records_timed_out() {
        let dir = tempfile::tempdir().unwrap();
        let mut plan = plan(dir.path(), "sleep", &["30"]);
        plan.deadline = Some(Duration::from_millis(100));
        supervise(&[plan]);
        let exit = recorded(dir.path());
        assert_eq!(exit["reason"], "timed_out");
        assert_eq!(exit["signal"], libc::SIGTERM);
    }

    #[test]
    fn every_disposition_has_its_own_exit_reason() {
        let cases = [
            (
                Disposition::Exited {
                    code: Some(3),
                    signal: None,
                },
                "exited",
            ),
            (
                Disposition::TimedOut {
                    code: None,
                    signal: Some(15),
                },
                "timed_out",
            ),
            (
                Disposition::Aborted {
                    trigger: Trigger::StopRequest,
                    code: None,
                    signal: Some(15),
                },
                "stopped",
            ),
            (
                Disposition::Aborted {
                    trigger: Trigger::Signal(2),
                    code: None,
                    signal: Some(15),
                },
                "interrupted",
            ),
            (
                Disposition::SpawnFailed {
                    message: String::new(),
                },
                "spawn_failed",
            ),
            (
                Disposition::WaitFailed {
                    message: String::new(),
                },
                "wait_failed",
            ),
        ];
        for (disposition, reason) in cases {
            let outcome = RunOutcome {
                disposition,
                duration: Duration::from_millis(1500),
            };
            let record = exit_record(&outcome);
            assert_eq!(record["reason"], reason, "{record}");
            assert_eq!(record["duration_ms"], 1500);
        }
        let exited = exit_record(&RunOutcome {
            disposition: Disposition::Exited {
                code: Some(3),
                signal: None,
            },
            duration: Duration::ZERO,
        });
        assert_eq!(exited["code"], 3);
    }
}
