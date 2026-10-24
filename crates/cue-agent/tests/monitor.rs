//! Discovering, inspecting and stopping running batches from another process.

mod helpers;

use helpers::{Sandbox, receipt, run_of, spec};
use std::process::{Child, Output, Stdio};
use std::time::{Duration, Instant};

const MANIFEST: &str = r#"{
  "agents": {
    "alpha": { "description": "a" },
    "beta": { "description": "b" }
  }
}"#;

fn start(sandbox: &Sandbox, spec: &serde_json::Value) -> Child {
    sandbox
        .cmd()
        .args(["run", "--json"])
        .arg(spec.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cue-agent")
}

fn json(sandbox: &Sandbox, args: &[&str]) -> (Output, serde_json::Value) {
    let output = sandbox.cmd().args(args).output().expect("run cue-agent");
    let value = serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!("not JSON: {err}: {output:?}");
    });
    (output, value)
}

fn ps(sandbox: &Sandbox) -> Vec<serde_json::Value> {
    let (output, value) = json(sandbox, &["ps", "--json"]);
    assert!(output.status.success(), "{output:?}");
    value["batches"].as_array().expect("batches").clone()
}

/// Poll until `check` holds, or panic after a few seconds.
fn wait_for(what: &str, mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !check() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(30));
    }
}

fn wait_sessions(sandbox: &Sandbox, count: usize) {
    wait_for("harnesses to start", || {
        sandbox.recorded_sessions().len() >= count
    });
}

#[test]
fn ps_discovers_a_running_batch_without_identifiers() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let child = start(
        &sandbox,
        &spec(&[("alpha", "SLEEP=30"), ("beta", "SLEEP=30")]),
    );
    wait_sessions(&sandbox, 2);

    let batches = ps(&sandbox);
    assert_eq!(batches.len(), 1, "{batches:?}");
    let batch = &batches[0];
    assert_eq!(batch["state"], "running");
    assert_eq!(batch["pid"], child.id());
    let runs = batch["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0]["agent"], "alpha");
    assert_eq!(runs[0]["phase"], "running");
    assert!(runs[0]["idle_secs"].is_number(), "{}", runs[0]);

    let (_, stopped) = json(
        &sandbox,
        &[
            "stop",
            "--json",
            "--wait",
            batch["batch_id"].as_str().unwrap(),
        ],
    );
    assert_eq!(stopped["result"], "requested", "{stopped}");
    let output = child.wait_with_output().unwrap();
    assert!(ps(&sandbox).is_empty(), "a finished batch is not listed");
    let receipt = receipt(&output.stdout);
    for agent in ["alpha", "beta"] {
        let run = run_of(&receipt, agent);
        assert_eq!(run["outcome"], "aborted", "{run}");
        assert_eq!(run["error"], "stopped by request");
    }
}

#[test]
fn a_killed_supervisor_is_reported_as_crashed_never_running() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let mut child = start(&sandbox, &spec(&[("alpha", "SLEEP=30")]));
    wait_sessions(&sandbox, 1);
    let batch_id = ps(&sandbox)[0]["batch_id"].as_str().unwrap().to_string();

    child.kill().unwrap();
    child.wait().unwrap();

    let (_, status) = json(&sandbox, &["status", "--json", &batch_id]);
    assert_eq!(status["state"], "crashed", "{status}");
    let run = &status["runs"][0];
    assert_eq!(run["phase"], "crashed");
    // The harness kept its own process group and outlived the supervisor.
    assert_eq!(run["harness_group_alive"], true, "{run}");
    let pgid = run["harness_pgid"].as_i64().unwrap();
    unsafe { libc::kill(-(pgid as i32), libc::SIGKILL) };

    // Filtered listings that do not report the crash must leave it in place.
    for filter in [
        &["ps", "--json", "--job", "other"][..],
        &["ps", "--json", "--since", "300000000000"][..],
    ] {
        let (output, value) = json(&sandbox, filter);
        assert!(output.status.success(), "{output:?}");
        assert_eq!(value["batches"], serde_json::json!([]), "{filter:?}");
    }

    let listed = ps(&sandbox);
    assert_eq!(listed.len(), 1, "the crash is still reported: {listed:?}");
    assert!(
        listed.iter().all(|batch| batch["state"] != "running"),
        "{listed:?}"
    );
    assert!(ps(&sandbox).is_empty(), "a crashed entry is reported once");
}

#[test]
fn stopping_one_run_lets_its_sibling_complete() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let child = start(
        &sandbox,
        &spec(&[("alpha", "SLEEP=30"), ("beta", "SLEEP=2")]),
    );
    wait_sessions(&sandbox, 2);
    let run_id = ps(&sandbox)[0]["runs"][0]["run_id"]
        .as_str()
        .unwrap()
        .to_string();

    let (output, stopped) = json(&sandbox, &["stop", "--json", &run_id]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(stopped["result"], "requested");
    let (_, again) = json(&sandbox, &["stop", "--json", &run_id]);
    assert_eq!(again["result"], "already_requested", "{again}");

    let started = Instant::now();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let receipt = receipt(&output.stdout);
    let alpha = run_of(&receipt, "alpha");
    assert_eq!(alpha["outcome"], "aborted", "{alpha}");
    assert_eq!(alpha["error"], "stopped by request");
    let beta = run_of(&receipt, "beta");
    assert_eq!(beta["outcome"], "completed", "{beta}");
    assert_eq!(beta["response"], "slept 2");
    assert!(started.elapsed() < Duration::from_secs(8));

    let (_, finished) = json(&sandbox, &["stop", "--json", &run_id]);
    assert_eq!(finished["result"], "already_finished", "{finished}");
    let (_, status) = json(&sandbox, &["status", "--json", &run_id]);
    assert_eq!(status["phase"], "finished", "{status}");
    assert_eq!(status["result"]["outcome"], "aborted");
}

#[test]
fn an_ended_run_is_reported_as_exited_while_its_siblings_run() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let child = start(
        &sandbox,
        &spec(&[
            ("alpha", "TOOL_WAIT=30"),
            ("beta", "SLEEP=0"),
            ("alpha", "SLEEP=30"),
        ]),
    );
    wait_sessions(&sandbox, 3);
    let runs = ps(&sandbox)[0]["runs"].as_array().unwrap().clone();
    let stopped_id = runs[0]["run_id"].as_str().unwrap().to_string();
    let natural_id = runs[1]["run_id"].as_str().unwrap().to_string();

    wait_for("the tool call to be observed", || {
        json(&sandbox, &["status", "--json", &stopped_id]).1["last_tool"].is_object()
    });
    json(&sandbox, &["stop", "--json", &stopped_id]);

    let mut statuses = (serde_json::Value::Null, serde_json::Value::Null);
    wait_for("both runs to be reported as exited", || {
        statuses = (
            json(&sandbox, &["status", "--json", &stopped_id]).1,
            json(&sandbox, &["status", "--json", &natural_id]).1,
        );
        statuses.0["phase"] == "exited" && statuses.1["phase"] == "exited"
    });
    let (stopped, natural) = statuses;
    assert_eq!(stopped["exit"]["reason"], "stopped", "{stopped}");
    assert_eq!(stopped["last_tool"]["pending"], false, "{stopped}");
    assert_eq!(natural["exit"]["reason"], "exited", "{natural}");
    assert_eq!(natural["exit"]["code"], 0, "{natural}");
    assert_eq!(
        ps(&sandbox)[0]["state"],
        "running",
        "the sibling still runs"
    );

    // Elapsed and idle are frozen at exit rather than advancing.
    std::thread::sleep(Duration::from_millis(1100));
    let later = json(&sandbox, &["status", "--json", &natural_id]).1;
    assert_eq!(later["elapsed_secs"], natural["elapsed_secs"], "{later}");
    assert_eq!(later["idle_secs"], natural["idle_secs"], "{later}");

    let batch_id = ps(&sandbox)[0]["batch_id"].as_str().unwrap().to_string();
    json(&sandbox, &["stop", "--json", "--wait", &batch_id]);
    child.wait_with_output().unwrap();
}

#[test]
fn status_reports_the_last_tool_call_and_whether_it_is_pending() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let child = start(&sandbox, &spec(&[("alpha", "TOOL_WAIT=30")]));
    wait_sessions(&sandbox, 1);
    let run_id = sandbox.recorded_sessions()[0].clone();

    let mut status = serde_json::Value::Null;
    wait_for("the tool call to be observed", || {
        status = json(&sandbox, &["status", "--json", &run_id]).1;
        status["last_tool"].is_object()
    });
    assert_eq!(status["phase"], "running");
    assert_eq!(status["turns"], 1);
    assert_eq!(status["tokens_input"], 11);
    let tool = &status["last_tool"];
    assert_eq!(tool["name"], "bash");
    assert_eq!(tool["pending"], true);
    assert!(
        tool["arguments"].as_str().unwrap().contains("sleep 30"),
        "{tool}"
    );

    let (_, logs) = (
        (),
        sandbox
            .cmd()
            .args(["logs", &run_id])
            .output()
            .expect("logs"),
    );
    let text = String::from_utf8_lossy(&logs.stdout);
    assert!(text.contains("tool_execution_start"), "{text}");

    json(&sandbox, &["stop", "--json", "--wait", &run_id]);
    child.wait_with_output().unwrap();
}

#[test]
fn a_stopped_run_with_a_final_response_is_still_captured() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    // Answers, then keeps running: the answer is captured on stop.
    let child = start(&sandbox, &spec(&[("alpha", "ANSWER_THEN_SLEEP=30")]));
    wait_sessions(&sandbox, 1);
    let run_id = sandbox.recorded_sessions()[0].clone();
    wait_for("the answer", || {
        json(&sandbox, &["status", "--json", &run_id]).1["turns"] == 1
    });
    json(&sandbox, &["stop", "--json", &run_id]);
    let output = child.wait_with_output().unwrap();
    let receipt = receipt(&output.stdout);
    let run = run_of(&receipt, "alpha");
    assert_eq!(run["outcome"], "aborted");
    assert_eq!(run["response"], "early answer");
}

#[test]
fn logs_follow_streams_until_the_batch_ends() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let child = start(&sandbox, &spec(&[("alpha", "SLEEP=2")]));
    wait_sessions(&sandbox, 1);
    let run_id = sandbox.recorded_sessions()[0].clone();
    let started = Instant::now();
    let logs = sandbox
        .cmd()
        .args(["logs", "--follow", &run_id])
        .output()
        .expect("logs");
    assert!(logs.status.success(), "{logs:?}");
    assert!(started.elapsed() < Duration::from_secs(8));
    let text = String::from_utf8_lossy(&logs.stdout);
    assert!(text.contains("slept 2"), "followed to the end: {text}");
    child.wait_with_output().unwrap();
}

#[test]
fn unknown_targets_are_usage_errors() {
    let sandbox = Sandbox::new();
    for args in [
        vec!["status", "nope"],
        vec!["stop", "nope"],
        vec!["logs", "nope"],
    ] {
        let output = sandbox.cmd().args(&args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
    }
}
