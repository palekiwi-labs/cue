//! Deadlines, interrupts and descendants that outlive the harness.

mod helpers;

use helpers::{Sandbox, receipt, run_of};
use std::process::Stdio;
use std::time::{Duration, Instant};

const MANIFEST: &str = r#"{
  "agents": {
    "alpha": { "description": "a" },
    "beta": { "description": "b" }
  }
}"#;

#[test]
fn a_run_past_its_deadline_is_torn_down_and_reported_as_a_timeout() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);

    let started = Instant::now();
    let output = sandbox
        .cmd()
        .args(["run", "alpha", "--prompt", "SLEEP=30", "--timeout", "1"])
        .output()
        .expect("run cue-agent");
    let elapsed = started.elapsed();

    assert_eq!(output.status.code(), Some(1));
    let run = receipt(&output.stdout);
    let run = run_of(&run, "alpha");
    assert_eq!(run["outcome"], "timeout");
    assert_eq!(run["signal"], 15, "SIGTERM comes first");
    assert!(
        elapsed < Duration::from_secs(8),
        "the deadline must end the run rather than wait it out: {elapsed:?}"
    );
}

#[test]
fn a_child_that_ignores_sigterm_is_escalated_to_sigkill() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);

    let started = Instant::now();
    let output = sandbox
        .cmd()
        .env("CUE_AGENT_GRACE_MS", "300")
        .args([
            "run",
            "alpha",
            "--prompt",
            "IGNORE_TERM=60",
            "--timeout",
            "1",
        ])
        .output()
        .expect("run cue-agent");
    let elapsed = started.elapsed();

    let run = receipt(&output.stdout);
    let run = run_of(&run, "alpha");
    assert_eq!(run["outcome"], "timeout");
    assert_eq!(run["signal"], 9, "the grace window ends in SIGKILL");
    assert!(
        elapsed < Duration::from_secs(8),
        "escalation must not wait for the child to relent: {elapsed:?}"
    );
}

#[test]
fn an_interrupt_tears_down_every_run_in_the_batch() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let batch = serde_json::json!({
        "runs": [
            { "agent": "alpha", "prompt": "SLEEP=30" },
            { "agent": "beta", "prompt": "SLEEP=30" }
        ]
    })
    .to_string();
    let batch_path = sandbox.project().join("batch.json");
    std::fs::write(&batch_path, batch).unwrap();

    let child = sandbox
        .cmd()
        .args(["run", "--batch"])
        .arg(&batch_path)
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn cue-agent");

    // Wait until both children are actually running, then interrupt.
    let deadline = Instant::now() + Duration::from_secs(5);
    while sandbox.recorded_sessions().len() < 2 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    unsafe { libc::kill(child.id() as i32, libc::SIGINT) };

    let started = Instant::now();
    let output = child.wait_with_output().expect("wait");
    let elapsed = started.elapsed();

    assert_eq!(output.status.code(), Some(1));
    let receipt = receipt(&output.stdout);
    for agent in ["alpha", "beta"] {
        let run = run_of(&receipt, agent);
        assert_eq!(run["outcome"], "aborted", "{run}");
        assert!(
            run["error"].as_str().unwrap().contains("signal 2"),
            "the interrupt that triggered teardown is recorded: {run}"
        );
    }
    assert!(
        elapsed < Duration::from_secs(8),
        "an interrupt must land within a poll tick: {elapsed:?}"
    );
}

#[test]
fn interrupt_escalates_sigterm_ignoring_children_and_preserves_the_trigger() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let child = sandbox
        .cmd()
        .args(["run", "alpha", "--prompt", "IGNORE_TERM=10"])
        .env("CUE_AGENT_GRACE_MS", "100")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let ready = loop {
        if sandbox
            .recorded_sessions()
            .iter()
            .any(|id| sandbox.harness_log().join(format!("{id}.ready")).exists())
        {
            break true;
        }
        if Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(15));
    };
    // SAFETY: signal the child process owned by this test.
    unsafe {
        libc::kill(child.id() as i32, libc::SIGINT);
    }
    let started = Instant::now();
    let output = child.wait_with_output().unwrap();
    assert!(ready, "harness never became ready");
    assert_eq!(output.status.code(), Some(1));
    let receipt = receipt(&output.stdout);
    let run = run_of(&receipt, "alpha");
    assert_eq!(run["outcome"], "aborted");
    assert_eq!(run["signal"], 9);
    assert!(run["error"].as_str().unwrap().contains("signal 2"));
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn a_grandchild_holding_the_inherited_stdout_does_not_hold_the_batch_open() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);

    // The regression that motivated file-backed capture: with a pipe, a
    // grandchild that escapes the process group keeps the write end open, so
    // the parent's reader never sees EOF and the run hangs. With a file there
    // is nothing to wait for.
    let started = Instant::now();
    let output = sandbox
        .cmd()
        .args(["run", "alpha", "--prompt", "ORPHAN=5"])
        .output()
        .expect("run cue-agent");
    let elapsed = started.elapsed();

    assert!(output.status.success(), "{output:?}");
    let run = receipt(&output.stdout);
    let run = run_of(&run, "alpha");
    assert_eq!(run["outcome"], "completed");
    assert_eq!(run["response"], "spawned orphan");
    assert!(
        elapsed < Duration::from_secs(3),
        "the run must complete when the harness exits, not when its \
         descendants do: {elapsed:?}"
    );
}
