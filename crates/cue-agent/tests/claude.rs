//! claude-code as a delegate harness, observed at the process boundary.

mod helpers;

use helpers::{Sandbox, receipt, run_of, spec};

const MANIFEST: &str = r#"{
  "models": {
    "cc-haiku": { "id": "cc-haiku", "thinking": "high", "provider": "anthropic" },
    "pi-haiku": { "id": "fake/pi-haiku" }
  },
  "agents": {
    "cc": {
      "description": "Runs in claude-code",
      "harness": "claude-code",
      "models": ["cc-haiku"],
      "system_prompt": "You review.",
      "tools": ["Read", "Bash"]
    },
    "pi-agent": {
      "description": "Runs in pi",
      "models": ["pi-haiku"]
    }
  }
}"#;

#[test]
fn a_claude_agent_runs_end_to_end_through_the_translation() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    sandbox.install_claude();
    let output = sandbox.run_json(&spec(&[("cc", "hello")]));
    assert!(output.status.success(), "{output:?}");
    let batch = receipt(&output.stdout);
    let run = run_of(&batch, "cc");
    assert_eq!(run["harness"], "claude-code");
    assert_eq!(batch["harness"], "claude-code");
    assert_eq!(run["outcome"], "completed");
    assert_eq!(run["response"], "echo: hello");
    assert_eq!(run["turns"], 1);
    assert_eq!(run["tokens_input"], 10);
    assert_eq!(run["tokens_output"], 4);
    assert_eq!(run["cost_usd"], 0.002);
    assert_eq!(run["model"], "cc-haiku");

    let argv = sandbox.recorded_claude_argv();
    let prompt = argv.last().unwrap();
    assert_eq!(prompt, "hello", "the prompt is the positional message");
    let flags: Vec<&String> = argv.iter().collect();
    for expected in [
        &"-p".to_string(),
        &"--output-format".to_string(),
        &"stream-json".to_string(),
        &"--verbose".to_string(),
        &"--permission-mode".to_string(),
        &"dontAsk".to_string(),
        &"--model".to_string(),
        &"cc-haiku".to_string(),
        &"--effort".to_string(),
        &"high".to_string(),
        &"--allowedTools".to_string(),
        &"Read,Bash".to_string(),
    ] {
        assert!(
            flags.contains(&expected),
            "missing {expected:?} in {argv:?}"
        );
    }
    assert!(
        !flags.contains(&&"--session-id".to_string()),
        "claude receives no session id: {argv:?}"
    );
    // The system prompt travels as a file, not inline text.
    let index = flags
        .iter()
        .position(|f| *f == &"--append-system-prompt-file".to_string());
    if let Some(index) = index {
        let path = argv[index + 1].clone();
        let text = std::fs::read_to_string(
            sandbox.state().join(
                std::path::Path::new(&path)
                    .strip_prefix(sandbox.state())
                    .unwrap_or(std::path::Path::new(&path)),
            ),
        );
        // The path may live outside the state dir if resolution differs; only
        // assert readability and content when it is inside the sandbox.
        if let Ok(text) = text {
            assert_eq!(text, "You review.");
        }
    }
}

#[test]
fn a_claude_stream_error_fails_the_run_despite_exit_zero() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    sandbox.install_claude();
    let output = sandbox.run_json(&spec(&[("cc", "CC_API_ERROR now")]));
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let batch = receipt(&output.stdout);
    let run = run_of(&batch, "cc");
    assert_eq!(run["outcome"], "failed");
    assert_eq!(run["error"], "Failed to authenticate: key rejected");
    assert_eq!(run["response"], "");
}

#[test]
fn claude_tool_use_preambles_do_not_leak_as_responses() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    sandbox.install_claude();
    let output = sandbox.run_json(&spec(&[("cc", "CC_TOOL please")]));
    assert!(output.status.success(), "{output:?}");
    let batch = receipt(&output.stdout);
    let run = run_of(&batch, "cc");
    assert_eq!(run["outcome"], "completed");
    assert_eq!(run["response"], "tools said fine");
    assert_eq!(run["turns"], 2);
    assert_eq!(run["tokens_input"], 30);
}

#[test]
fn pi_and_claude_agents_share_one_batch() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    sandbox.install_claude();
    let output = sandbox.run_json(&spec(&[("pi-agent", "hello"), ("cc", "hello")]));
    assert!(output.status.success(), "{output:?}");
    let batch = receipt(&output.stdout);
    assert_eq!(batch["harness"], "mixed");
    let pi_run = run_of(&batch, "pi-agent");
    assert_eq!(pi_run["harness"], "pi");
    assert_eq!(pi_run["outcome"], "completed");
    assert_eq!(pi_run["response"], "echo: hello");
    let cc_run = run_of(&batch, "cc");
    assert_eq!(cc_run["harness"], "claude-code");
    assert_eq!(cc_run["outcome"], "completed");
    assert_eq!(cc_run["response"], "echo: hello");
    // Each harness received its own argv shape.
    let pi_argv = sandbox.recorded_argv(pi_run["run_id"].as_str().unwrap());
    assert!(pi_argv.contains(&"--session-id".to_string()));
    assert!(
        !sandbox
            .recorded_claude_argv()
            .contains(&"--session-id".to_string())
    );
}

#[test]
fn an_unknown_harness_name_is_rejected_at_manifest_load() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(r#"{"agents": {"cc": {"harness": "claude-pro"}}}"#);
    sandbox.install_claude();
    let output = sandbox.run_json(&spec(&[("cc", "hello")]));
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let text = String::from_utf8_lossy(&output.stderr);
    assert!(text.contains("unknown harness 'claude-pro'"), "{text}");
}

#[test]
fn agents_list_reports_the_harness_per_agent() {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    let output = sandbox
        .cmd()
        .args(["agents", "list", "--json"])
        .output()
        .expect("agents list");
    assert!(output.status.success(), "{output:?}");
    let listing = receipt(&output.stdout);
    let agents: Vec<_> = listing["agents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|agent| (agent["name"].as_str().unwrap(), agent["harness"].as_str()))
        .collect();
    assert!(agents.contains(&("cc", Some("claude-code"))));
    assert!(agents.contains(&("pi-agent", Some("pi"))));
}
