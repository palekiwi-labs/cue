//! The `cue add` helper that writes traces is bounded: descendants holding
//! its output, a stall or an interrupt cannot hold back the batch's results
//! or its worktree cleanup.

mod helpers;

use helpers::{Sandbox, branches, receipt, write_executable};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

const MANIFEST: &str = r#"{"agents": {"explore": {}}}"#;
const CONTEXT: &str = "acme/widgets/work";

fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    sandbox.init_git_repo("git@github.com:acme/widgets.git");
    sandbox.context(CONTEXT);
    sandbox
}

/// Install a fake `cue` running `body` after recording its process ids.
fn install_cue(sandbox: &Sandbox, body: &str) -> PathBuf {
    let log = sandbox.harness_log();
    let path = sandbox.dir.path().join("stalling-cue");
    write_executable(
        &path,
        &format!(
            "#!/usr/bin/env bash\nset -u\nlog={log}\n\
             printf '%s %s\\n' \"$$\" \"$(ps -o pgid= -p $$ | tr -d ' ')\" >\"$log/cue.ids\"\n\
             {body}\n",
            log = log.display()
        ),
    );
    path
}

/// The helper's pid and process group, as it recorded them.
fn helper_ids(sandbox: &Sandbox) -> (i32, i32) {
    let raw = std::fs::read_to_string(sandbox.harness_log().join("cue.ids")).unwrap();
    let mut ids = raw.split_whitespace().map(|id| id.parse().unwrap());
    (ids.next().unwrap(), ids.next().unwrap())
}

fn read_pid(path: &Path) -> i32 {
    std::fs::read_to_string(path)
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

/// Whether `pid` is a live process; a zombie awaiting its reaper is not.
fn alive(pid: i32) -> bool {
    match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => !stat
            .rsplit(')')
            .next()
            .unwrap_or_default()
            .trim_start()
            .starts_with('Z'),
        Err(_) => false,
    }
}

fn eventually_dead(pid: i32) -> bool {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if !alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn wait_for(what: &str, ready: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn the_run(output: &std::process::Output) -> Value {
    receipt(&output.stdout)["runs"][0].clone()
}

#[test]
fn an_interrupt_during_a_stalled_trace_write_still_returns_results_and_cleans_up() {
    let sandbox = sandbox();
    let project = sandbox.project();
    let checkout = sandbox.dir.path().join("eph-wt");
    let stalled = sandbox.harness_log().join("stalled");
    let descendant = sandbox.harness_log().join("descendant.pid");
    let cue = install_cue(
        &sandbox,
        &format!(
            "trap '' INT TERM\nsleep 60 &\nprintf '%s\\n' \"$!\" >{pid}\n: >{mark}\nwait",
            pid = descendant.display(),
            mark = stalled.display()
        ),
    );

    let child = sandbox
        .cmd()
        .env("CUE_AGENT_CUE_BIN", &cue)
        .env("CUE_AGENT_GRACE_MS", "300")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(["run", "--json"])
        .arg(
            json!({"tasks": [{"agent": "explore", "prompt": "hello", "context": CONTEXT,
                              "worktree": {"base": "main", "ephemeral": true, "path": checkout}}]})
            .to_string(),
        )
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn cue-agent");
    wait_for("the trace helper", || stalled.exists());
    let (helper, group) = helper_ids(&sandbox);
    // SAFETY: getpgid only reads process state.
    let batch_group = unsafe { libc::getpgid(child.id() as i32) };
    // SAFETY: signal the cue-agent process owned by this test.
    unsafe { libc::kill(child.id() as i32, libc::SIGINT) };
    let started = Instant::now();
    let output = child.wait_with_output().expect("wait");
    let elapsed = started.elapsed();

    assert_eq!(group, helper, "the helper leads its own process group");
    assert_ne!(
        group, batch_group,
        "the helper is outside the batch's group"
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "the interrupt reaches finalization within the grace window: {elapsed:?}"
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let run = the_run(&output);
    assert_eq!(run["outcome"], "completed", "the result is kept: {run}");
    assert_eq!(run["response"], "echo: hello");
    assert_eq!(run["trace"], Value::Null, "{run}");
    assert!(
        run["trace_error"]
            .as_str()
            .is_some_and(|error| error.contains("interrupted")),
        "{run}"
    );
    assert_eq!(run["cleanup_errors"], json!([]), "{run}");
    assert!(!checkout.exists(), "the ephemeral checkout is removed");
    assert_eq!(branches(&project), ["main"]);
    let stored: Value = serde_json::from_slice(
        &std::fs::read(Path::new(run["run_path"].as_str().unwrap()).join("receipt.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(stored["outcome"], "completed");
    assert_eq!(stored["trace_error"], run["trace_error"]);
    assert!(!alive(helper), "the stalled helper is stopped");
    assert!(
        eventually_dead(read_pid(&descendant)),
        "the helper's whole group is stopped"
    );
}

#[test]
fn a_response_available_after_an_interrupt_during_supervision_is_still_captured() {
    let sandbox = sandbox();
    let project = sandbox.project();
    let checkout = sandbox.dir.path().join("eph-wt");
    let ready = sandbox.harness_log().join("pi.ready");
    // The harness answers the teardown SIGTERM with its final message, so a
    // response exists only once the batch is already interrupted.
    sandbox.install_pi(&format!(
        "#!/usr/bin/env bash\nset -u\n\
         [[ \"${{1:-}}\" == --version ]] && {{ echo 'fake-pi 9.9.9'; exit 0; }}\n\
         answer() {{\n\
         printf '%s\\n' '{{\"type\":\"message_end\",\"message\":{{\"role\":\"assistant\",\
         \"stopReason\":\"stop\",\"content\":[{{\"type\":\"text\",\"text\":\"final after interrupt\"}}]}}}}'\n\
         exit 0\n}}\n\
         trap answer TERM\n\
         sleep 60 &\n\
         : >{ready}\n\
         wait\n",
        ready = ready.display()
    ));

    let child = sandbox
        .cmd()
        .env("CUE_AGENT_CUE_BIN", helpers::real_cue())
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(["run", "--json"])
        .arg(
            json!({"tasks": [{"agent": "explore", "prompt": "hello", "context": CONTEXT,
                              "worktree": {"base": "main", "ephemeral": true, "path": checkout}}]})
            .to_string(),
        )
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn cue-agent");
    wait_for("the harness", || ready.exists());
    // SAFETY: signal the cue-agent process owned by this test.
    unsafe { libc::kill(child.id() as i32, libc::SIGINT) };
    let output = child.wait_with_output().expect("wait");

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let run = the_run(&output);
    assert_eq!(run["outcome"], "aborted", "{run}");
    assert_eq!(run["response"], "final after interrupt");
    assert_eq!(
        run["trace_error"],
        Value::Null,
        "the trace helper gets its own grace window: {run}"
    );
    let address = run["trace"].as_str().expect("trace address");
    let stored = std::fs::read_to_string(sandbox.store().join(address)).unwrap();
    assert!(stored.ends_with("\n---\nfinal after interrupt"), "{stored}");
    assert_eq!(run["cleanup_errors"], json!([]), "{run}");
    assert!(!checkout.exists(), "the ephemeral checkout is removed");
    assert_eq!(branches(&project), ["main"]);
}

#[test]
fn helper_descendants_holding_its_output_do_not_hold_the_results() {
    let sandbox = sandbox();
    let descendant = sandbox.harness_log().join("descendant.pid");
    let cue = install_cue(
        &sandbox,
        &format!(
            "sleep 30 &\nprintf '%s\\n' \"$!\" >{pid}\nprintf 'trace\\n'",
            pid = descendant.display()
        ),
    );

    let started = Instant::now();
    let output = sandbox
        .cmd()
        .env("CUE_AGENT_CUE_BIN", &cue)
        .args(["run", "--json"])
        .arg(
            json!({"tasks": [{"agent": "explore", "prompt": "hello", "context": CONTEXT}]})
                .to_string(),
        )
        .output()
        .expect("run cue-agent");
    let elapsed = started.elapsed();

    assert!(output.status.success(), "{output:?}");
    let run = the_run(&output);
    assert_eq!(run["outcome"], "completed", "{run}");
    assert_eq!(run["trace_error"], Value::Null, "{run}");
    assert!(run["trace"].as_str().is_some(), "{run}");
    assert!(
        elapsed < Duration::from_secs(10),
        "the helper's exit ends the write, not its descendants: {elapsed:?}"
    );
    assert!(
        eventually_dead(read_pid(&descendant)),
        "descendants left in the helper's group are stopped"
    );
}
