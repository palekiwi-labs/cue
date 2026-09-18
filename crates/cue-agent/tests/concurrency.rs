//! Concurrency, admission and isolation for one batch.

mod helpers;

use helpers::{Sandbox, receipt, run_of};
use std::time::Instant;

const MANIFEST: &str = r#"{
  "agents": {
    "alpha": { "description": "a" },
    "beta": { "description": "b" },
    "gamma": { "description": "c" },
    "delta": { "description": "d" },
    "epsilon": { "description": "e" }
  }
}"#;

fn batch(runs: &[(&str, &str)]) -> String {
    let runs: Vec<serde_json::Value> = runs
        .iter()
        .map(|(agent, prompt)| serde_json::json!({ "agent": agent, "prompt": prompt }))
        .collect();
    serde_json::json!({ "runs": runs }).to_string()
}

/// The peak number of harness processes the fake harness saw alive at once.
fn peak_concurrency(sandbox: &Sandbox) -> usize {
    let raw = std::fs::read_to_string(sandbox.harness_log().join("concurrency")).expect("witness");
    raw.lines()
        .filter_map(|line| line.trim().parse::<usize>().ok())
        .max()
        .unwrap_or(0)
}

#[test]
fn a_full_batch_runs_concurrently_and_results_stay_attributed_in_reverse_order() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let batch = batch(&[
        ("alpha", "SLEEP=0.8"),
        ("beta", "SLEEP=0.5"),
        ("gamma", "SLEEP=0.3"),
        ("delta", "SLEEP=0.1"),
    ]);

    let started = Instant::now();
    let output = sandbox
        .cmd()
        .args(["run", "--batch", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .take()
                .expect("stdin")
                .write_all(batch.as_bytes())?;
            child.wait_with_output()
        })
        .expect("run cue-agent");
    let elapsed = started.elapsed();
    assert!(output.status.success(), "{output:?}");

    let receipt = receipt(&output.stdout);
    // Completion order is the reverse of the request order; each result must
    // still be reported against the agent that produced it.
    assert_eq!(run_of(&receipt, "alpha")["response"], "slept 0.8");
    assert_eq!(run_of(&receipt, "beta")["response"], "slept 0.5");
    assert_eq!(run_of(&receipt, "gamma")["response"], "slept 0.3");
    assert_eq!(run_of(&receipt, "delta")["response"], "slept 0.1");

    assert_eq!(peak_concurrency(&sandbox), 4, "all four ran at once");
    assert!(
        elapsed.as_secs_f64() < 1.7,
        "a batch must take about as long as its slowest run, not their sum: {elapsed:?}"
    );
}

#[test]
fn a_batch_over_the_cap_fails_before_anything_is_spawned() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let batch = batch(&[
        ("alpha", "one"),
        ("beta", "two"),
        ("gamma", "three"),
        ("delta", "four"),
        ("epsilon", "five"),
    ]);
    let batch_path = sandbox.project().join("batch.json");
    std::fs::write(&batch_path, batch).unwrap();

    let output = sandbox
        .cmd()
        .args(["run", "--batch"])
        .arg(&batch_path)
        .output()
        .expect("run cue-agent");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("exceeds the per-call cap of 4"), "{stderr}");
    assert!(stderr.contains("does not queue"), "{stderr}");
    assert!(
        output.stdout.is_empty(),
        "a refused batch prints no receipt"
    );
    assert!(
        sandbox.recorded_sessions().is_empty(),
        "no harness process may start"
    );
    assert!(
        !sandbox.state().join("cue/agent/runs").exists(),
        "no run directory may be created"
    );
}

#[test]
fn one_failing_run_neither_stops_nor_taints_the_others() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let batch = batch(&[
        ("alpha", "FAIL=3"),
        ("beta", "MALFORMED"),
        ("gamma", "SLEEP=0.2"),
    ]);
    let batch_path = sandbox.project().join("batch.json");
    std::fs::write(&batch_path, batch).unwrap();

    let output = sandbox
        .cmd()
        .args(["run", "--batch"])
        .arg(&batch_path)
        .output()
        .expect("run cue-agent");

    assert_eq!(
        output.status.code(),
        Some(1),
        "a batch holding a failed run exits 1 but still prints its receipt"
    );
    let receipt = receipt(&output.stdout);

    let failed = run_of(&receipt, "alpha");
    assert_eq!(failed["outcome"], "failed");
    assert_eq!(failed["exit_code"], 3);
    assert!(
        failed["stderr_excerpt"]
            .as_str()
            .unwrap()
            .contains("fake harness exploded"),
        "{failed}"
    );

    let malformed = run_of(&receipt, "beta");
    assert_eq!(malformed["outcome"], "completed");
    assert_eq!(malformed["response"], "survived malformed lines");
    assert_eq!(malformed["events_malformed"], 2);

    let healthy = run_of(&receipt, "gamma");
    assert_eq!(healthy["outcome"], "completed");
    assert_eq!(healthy["response"], "slept 0.2");
}

#[test]
fn an_oversized_event_line_is_skipped_rather_than_swallowing_the_run() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);

    let output = sandbox
        .cmd()
        .args(["run", "alpha", "--prompt", "OVERSIZED"])
        .output()
        .expect("run cue-agent");
    assert!(output.status.success(), "{output:?}");

    let receipt = receipt(&output.stdout);
    let run = run_of(&receipt, "alpha");
    assert_eq!(run["events_oversized"], 1);
    assert_eq!(run["response"], "survived an oversized line");
}

#[test]
fn a_missing_harness_fails_that_run_and_still_yields_a_receipt() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);

    let output = sandbox
        .cmd()
        .env("CUE_AGENT_HARNESS", sandbox.project().join("no-such-pi"))
        .args(["run", "alpha", "--prompt", "hi"])
        .output()
        .expect("run cue-agent");

    assert_eq!(output.status.code(), Some(1));
    let receipt = receipt(&output.stdout);
    let run = run_of(&receipt, "alpha");
    assert_eq!(run["outcome"], "failed");
    assert!(
        run["error"].as_str().unwrap().contains("Could not start"),
        "{run}"
    );
}
