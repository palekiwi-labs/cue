//! Trace capture through the real `cue` binary: artifact names, verbatim
//! bodies, source provenance and local-record failures.

mod helpers;

use helpers::{Sandbox, branches, git, init_repo, receipt, write_executable};
use serde_json::{Value, json};
use std::path::Path;

const MANIFEST: &str = r#"{"agents": {"explore": {}, "review": {}}}"#;

fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    sandbox.init_git_repo("git@github.com:acme/widgets.git");
    sandbox
}

/// Run with the real `cue` writing traces and host Git configuration ignored.
fn run(sandbox: &Sandbox, spec: &Value) -> std::process::Output {
    sandbox
        .cmd()
        .env("CUE_AGENT_CUE_BIN", helpers::real_cue())
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(["run", "--json"])
        .arg(spec.to_string())
        .output()
        .expect("run cue-agent")
}

fn runs(output: &std::process::Output) -> Vec<Value> {
    receipt(&output.stdout)["runs"].as_array().unwrap().clone()
}

/// A stored trace split into its frontmatter and its body bytes.
fn stored(sandbox: &Sandbox, address: &str) -> (serde_yaml::Value, Vec<u8>) {
    let path = sandbox.store().join(address);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|err| panic!("missing trace {}: {err}", path.display()));
    let rest = bytes.strip_prefix(b"---\n").expect("frontmatter opens");
    let end = rest
        .windows(5)
        .position(|window| window == b"\n---\n")
        .expect("frontmatter closes");
    let frontmatter = serde_yaml::from_slice(&rest[..end + 1]).expect("frontmatter yaml");
    (frontmatter, rest[end + 5..].to_vec())
}

#[test]
fn traces_share_a_labelled_batch_directory_and_specification_positions() {
    let sandbox = sandbox();
    sandbox.context("acme/widgets/first");
    sandbox.context("other/project/second");

    let output = run(
        &sandbox,
        &json!({
            "label": "Review the diff!",
            "defaults": {"context": "acme/widgets/first"},
            "tasks": [
                {"agent": "explore", "prompt": "SLEEP=0.8"},
                {"agent": "explore", "prompt": "two", "context": "other/project/second"},
                {"agent": "review", "prompt": "three", "context": null},
                {"agent": "explore", "prompt": "four", "label": "last one"}
            ]
        }),
    );
    assert!(output.status.success(), "{output:?}");
    let receipt = receipt(&output.stdout);
    let batch = receipt["batch_id"].as_str().unwrap();
    let runs = receipt["runs"].as_array().unwrap();
    let responses: Vec<_> = runs.iter().map(|run| run["response"].clone()).collect();
    assert_eq!(
        responses,
        ["slept 0.8", "echo: two", "echo: three", "echo: four"],
        "results keep specification order, not completion order"
    );

    let dir = format!("agent/review-the-diff-{batch}");
    let traces: Vec<_> = runs.iter().map(|run| run["trace"].clone()).collect();
    assert_eq!(
        traces,
        [
            json!(format!("acme/widgets/first/trace/{dir}/001-explore.md")),
            json!(format!("other/project/second/trace/{dir}/002-explore.md")),
            Value::Null,
            json!(format!("acme/widgets/first/trace/{dir}/004-explore.md")),
        ],
        "numbering is global across destinations and survives repeated agents"
    );
    for run in runs {
        assert_eq!(run["trace_error"], Value::Null, "{run}");
    }

    let (first, body) = stored(&sandbox, traces[0].as_str().unwrap());
    assert_eq!(body, b"slept 0.8");
    assert_eq!(first["batch_label"], "Review the diff!");
    assert_eq!(first["batch_id"], batch);
    assert_eq!(first["description"], "Review the diff!");
    let (foreign, body) = stored(&sandbox, traces[1].as_str().unwrap());
    assert_eq!(body, b"echo: two");
    assert_eq!(foreign["repo_id"], "acme/widgets");
    assert_eq!(
        foreign["commit_hash"],
        git(&sandbox.project(), &["rev-parse", "--short", "HEAD"])
    );
    let (last, _) = stored(&sandbox, traces[3].as_str().unwrap());
    assert_eq!(last["description"], "last one");
    assert_eq!(last["batch_label"], "Review the diff!");
    assert!(
        !sandbox
            .store()
            .join("acme/widgets/first/trace")
            .join(&dir)
            .join("003-review.md")
            .exists(),
        "null clears the inherited destination"
    );
}

#[test]
fn trace_bodies_are_the_final_response_byte_for_byte() {
    let sandbox = sandbox();
    sandbox.context("acme/widgets/verbatim");
    let first = "---\nkind: not frontmatter\n---\n\n  leading spaces and \"quotes\"\\ ";
    let second = "\nunicode: \u{e9}\u{4e2d}\t\n\n";
    let events = sandbox.dir.path().join("events.jsonl");
    let line = json!({"type": "message_end", "message": {
        "role": "assistant", "stopReason": "stop",
        "content": [{"type": "text", "text": first}, {"type": "text", "text": second}]
    }});
    std::fs::write(&events, format!("{line}\n")).unwrap();

    let output = run(
        &sandbox,
        &json!({"tasks": [{"agent": "explore", "prompt": format!("EMIT={}", events.display()),
                           "context": "acme/widgets/verbatim"}]}),
    );
    assert!(output.status.success(), "{output:?}");
    let receipt = receipt(&output.stdout);
    let run = &receipt["runs"][0];
    let expected = format!("{first}{second}");
    assert_eq!(run["response"], expected);
    let address = run["trace"].as_str().expect("trace");
    assert_eq!(
        address,
        format!(
            "acme/widgets/verbatim/trace/agent/batch-{}/001-explore.md",
            receipt["batch_id"].as_str().unwrap()
        ),
        "an unlabelled batch gets a neutral directory"
    );
    let (frontmatter, body) = stored(&sandbox, address);
    assert_eq!(body, expected.as_bytes());
    assert_eq!(frontmatter["agent"], "explore");
    assert!(frontmatter.get("batch_label").is_none(), "{frontmatter:?}");
}

#[test]
fn source_provenance_ignores_inherited_git_routing_variables() {
    let sandbox = sandbox();
    sandbox.context("acme/widgets/routed");
    let other = sandbox.dir.path().join("other");
    init_repo(&other, "other\n");
    git(
        &other,
        &["remote", "add", "origin", "git@github.com:acme/other.git"],
    );
    let expected = git(&sandbox.project(), &["rev-parse", "--short", "HEAD"]);

    let output = sandbox
        .cmd()
        .env("CUE_AGENT_CUE_BIN", helpers::real_cue())
        .env("GIT_DIR", other.join(".git"))
        .env("GIT_WORK_TREE", &other)
        .args(["run", "--json"])
        .arg(
            json!({"tasks": [{"agent": "explore", "prompt": "hello",
                              "context": "acme/widgets/routed"}]})
            .to_string(),
        )
        .output()
        .expect("run cue-agent");
    assert!(output.status.success(), "{output:?}");
    let runs = runs(&output);
    let run = &runs[0];
    assert_eq!(run["trace_error"], Value::Null, "{run}");
    let (frontmatter, _) = stored(&sandbox, run["trace"].as_str().unwrap());
    assert_eq!(frontmatter["repo_id"], "acme/widgets");
    assert_eq!(frontmatter["commit_hash"], expected.as_str());

    let manifest: Value = serde_json::from_slice(
        &std::fs::read(Path::new(run["run_path"].as_str().unwrap()).join("manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["repo"], "acme/widgets");
    assert_eq!(manifest["commit"], expected.as_str());
}

#[test]
fn a_cross_scope_trace_preserves_an_ephemeral_checkouts_final_revision() {
    let sandbox = sandbox();
    sandbox.context("other/project/work");
    let project = sandbox.project();
    let checkout = sandbox.dir.path().join("eph-wt");

    let output = run(
        &sandbox,
        &json!({"tasks": [{"agent": "explore", "prompt": "COMMIT", "cwd": project,
                           "context": "other/project/work",
                           "worktree": {"base": "main", "ephemeral": true, "path": checkout}}]}),
    );
    assert!(output.status.success(), "{output:?}");
    let runs = runs(&output);
    let run = &runs[0];
    let full = run["response"]
        .as_str()
        .unwrap()
        .strip_prefix("committed ")
        .unwrap()
        .to_string();
    assert!(!checkout.exists(), "the checkout is removed");
    assert_eq!(branches(&project), ["main"]);
    let (frontmatter, _) = stored(&sandbox, run["trace"].as_str().unwrap());
    assert_eq!(frontmatter["repo_id"], "acme/widgets");
    let stamped = frontmatter["commit_hash"].as_str().unwrap();
    assert!(
        full.starts_with(stamped) && stamped.len() >= 7,
        "{stamped} is not the executed revision {full}"
    );
}

#[test]
fn an_unavailable_state_directory_fails_every_task_with_results() {
    let sandbox = sandbox();
    sandbox.context("acme/widgets/work");
    let blocked = sandbox.dir.path().join("state-file");
    std::fs::write(&blocked, "not a directory").unwrap();
    let checkout = sandbox.dir.path().join("never-wt");

    let output = sandbox
        .cmd()
        .env("XDG_STATE_HOME", &blocked)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(["run", "--json"])
        .arg(
            json!({"defaults": {"context": "acme/widgets/work"}, "tasks": [
                {"agent": "explore", "prompt": "one"},
                {"agent": "review", "prompt": "two", "cwd": sandbox.project(),
                 "worktree": {"base": "main", "ephemeral": false, "path": checkout}}
            ]})
            .to_string(),
        )
        .output()
        .expect("run cue-agent");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let runs = runs(&output);
    assert_eq!(runs.len(), 2);
    for (run, agent) in runs.iter().zip(["explore", "review"]) {
        assert_eq!(run["agent"], agent);
        assert_eq!(run["outcome"], "failed", "{run}");
        assert!(
            run["error"].as_str().unwrap().contains("local run record"),
            "{run}"
        );
        assert_eq!(run["trace"], Value::Null);
    }
    assert!(sandbox.recorded_sessions().is_empty(), "nothing launched");
    assert!(
        !checkout.exists(),
        "no worktree is prepared without a record"
    );
    assert_eq!(branches(&sandbox.project()), ["main"]);
}

#[test]
fn a_request_record_failure_fails_only_its_task_and_removes_its_worktree() {
    let sandbox = sandbox();
    let root = sandbox.dir.path().canonicalize().unwrap();
    let hooked = root.join("hooked");
    init_repo(&hooked, "hooked\n");
    let plain = root.join("plain");
    init_repo(&plain, "plain\n");
    // While the first task's checkout is created, its request record path is
    // made a directory, so the record written after preparation fails.
    let hooks = root.join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    write_executable(
        &hooks.join("post-checkout"),
        "#!/usr/bin/env bash\nfor run in \"$XDG_STATE_HOME\"/cue/agent/runs/*/*/explore-1; do\n  mkdir \"$run/manifest.json\"\ndone\n",
    );
    git(
        &hooked,
        &["config", "core.hooksPath", hooks.to_str().unwrap()],
    );
    let persistent = root.join("persistent-wt");
    let other = root.join("other-wt");

    let output = sandbox
        .cmd()
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args(["run", "--json"])
        .arg(
            json!({"tasks": [
                {"agent": "explore", "prompt": "one", "cwd": hooked,
                 "worktree": {"base": "main", "ephemeral": false, "path": persistent}},
                {"agent": "review", "prompt": "two", "cwd": plain,
                 "worktree": {"base": "main", "ephemeral": false, "path": other}}
            ]})
            .to_string(),
        )
        .output()
        .expect("run cue-agent");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let runs = runs(&output);
    assert_eq!(runs[0]["outcome"], "failed", "{}", runs[0]);
    assert!(
        runs[0]["error"].as_str().unwrap().contains("manifest.json"),
        "{}",
        runs[0]
    );
    assert_eq!(runs[0]["cleanup_errors"], json!([]));
    assert!(runs[0].get("worktree").is_none(), "{}", runs[0]);
    assert!(!persistent.exists(), "an unlaunched checkout is removed");
    assert_eq!(branches(&hooked), ["main"]);

    assert_eq!(runs[1]["outcome"], "completed", "{}", runs[1]);
    assert_eq!(runs[1]["response"], "echo: two");
    assert!(other.exists(), "unaffected persistent work is kept");
    assert_eq!(sandbox.recorded_sessions().len(), 1);
}
