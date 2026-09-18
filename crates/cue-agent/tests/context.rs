//! Context propagation and the trace artifact.

mod helpers;

use helpers::{Sandbox, receipt, run_of};

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
fn without_a_context_the_child_gets_none_and_no_trace_is_written() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);

    let output = sandbox
        .cmd()
        .env("CUE_AGENT_CUE_BIN", sandbox.fake_cue())
        .args(["run", "explore", "--prompt", "hello"])
        .output()
        .expect("run cue-agent");
    assert!(output.status.success(), "{output:?}");

    let receipt = receipt(&output.stdout);
    assert_eq!(receipt["context"], serde_json::Value::Null);
    let run = run_of(&receipt, "explore");
    assert_eq!(run["trace"], serde_json::Value::Null);
    assert_eq!(
        child_context(&sandbox, run["run_id"].as_str().unwrap()),
        "<unset>",
        "an absent context is removed from the child environment, never inherited"
    );
    assert!(
        !sandbox.harness_log().join("cue.argv").exists(),
        "cue is not invoked when there is no context to write into"
    );
}

#[test]
fn an_explicit_context_reaches_the_child_and_names_the_trace() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    sandbox.init_git_repo("git@github.com:acme/widgets.git");

    let output = sandbox
        .cmd()
        .env("CUE_AGENT_CUE_BIN", sandbox.fake_cue())
        .args([
            "run",
            "explore",
            "--prompt",
            "hello",
            "--context",
            "auth-redesign",
            "--label",
            "Review the diff",
        ])
        .output()
        .expect("run cue-agent");
    assert!(output.status.success(), "{output:?}");

    let receipt = receipt(&output.stdout);
    assert_eq!(receipt["context"], "auth-redesign");
    let run = run_of(&receipt, "explore");
    let run_id = run["run_id"].as_str().unwrap();
    assert_eq!(child_context(&sandbox, run_id), "auth-redesign");

    let address = run["trace"].as_str().expect("trace address");
    assert!(
        address.starts_with("acme/widgets/auth-redesign/trace/agent/review-the-diff-explore-"),
        "{address}"
    );

    let argv = sandbox.recorded_cue_argv();
    assert_eq!(argv[0], "-C");
    assert_eq!(argv[1], sandbox.project().display().to_string());
    assert_eq!(argv[2], "add");
    assert!(
        argv[3].starts_with("agent/review-the-diff-explore-") && argv[3].ends_with(".md"),
        "{argv:?}"
    );
    assert_eq!(argv[4], "--file");
    assert!(argv.contains(&"--type".to_string()));
    assert!(argv.contains(&"trace".to_string()));
    assert!(argv.contains(&"--context".to_string()));
    assert!(argv.contains(&"auth-redesign".to_string()));
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
fn an_ambient_cue_context_is_inherited_when_no_context_is_passed() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    sandbox.init_git_repo("git@github.com:acme/widgets.git");

    let output = sandbox
        .cmd()
        .env("CUE_CONTEXT", "ambient-context")
        .env("CUE_AGENT_CUE_BIN", sandbox.fake_cue())
        .args(["run", "explore", "--prompt", "hello"])
        .output()
        .expect("run cue-agent");
    assert!(output.status.success(), "{output:?}");

    let receipt = receipt(&output.stdout);
    assert_eq!(receipt["context"], "ambient-context");
    let run_id = run_of(&receipt, "explore")["run_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(child_context(&sandbox, &run_id), "ambient-context");
}

#[test]
fn a_run_with_no_response_writes_no_trace_but_still_records_the_run() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    sandbox.init_git_repo("git@github.com:acme/widgets.git");

    let output = sandbox
        .cmd()
        .env("CUE_AGENT_CUE_BIN", sandbox.fake_cue())
        .args(["run", "explore", "--prompt", "SILENT", "--context", "auth"])
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
        .args([
            "run",
            "explore",
            "--prompt",
            "hello",
            "--context",
            "delegation",
            "--label",
            "Consult",
        ])
        .output()
        .expect("run cue-agent");
    assert!(output.status.success(), "{output:?}");

    let receipt = receipt(&output.stdout);
    let run = run_of(&receipt, "explore");
    let address = run["trace"].as_str().expect("trace address");
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
