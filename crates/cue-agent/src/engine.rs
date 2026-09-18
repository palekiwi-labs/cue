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
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::{Duration, Instant};

/// Window between SIGTERM and SIGKILL when tearing a group down.
const DEFAULT_GRACE: Duration = Duration::from_secs(5);
/// Idle sleep between polls: long enough to leave the CPU alone, far below
/// human perception of an interrupt.
const POLL_INTERVAL: Duration = Duration::from_millis(15);

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

fn abort_requested() -> Option<i32> {
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
    /// The caller's cue context, exported so a subagent's own cue writes land
    /// there. When absent the variable is removed rather than left inherited,
    /// so the child's cue writes fail loudly instead of guessing a context.
    pub context: Option<String>,
    pub events_path: PathBuf,
    pub stderr_path: PathBuf,
    pub deadline: Option<Duration>,
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
    /// The batch was interrupted; both the trigger and how the child actually
    /// died are recorded, because they are distinct facts.
    Aborted {
        trigger: i32,
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

    loop {
        let mut progressed = false;
        let mut live = 0usize;

        for state in states.iter_mut() {
            let State::Live(run) = state else { continue };
            live += 1;

            match run.child.try_wait() {
                Ok(Some(status)) => {
                    *state = State::Done(run.finish(status));
                    progressed = true;
                    continue;
                }
                Ok(None) => {}
                Err(err) => {
                    run.group.kill_now();
                    *state = State::Done(RunOutcome {
                        disposition: Disposition::WaitFailed {
                            message: format!("could not wait on the harness: {err}"),
                        },
                        duration: run.started.elapsed(),
                    });
                    progressed = true;
                    continue;
                }
            }

            if let Some(signal) = abort_requested()
                && run.teardown.is_none()
            {
                run.begin_teardown(Reason::Aborted(signal), grace);
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
fn grace_window() -> Duration {
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
    Aborted(i32),
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
    match &plan.context {
        Some(context) => command.env("CUE_CONTEXT", context),
        None => command.env_remove("CUE_CONTEXT"),
    };

    let started = Instant::now();
    let child = command
        .spawn()
        .with_context(|| format!("Could not start the harness {}", plan.program.display()))?;
    // Capture the group id once, immediately: `child.id()` is unusable after
    // the child is reaped, and reusing a reaped pid would signal a stranger.
    let group = ProcessGroup::new(child.id() as i32);

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
