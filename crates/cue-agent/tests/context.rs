//! Trace capture destinations and the child's `CUE_CONTEXT`, which are
//! independent of each other.

mod helpers;

use helpers::{Sandbox, receipt, run_of, spec};
use serde_json::json;

const MANIFEST: &str = r#"{
  "agents": {
    "explore": { "description": "Explores", "model": "anthropic/haiku" }
  }
}"#;

/// What the fake harness saw in `CUE_CONTEXT`.
fn child_context(sandbox: &Sandbox, run_id: &str) -> String {
    let path = sandbox.harness_log().join(format!("{run_id}.context"));
    std::fs::read_to_string(path)
        .expect("context record")
        .trim()
        .to_string()
}

#[test]
fn without_a_context_no_trace_is_written_and_the_child_env_is_inherited() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);

    let output = sandbox
        .cmd()
        .env("CUE_AGENT_CUE_BIN", sandbox.fake_cue())
        .env("CUE_CONTEXT", "ambient-context")
        .args(["run", "--json"])
        .arg(spec(&[("explore", "hello")]).to_string())
        .output()
        .expect("run cue-agent");
    assert!(output.status.success(), "{output:?}");

    let receipt = receipt(&output.stdout);
    assert!(receipt.get("context").is_none(), "{receipt}");
    let run = run_of(&receipt, "explore");
    assert_eq!(run["trace"], serde_json::Value::Null);
    assert_eq!(
        child_context(&sandbox, run["run_id"].as_str().unwrap()),
        "ambient-context",
        "an inherited CUE_CONTEXT is neither cleared nor used as a destination"
    );
    assert!(
        !sandbox.harness_log().join("cue.argv").exists(),
        "cue is not invoked when there is no context to write into"
    );
}

#[test]
fn a_capture_context_names_the_trace_but_never_assigns_cue_context() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    sandbox.init_git_repo("git@github.com:acme/widgets.git");
    sandbox.context("acme/widgets/auth-redesign");

    let output = sandbox
        .cmd()
        .env("CUE_AGENT_CUE_BIN", sandbox.fake_cue())
        .args(["run", "--json"])
        .arg(
            json!({
                "label": "Review the diff",
                "tasks": [{
                    "agent": "explore",
                    "prompt": "hello",
                    "context": "acme/widgets/auth-redesign"
                }]
            })
            .to_string(),
        )
        .output()
        .expect("run cue-agent");
    assert!(output.status.success(), "{output:?}");

    let receipt = receipt(&output.stdout);
    let run = run_of(&receipt, "explore");
    let run_id = run["run_id"].as_str().unwrap();
    assert_eq!(child_context(&sandbox, run_id), "<unset>");

    let name = format!(
        "agent/review-the-diff-{}/001-explore.md",
        receipt["batch_id"].as_str().unwrap()
    );
    assert_eq!(
        run["trace"],
        format!("acme/widgets/auth-redesign/trace/{name}")
    );

    let argv = sandbox.recorded_cue_argv();
    assert_eq!(argv[0], "-C");
    assert_eq!(argv[1], sandbox.project().display().to_string());
    assert_eq!(argv[2], "add");
    assert_eq!(argv[3], name, "{argv:?}");
    assert_eq!(argv[4], "--file");
    let context_at = argv
        .iter()
        .position(|arg| arg == "--context")
        .expect("--context");
    assert_eq!(argv[context_at + 1], "acme/widgets/auth-redesign");
    assert!(argv.contains(&"--type".to_string()));
    assert!(argv.contains(&"trace".to_string()));
    assert!(argv.contains(&"kind=agent-run".to_string()), "{argv:?}");
    assert!(argv.contains(&"agent=explore".to_string()), "{argv:?}");
    assert!(
        argv.contains(&"model=anthropic/haiku".to_string()),
        "{argv:?}"
    );
    assert!(argv.contains(&"outcome=completed".to_string()), "{argv:?}");
    assert!(argv.contains(&"turns=1".to_string()), "{argv:?}");
    assert!(argv.contains(&"tokens_input=11".to_string()), "{argv:?}");
    assert!(argv.contains(&"tokens_output=22".to_string()), "{argv:?}");
    assert!(
        argv.contains(&"harness_version=fake-pi 9.9.9".to_string()),
        "{argv:?}"
    );
    assert!(
        argv.contains(&"description=Review the diff".to_string()),
        "{argv:?}"
    );
    assert!(
        argv.iter().any(|arg| arg.starts_with("run_id=")),
        "{argv:?}"
    );
    assert!(
        !argv.iter().any(|arg| arg.starts_with("prompt=")),
        "the prompt is never frontmatter: {argv:?}"
    );

    let body = std::fs::read_to_string(sandbox.harness_log().join("cue.body")).expect("body");
    assert_eq!(
        body, "echo: hello",
        "the trace body is the final message verbatim, with nothing added"
    );
}

#[test]
fn a_failed_trace_write_fails_the_batch_but_keeps_the_completed_result() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    sandbox.init_git_repo("git@github.com:acme/widgets.git");
    sandbox.context("acme/widgets/auth");
    let failing_cue = sandbox.dir.path().join("failing-cue");
    helpers::write_executable(
        &failing_cue,
        "#!/usr/bin/env bash\nprintf 'store is read-only\\n' >&2\nexit 1\n",
    );
    let request = json!({"tasks": [
        {"agent": "explore", "prompt": "hello", "context": "acme/widgets/auth"}
    ]})
    .to_string();

    let output = sandbox
        .cmd()
        .env("CUE_AGENT_CUE_BIN", &failing_cue)
        .args(["run", "--json", &request])
        .output()
        .expect("run cue-agent");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let receipt = receipt(&output.stdout);
    let run = run_of(&receipt, "explore");
    assert_eq!(run["outcome"], "completed", "execution is not relabelled");
    assert_eq!(run["response"], "echo: hello");
    assert_eq!(run["trace"], serde_json::Value::Null);
    assert!(
        run["trace_error"]
            .as_str()
            .unwrap()
            .contains("store is read-only"),
        "{run}"
    );

    let human = sandbox
        .cmd()
        .env("CUE_AGENT_CUE_BIN", &failing_cue)
        .args(["run", &request])
        .output()
        .expect("run cue-agent");
    assert_eq!(human.status.code(), Some(1), "{human:?}");
    let stdout = String::from_utf8_lossy(&human.stdout);
    assert!(stdout.contains("trace error"), "{stdout}");
    assert!(stdout.contains("store is read-only"), "{stdout}");
    assert!(stdout.contains("echo: hello"), "{stdout}");
}

#[test]
fn tasks_choose_capture_contexts_and_cue_context_independently() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    sandbox.init_git_repo("git@github.com:acme/widgets.git");
    sandbox.context("acme/widgets/first");
    sandbox.context("acme/widgets/second");

    let output = sandbox
        .cmd()
        .env("CUE_AGENT_CUE_BIN", sandbox.fake_cue())
        .env("CUE_CONTEXT", "ambient")
        .args(["run", "--json"])
        .arg(
            json!({
                "defaults": {"context": "acme/widgets/first"},
                "tasks": [
                    {"agent": "explore", "prompt": "SILENT one"},
                    {"agent": "explore", "prompt": "SILENT two",
                     "context": "acme/widgets/second",
                     "env": {"CUE_CONTEXT": "explicit"}},
                    {"agent": "explore", "prompt": "SILENT three", "context": null,
                     "env": {"CUE_CONTEXT": null}}
                ]
            })
            .to_string(),
        )
        .output()
        .expect("run cue-agent");
    assert!(output.status.success(), "{output:?}");

    let receipt = receipt(&output.stdout);
    let runs = receipt["runs"].as_array().unwrap();
    let contexts: Vec<String> = runs
        .iter()
        .map(|run| child_context(&sandbox, run["run_id"].as_str().unwrap()))
        .collect();
    assert_eq!(contexts, ["ambient", "explicit", "<unset>"]);

    let destinations: Vec<serde_json::Value> = runs
        .iter()
        .map(|run| {
            let path = std::path::Path::new(run["run_path"].as_str().unwrap());
            let manifest: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path.join("manifest.json")).unwrap())
                    .unwrap();
            manifest["context"].clone()
        })
        .collect();
    assert_eq!(
        destinations,
        [
            json!("acme/widgets/first"),
            json!("acme/widgets/second"),
            serde_json::Value::Null
        ]
    );
}

#[test]
fn a_run_with_no_response_writes_no_trace_but_still_records_the_run() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    sandbox.init_git_repo("git@github.com:acme/widgets.git");
    sandbox.context("acme/widgets/auth");

    let output = sandbox
        .cmd()
        .env("CUE_AGENT_CUE_BIN", sandbox.fake_cue())
        .args(["run", "--json"])
        .arg(
            json!({"tasks": [{"agent": "explore", "prompt": "SILENT",
                              "context": "acme/widgets/auth"}]})
            .to_string(),
        )
        .output()
        .expect("run cue-agent");
    assert!(output.status.success(), "{output:?}");

    let run = receipt(&output.stdout);
    let run = run_of(&run, "explore");
    assert_eq!(run["trace"], serde_json::Value::Null);
    assert!(
        !sandbox.harness_log().join("cue.argv").exists(),
        "a trace whose body would be empty is skipped, not written empty"
    );
    let run_path = std::path::PathBuf::from(run["run_path"].as_str().unwrap());
    assert!(
        run_path.join("receipt.json").is_file(),
        "plane 2 still holds the run"
    );
}

#[test]
fn a_run_cut_short_after_a_tool_use_message_promotes_no_response() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    sandbox.init_git_repo("git@github.com:acme/widgets.git");
    sandbox.context("acme/widgets/auth");
    sandbox.install_pi(concat!(
        "#!/usr/bin/env bash\n",
        "[[ \"${1:-}\" == --version ]] && { echo 'fake-pi 9.9.9'; exit 0; }\n",
        "printf '%s\\n' '{\"type\":\"message_end\",\"message\":{\"role\":\"assistant\",",
        "\"stopReason\":\"toolUse\",\"content\":[{\"type\":\"text\",\"text\":\"Let me look\"},",
        "{\"type\":\"toolCall\",\"id\":\"t1\",\"name\":\"read\",\"arguments\":{}}],",
        "\"usage\":{\"input\":5,\"output\":7,\"cost\":{\"total\":0.25}}}}'\n",
        "exit 3\n",
    ));

    let output = sandbox
        .cmd()
        .env("CUE_AGENT_CUE_BIN", sandbox.fake_cue())
        .args(["run", "--json"])
        .arg(
            json!({"tasks": [{"agent": "explore", "prompt": "hello",
                              "context": "acme/widgets/auth"}]})
            .to_string(),
        )
        .output()
        .expect("run cue-agent");
    assert_eq!(output.status.code(), Some(1), "{output:?}");

    let receipt = receipt(&output.stdout);
    let run = run_of(&receipt, "explore");
    assert_eq!(run["outcome"], "failed", "{run}");
    assert_eq!(run["response"], "", "{run}");
    assert_eq!(run["trace"], serde_json::Value::Null, "{run}");
    assert_eq!(run["turns"], 1, "{run}");
    assert_eq!(run["tokens_input"], 5, "{run}");
    assert_eq!(run["tokens_output"], 7, "{run}");
    assert!(
        !sandbox.harness_log().join("cue.argv").exists(),
        "a tool-use preamble is never promoted into a trace"
    );
}

#[test]
fn the_trace_lands_in_the_store_with_its_frontmatter_stamped() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    sandbox.init_git_repo("git@github.com:acme/widgets.git");
    let cue = helpers::real_cue();

    let created = std::process::Command::new(&cue)
        .args(["context", "create", "delegation", "--title", "Delegation"])
        .current_dir(sandbox.project())
        .env("CUE_STORE", sandbox.store())
        .output()
        .expect("cue context create");
    assert!(created.status.success(), "{created:?}");

    let output = sandbox
        .cmd()
        .env("CUE_AGENT_CUE_BIN", &cue)
        .args(["run", "--json"])
        .arg(
            json!({"label": "Consult", "tasks": [{"agent": "explore", "prompt": "hello",
                   "context": "acme/widgets/delegation"}]})
            .to_string(),
        )
        .output()
        .expect("run cue-agent");
    assert!(output.status.success(), "{output:?}");

    let receipt = receipt(&output.stdout);
    let run = run_of(&receipt, "explore");
    assert_eq!(run["trace_error"], serde_json::Value::Null, "{run}");
    let address = run["trace"].as_str().expect("trace address");
    assert!(
        address.starts_with("acme/widgets/delegation/trace/"),
        "{address}"
    );
    let artifact = sandbox.store().join(address);
    let written = std::fs::read_to_string(&artifact)
        .unwrap_or_else(|err| panic!("missing trace {}: {err}", artifact.display()));

    assert!(written.contains("kind: agent-run"), "{written}");
    assert!(written.contains("agent: explore"), "{written}");
    assert!(written.contains("repo_id: acme/widgets"), "{written}");
    assert!(written.contains("commit_hash:"), "{written}");
    assert!(
        written.ends_with("echo: hello"),
        "the body is the final message verbatim: {written}"
    );
}
