//! What `ps` and `status` give a generic monitor: labels, contexts, cache
//! tokens, response documents, bounded history and job grouping.

mod helpers;

use helpers::{Sandbox, receipt};
use serde_json::{Value, json};

const MANIFEST: &str = r#"{
  "agents": {
    "alpha": { "description": "a" },
    "beta": { "description": "b" }
  }
}"#;

fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    sandbox
}

/// The receipt of a synchronous run, whatever its exit code.
fn run_receipt(sandbox: &Sandbox, spec: &Value) -> Value {
    let output = sandbox.run_json(spec);
    receipt(&output.stdout)
}

fn cli_json(sandbox: &Sandbox, args: &[&str]) -> Value {
    let output = sandbox.cmd().args(args).output().expect("run cue-agent");
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).expect("JSON")
}

fn ps_all(sandbox: &Sandbox, extra: &[&str]) -> Vec<Value> {
    let mut args = vec!["ps", "--all", "--json"];
    args.extend_from_slice(extra);
    cli_json(sandbox, &args)["batches"]
        .as_array()
        .expect("batches")
        .clone()
}

#[test]
fn ps_reports_each_runs_label_and_context() {
    let sandbox = sandbox();
    sandbox.context("acme/widgets/work");
    let cue = sandbox.fake_cue();
    let output = sandbox
        .cmd()
        .env("CUE_AGENT_CUE_BIN", cue)
        .args(["run", "--json"])
        .arg(
            json!({
                "label": "sweep",
                "tasks": [
                    {"agent": "alpha", "prompt": "one", "label": "first",
                     "context": "acme/widgets/work"},
                    {"agent": "beta", "prompt": "two"}
                ]
            })
            .to_string(),
        )
        .output()
        .unwrap();
    let receipt = receipt(&output.stdout);
    let batches = ps_all(&sandbox, &[]);
    let runs = batches[0]["runs"].as_array().unwrap();
    assert_eq!(runs[0]["label"], "first");
    assert_eq!(runs[0]["context"], "acme/widgets/work");
    // A task without its own label inherits the batch label.
    assert_eq!(runs[1]["label"], "sweep");
    assert_eq!(runs[1]["context"], Value::Null);

    let run_id = receipt["runs"][0]["run_id"].as_str().unwrap();
    let status = cli_json(&sandbox, &["status", "--json", run_id]);
    assert_eq!(status["label"], "first", "{status}");
    assert_eq!(status["context"], "acme/widgets/work", "{status}");
}

#[test]
fn ps_reports_cache_tokens_beside_input_tokens() {
    let sandbox = sandbox();
    run_receipt(
        &sandbox,
        &json!({"tasks": [{"agent": "alpha", "prompt": "one"}]}),
    );
    let run = &ps_all(&sandbox, &[])[0]["runs"][0];
    assert_eq!(run["tokens_input"], 11, "{run}");
    assert_eq!(run["tokens_cache_read"], 5, "{run}");
    assert_eq!(run["tokens_cache_write"], 3, "{run}");
}

#[test]
fn a_run_with_a_response_writes_response_md_and_one_without_does_not() {
    let sandbox = sandbox();
    let receipt = run_receipt(
        &sandbox,
        &json!({"tasks": [
            {"agent": "alpha", "prompt": "answer me"},
            {"agent": "beta", "prompt": "FAIL=3"}
        ]}),
    );
    let runs = receipt["runs"].as_array().unwrap();
    let answered = std::path::Path::new(runs[0]["run_path"].as_str().unwrap());
    let response = std::fs::read_to_string(answered.join("response.md")).unwrap();
    assert_eq!(response, runs[0]["response"].as_str().unwrap());
    let failed = std::path::Path::new(runs[1]["run_path"].as_str().unwrap());
    assert_eq!(runs[1]["response"], "", "{}", runs[1]);
    assert!(!failed.join("response.md").exists());
}

#[test]
fn ps_all_is_bounded_by_limit_and_since() {
    let sandbox = sandbox();
    let spec = json!({"tasks": [{"agent": "alpha", "prompt": "one"}]});
    let mut ids = Vec::new();
    for _ in 0..3 {
        ids.push(
            run_receipt(&sandbox, &spec)["batch_id"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    // An old batch that only an unbounded scan reaches.
    let old = sandbox
        .state()
        .join("cue/agent/runs/2000-01-01/20000101-000000-abcdef");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(
        old.join("batch.json"),
        r#"{"batch_id":"20000101-000000-abcdef","runs":[]}"#,
    )
    .unwrap();

    let listed = |extra: &[&str]| -> Vec<String> {
        ps_all(&sandbox, extra)
            .iter()
            .map(|batch| batch["batch_id"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(listed(&[]).len(), 4);
    ids.sort();
    ids.reverse();
    assert_eq!(listed(&["--limit", "2"]), ids[..2].to_vec());
    assert_eq!(listed(&["--since", "1d"]), ids);
    assert_eq!(
        listed(&["--since", "1d", "--limit", "1"]),
        ids[..1].to_vec()
    );
    assert_eq!(listed(&["--since", "946684800"]), {
        let mut all = ids.clone();
        all.push("20000101-000000-abcdef".to_string());
        all
    });
    assert!(listed(&["--since", "99999999999"]).is_empty());
    // Past year 9999 the bound is still after every batch.
    assert!(listed(&["--since", "300000000000"]).is_empty());

    let bad = sandbox
        .cmd()
        .args(["ps", "--all", "--since", "soon"])
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(2), "{bad:?}");
}

#[test]
fn batches_carry_an_optional_job_and_ps_filters_by_it() {
    let sandbox = sandbox();
    let task = json!([{"agent": "alpha", "prompt": "one"}]);
    let first = run_receipt(
        &sandbox,
        &json!({"job": {"id": "review-42", "label": "review f93e3b2"}, "tasks": task}),
    );
    let second = run_receipt(
        &sandbox,
        &json!({"job": {"id": "review-42"}, "tasks": task}),
    );
    run_receipt(&sandbox, &json!({"tasks": task}));

    let third = run_receipt(&sandbox, &json!({"tasks": task}));
    let batches = ps_all(&sandbox, &[]);
    let job_of = |receipt: &Value| {
        batches
            .iter()
            .find(|batch| batch["batch_id"] == receipt["batch_id"])
            .expect("listed")["job"]
            .clone()
    };
    assert_eq!(
        job_of(&first),
        json!({"id": "review-42", "label": "review f93e3b2"})
    );
    assert_eq!(job_of(&second), json!({"id": "review-42", "label": null}));
    assert_eq!(job_of(&third), Value::Null);

    let mut filtered: Vec<Value> = ps_all(&sandbox, &["--job", "review-42"])
        .iter()
        .map(|batch| batch["batch_id"].clone())
        .collect();
    filtered.sort_by_key(|id| id.to_string());
    let mut expected = vec![first["batch_id"].clone(), second["batch_id"].clone()];
    expected.sort_by_key(|id| id.to_string());
    assert_eq!(filtered, expected);

    let batch_id = first["batch_id"].as_str().unwrap();
    let status = cli_json(&sandbox, &["status", "--json", batch_id]);
    assert_eq!(status["job"]["id"], "review-42", "{status}");
}

#[test]
fn a_malformed_job_is_rejected_before_anything_runs() {
    let sandbox = sandbox();
    for job in [
        json!("review-42"),
        json!({}),
        json!({"id": ""}),
        json!({"id": "has/slash"}),
        json!({"id": "ok", "extra": 1}),
    ] {
        let output = sandbox.run_json(&json!({
            "job": job,
            "tasks": [{"agent": "alpha", "prompt": "one"}]
        }));
        assert_eq!(output.status.code(), Some(2), "{job}: {output:?}");
        sandbox.assert_nothing_ran(&output);
    }
}
