//! The run specification interface: transports, admission, effective
//! prompts, environment and working directories.

mod helpers;

use helpers::{Sandbox, receipt, spec};
use serde_json::json;
use std::io::Write;
use std::process::Stdio;

const MANIFEST: &str = r#"{
  "agents": {
    "explore": { "model": "anthropic/haiku", "system_prompt": "You explore." },
    "bare": {}
  }
}"#;

fn sandbox() -> Sandbox {
    let sandbox = Sandbox::new();
    sandbox.global_manifest(MANIFEST);
    sandbox
}

/// Run with the specification on standard input.
fn run_stdin(sandbox: &Sandbox, args: &[&str], input: &str) -> std::process::Output {
    let mut child = sandbox
        .cmd()
        .arg("run")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn cue-agent");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait")
}

fn responses(output: &std::process::Output) -> Vec<String> {
    receipt(&output.stdout)["runs"]
        .as_array()
        .expect("runs")
        .iter()
        .map(|run| run["response"].as_str().unwrap().to_string())
        .collect()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn the_specification_arrives_inline_on_stdin_or_from_a_file() {
    let sandbox = sandbox();
    let request = spec(&[("bare", "one"), ("explore", "two")]).to_string();

    let inline = sandbox
        .cmd()
        .args(["run", "--json", &request])
        .output()
        .unwrap();
    assert!(inline.status.success(), "{inline:?}");
    assert_eq!(responses(&inline), ["echo: one", "echo: two"]);

    let stdin = run_stdin(&sandbox, &["--json", "-"], &request);
    assert!(stdin.status.success(), "{stdin:?}");
    assert_eq!(responses(&stdin), ["echo: one", "echo: two"]);

    let path = sandbox.project().join("request.json");
    std::fs::write(&path, &request).unwrap();
    let file = sandbox
        .cmd()
        .args(["run", "--json", "--spec"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(file.status.success(), "{file:?}");
    assert_eq!(responses(&file), ["echo: one", "echo: two"]);

    // A relative --spec path is relative to the invocation directory.
    let relative = sandbox
        .cmd()
        .args(["run", "--json", "--spec", "request.json"])
        .output()
        .unwrap();
    assert!(relative.status.success(), "{relative:?}");
}

#[test]
fn exactly_one_specification_source_is_required() {
    let request = spec(&[("bare", "one")]).to_string();
    let sandbox = sandbox();
    let path = sandbox.project().join("request.json");
    std::fs::write(&path, &request).unwrap();

    let both = sandbox
        .cmd()
        .args(["run", "--json", &request, "--spec"])
        .arg(&path)
        .output()
        .unwrap();
    sandbox.assert_nothing_ran(&both);

    let none = sandbox.cmd().args(["run", "--json"]).output().unwrap();
    sandbox.assert_nothing_ran(&none);

    let bare = sandbox.cmd().arg("run").output().unwrap();
    sandbox.assert_nothing_ran(&bare);

    let two = sandbox
        .cmd()
        .args(["run", "--json", &request, &request])
        .output()
        .unwrap();
    sandbox.assert_nothing_ran(&two);
}

#[test]
fn the_removed_prototype_interface_is_rejected() {
    for args in [
        &["run", "bare", "--prompt", "hi"][..],
        &["run", "bare", "-p", "hi"],
        &["run", "bare", "--prompt-file", "p.md"],
        &["run", "--batch", "batch.json"],
        &[
            "run",
            "--harness",
            "pi",
            r#"{"tasks":[{"agent":"bare","prompt":"hi"}]}"#,
        ],
        &[
            "run",
            "--cwd",
            "/tmp",
            r#"{"tasks":[{"agent":"bare","prompt":"hi"}]}"#,
        ],
        &[
            "run",
            "--context",
            "a/b/c",
            r#"{"tasks":[{"agent":"bare","prompt":"hi"}]}"#,
        ],
        &[
            "run",
            "--label",
            "l",
            r#"{"tasks":[{"agent":"bare","prompt":"hi"}]}"#,
        ],
        &[
            "run",
            "--dry-run",
            r#"{"tasks":[{"agent":"bare","prompt":"hi"}]}"#,
        ],
        // Positional fan-out: a bare agent name is not JSON and is never guessed
        // to be a path.
        &["run", "bare"],
        &["run", "request.json"],
    ] {
        let sandbox = sandbox();
        std::fs::write(
            sandbox.project().join("request.json"),
            spec(&[("bare", "hi")]).to_string(),
        )
        .unwrap();
        std::fs::write(sandbox.project().join("p.md"), "hi").unwrap();
        let output = sandbox.cmd().args(args).output().unwrap();
        sandbox.assert_nothing_ran(&output);
        assert!(!stderr(&output).is_empty(), "{args:?}");
    }
}

#[test]
fn invalid_requests_are_refused_whole_before_any_side_effect() {
    let sandbox = sandbox();
    sandbox.context("acme/widgets/real");
    let ok = json!({"agent": "bare", "prompt": "fine"});
    for (bad, expected) in [
        (json!({"agent": "ghost", "prompt": "p"}), "ghost"),
        (json!({"agent": "bare"}), "tasks[1].prompt"),
        (
            json!({"agent": "bare", "prompt": "p", "timeout": 1}),
            "timeout",
        ),
        (
            json!({"agent": "bare", "prompt": "p", "cwd": "relative"}),
            "tasks[1].cwd",
        ),
        (
            json!({"agent": "bare", "prompt": {"file": "missing.md"}}),
            "missing.md",
        ),
        (
            json!({"agent": "bare", "prompt": "p", "context": "acme/widgets/none"}),
            "tasks[1].context",
        ),
        (
            json!({"agent": "bare", "prompt": "p", "context": "real"}),
            "tasks[1].context",
        ),
        (json!({"agent": "bare", "prompt": "p", "bogus": 1}), "bogus"),
        (
            json!({"agent": "bare", "prompt": "p",
                   "worktree": {"base": "main", "ephemeral": true}}),
            "worktree_root",
        ),
        (
            json!({"agent": "bare", "prompt": "p",
                   "worktree": {"base": "main", "path": "/tmp/wt"}}),
            "tasks[1].worktree",
        ),
    ] {
        let output = sandbox.run_json(&json!({"tasks": [ok, bad]}));
        sandbox.assert_nothing_ran(&output);
        let error = stderr(&output);
        assert!(error.contains(expected), "{bad}: {error}");
    }
    for (request, expected) in [
        ("not json", "JSON"),
        (r#"[{"agent": "bare", "prompt": "p"}]"#, "object"),
        (r#"{"tasks": []}"#, "at least one"),
        (r#"{"runs": []}"#, "runs"),
    ] {
        let output = sandbox
            .cmd()
            .args(["run", "--json", request])
            .output()
            .unwrap();
        sandbox.assert_nothing_ran(&output);
        assert!(stderr(&output).contains(expected), "{request}");
    }
    let output = sandbox
        .cmd()
        .args(["run", "--json", "--spec", "missing.json"])
        .output()
        .unwrap();
    sandbox.assert_nothing_ran(&output);
    assert!(stderr(&output).contains("missing.json"), "{output:?}");
}

#[test]
fn file_references_in_a_spec_file_resolve_beside_the_file_it_was_named_by() {
    let sandbox = sandbox();
    let specs = sandbox.project().join("specs");
    let elsewhere = sandbox.project().join("elsewhere");
    std::fs::create_dir_all(&specs).unwrap();
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::write(specs.join("prompt.md"), "from the spec directory").unwrap();
    std::fs::write(elsewhere.join("prompt.md"), "from the link target").unwrap();
    std::fs::write(
        sandbox.project().join("prompt.md"),
        "from the invocation directory",
    )
    .unwrap();
    let request = json!({"tasks": [{
        "agent": "bare", "prompt": {"file": "prompt.md"}, "cwd": elsewhere
    }]})
    .to_string();
    std::fs::write(elsewhere.join("request.json"), &request).unwrap();
    // The spec is reached through a symlink: its references resolve against
    // the directory the caller named, not wherever the link points.
    std::os::unix::fs::symlink(elsewhere.join("request.json"), specs.join("request.json")).unwrap();

    let output = sandbox
        .cmd()
        .args(["run", "--json", "--spec", "specs/request.json"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(responses(&output), ["echo: from the spec directory"]);
}

#[test]
fn file_references_in_inline_and_stdin_specs_resolve_against_the_invocation() {
    let sandbox = sandbox();
    let target = sandbox.project().join("target");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(sandbox.project().join("prompt.md"), "from the invocation").unwrap();
    std::fs::write(target.join("prompt.md"), "from the task cwd").unwrap();
    let request = json!({"tasks": [{
        "agent": "bare", "prompt": {"file": "prompt.md"}, "cwd": target
    }]});

    let inline = sandbox.run_json(&request);
    assert!(inline.status.success(), "{inline:?}");
    assert_eq!(responses(&inline), ["echo: from the invocation"]);

    let stdin = run_stdin(&sandbox, &["--json", "-"], &request.to_string());
    assert!(stdin.status.success(), "{stdin:?}");
    assert_eq!(responses(&stdin), ["echo: from the invocation"]);
}

#[test]
fn repeated_agents_run_independently_and_report_in_specification_order() {
    let sandbox = sandbox();
    let output = sandbox.run_json(&json!({
        "label": "sweep",
        "defaults": {"prompt": "shared"},
        "tasks": [
            {"agent": "explore", "prompt": "SLEEP=0.6"},
            {"agent": "bare"},
            {"agent": "explore", "prompt": "SLEEP=0.1"},
            {"agent": "explore", "label": "third explore"}
        ]
    }));
    assert!(output.status.success(), "{output:?}");
    let receipt = receipt(&output.stdout);
    let runs = receipt["runs"].as_array().unwrap();
    let agents: Vec<_> = runs.iter().map(|run| run["agent"].clone()).collect();
    assert_eq!(agents, ["explore", "bare", "explore", "explore"]);
    assert_eq!(
        responses(&output),
        ["slept 0.6", "echo: shared", "slept 0.1", "echo: shared"]
    );
    let ids: std::collections::BTreeSet<_> = runs
        .iter()
        .map(|run| run["run_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 4, "every execution has its own identity");
    let paths: std::collections::BTreeSet<_> = runs
        .iter()
        .map(|run| run["run_path"].as_str().unwrap())
        .collect();
    assert_eq!(paths.len(), 4);
}

#[test]
fn defaults_and_task_overrides_reach_the_harness() {
    let sandbox = sandbox();
    let output = sandbox.run_json(&json!({
        "defaults": {"model": "default/model", "tools": [], "thinking": "low"},
        "tasks": [
            {"agent": "explore", "prompt": "a"},
            {"agent": "explore", "prompt": "b", "model": null, "tools": ["grep"],
             "system_prompt": null, "thinking": null}
        ]
    }));
    assert!(output.status.success(), "{output:?}");
    let receipt = receipt(&output.stdout);
    let runs = receipt["runs"].as_array().unwrap();

    let first = runs[0]["run_id"].as_str().unwrap();
    let path = std::path::Path::new(runs[0]["run_path"].as_str().unwrap());
    assert_eq!(
        sandbox.recorded_argv(first),
        [
            "--mode",
            "json",
            "-p",
            "--no-session",
            "--session-id",
            first,
            "--model",
            "default/model",
            "--thinking",
            "low",
            "--no-tools",
            "--append-system-prompt",
            &path.join("system-prompt.md").display().to_string(),
            "a",
        ]
    );
    assert_eq!(runs[0]["model"], "default/model");

    let second = runs[1]["run_id"].as_str().unwrap();
    assert_eq!(
        sandbox.recorded_argv(second),
        [
            "--mode",
            "json",
            "-p",
            "--no-session",
            "--session-id",
            second,
            "--tools",
            "grep",
            "b",
        ]
    );
}

#[test]
fn env_inherits_then_overlays_defaults_then_tasks_and_null_removes() {
    let sandbox = sandbox();
    let output = sandbox
        .cmd()
        .env("AMBIENT", "inherited")
        .env("SHADOWED", "inherited")
        .args(["run", "--json"])
        .arg(
            json!({
                "defaults": {"env": {"SHARED": "default", "SHADOWED": "default",
                                     "AMBIENT": null}},
                "tasks": [
                    {"agent": "bare", "prompt": "one"},
                    {"agent": "bare", "prompt": "two",
                     "env": {"SHADOWED": "task", "AMBIENT": "restored", "SHARED": null}},
                    {"agent": "bare", "prompt": "three", "env": null}
                ]
            })
            .to_string(),
        )
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let receipt = receipt(&output.stdout);
    let envs: Vec<_> = receipt["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|run| sandbox.recorded_env(run["run_id"].as_str().unwrap()))
        .collect();
    let get = |index: usize, key: &str| envs[index].get(key).map(String::as_str);

    assert_eq!(get(0, "SHARED"), Some("default"));
    assert_eq!(get(0, "SHADOWED"), Some("default"));
    assert_eq!(
        get(0, "AMBIENT"),
        None,
        "null removes an inherited variable"
    );

    assert_eq!(get(1, "SHARED"), None, "a task null removes a default");
    assert_eq!(get(1, "SHADOWED"), Some("task"));
    assert_eq!(get(1, "AMBIENT"), Some("restored"));

    // A whole-field null drops the default overlay: the inherited
    // environment returns unchanged.
    assert_eq!(get(2, "SHARED"), None);
    assert_eq!(get(2, "SHADOWED"), Some("inherited"));
    assert_eq!(get(2, "AMBIENT"), Some("inherited"));
    for env in &envs {
        assert!(env.contains_key("FAKE_HARNESS_LOG"), "inheritance is kept");
    }
}

#[test]
fn environment_values_are_never_persisted() {
    let sandbox = sandbox();
    let secret = "s3cr3t-value-0b1f";
    let output = sandbox.run_json(&json!({
        "tasks": [{"agent": "bare", "prompt": "hi", "env": {"API_TOKEN": secret}}]
    }));
    assert!(output.status.success(), "{output:?}");
    let receipt = receipt(&output.stdout);
    let run = &receipt["runs"][0];
    assert_eq!(
        sandbox.recorded_env(run["run_id"].as_str().unwrap())["API_TOKEN"],
        secret
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains(secret));
    let mut stack = vec![sandbox.state()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                assert!(
                    !String::from_utf8_lossy(&bytes).contains(secret),
                    "{} holds an environment value",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn cwd_is_per_task_and_defaults_to_the_invocation_directory() {
    let sandbox = sandbox();
    let shared = sandbox.project().join("shared");
    let own = sandbox.dir.path().join("elsewhere");
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::create_dir_all(&own).unwrap();
    let output = sandbox.run_json(&json!({
        "defaults": {"cwd": shared},
        "tasks": [
            {"agent": "bare", "prompt": "a"},
            {"agent": "bare", "prompt": "b", "cwd": own},
            {"agent": "bare", "prompt": "c", "cwd": null}
        ]
    }));
    assert!(output.status.success(), "{output:?}");
    let receipt = receipt(&output.stdout);
    let cwds: Vec<_> = receipt["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|run| sandbox.recorded_cwd(run["run_id"].as_str().unwrap()))
        .collect();
    assert_eq!(
        cwds,
        [
            shared.display().to_string(),
            own.display().to_string(),
            sandbox.project().display().to_string()
        ]
    );
}

#[test]
fn agents_come_from_the_invocation_manifest_not_the_task_cwd() {
    let sandbox = Sandbox::new();
    sandbox.local_manifest(r#"{"agents": {"bare": {"model": "invocation/model"}}}"#);
    let other = sandbox.dir.path().join("other-repo");
    std::fs::create_dir_all(other.join(".git")).unwrap();
    std::fs::write(
        other.join("cue-agent.json"),
        r#"{"timeout": 1, "agents": {"bare": {"model": "other/model"}, "local-only": {}}}"#,
    )
    .unwrap();

    let output = sandbox.run_json(&json!({
        "tasks": [{"agent": "bare", "prompt": "SLEEP=1.5", "cwd": other}]
    }));
    assert!(output.status.success(), "{output:?}");
    let receipt = receipt(&output.stdout);
    let run = &receipt["runs"][0];
    assert_eq!(run["model"], "invocation/model");
    assert_eq!(run["outcome"], "completed", "the other timeout is not used");

    let sessions = sandbox.recorded_sessions().len();
    let output = sandbox.run_json(&json!({
        "tasks": [{"agent": "local-only", "prompt": "p", "cwd": other}]
    }));
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("Unknown agent 'local-only'"));
    assert_eq!(sandbox.recorded_sessions().len(), sessions);
}

#[test]
fn pi_is_looked_up_on_each_tasks_effective_path() {
    let sandbox = sandbox();
    let alt = sandbox.dir.path().join("alt-bin");
    std::fs::create_dir_all(&alt).unwrap();
    helpers::write_executable(
        &alt.join("pi"),
        &format!(
            "#!/usr/bin/env bash\nFAKE_PI_VERSION='alt-pi 1.0' exec '{}' \"$@\"\n",
            sandbox.harness().display()
        ),
    );
    let output = sandbox.run_json(&json!({"tasks": [
        {"agent": "bare", "prompt": "default"},
        {"agent": "bare", "prompt": "alternative", "env": {"PATH": sandbox.path_with(&alt)}}
    ]}));
    assert!(output.status.success(), "{output:?}");
    let receipt = receipt(&output.stdout);
    let runs = receipt["runs"].as_array().unwrap();
    let recorded: Vec<serde_json::Value> = runs
        .iter()
        .map(|run| {
            let path = std::path::Path::new(run["run_path"].as_str().unwrap());
            serde_json::from_slice(&std::fs::read(path.join("manifest.json")).unwrap()).unwrap()
        })
        .collect();
    assert_eq!(
        recorded[0]["harness_path"],
        sandbox.pi_dir().join("pi").display().to_string()
    );
    assert_eq!(recorded[0]["harness_version"], "fake-pi 9.9.9");
    assert_eq!(
        recorded[1]["harness_path"],
        alt.join("pi").display().to_string()
    );
    assert_eq!(recorded[1]["harness_version"], "alt-pi 1.0");
    assert!(
        receipt["harness_version"].is_null(),
        "no single version describes the batch: {receipt}"
    );
}

#[test]
fn the_version_is_probed_with_each_tasks_own_cwd_and_env() {
    let sandbox = sandbox();
    // One executable whose answer depends on the environment and the cwd.
    sandbox.install_pi(&format!(
        "#!/usr/bin/env bash\nif [[ $1 == --version ]]; then printf '%s in %s\\n' \"${{PI_FLAVOUR:-plain}}\" \"${{PWD##*/}}\"; exit 0; fi\nexec '{}' \"$@\"\n",
        sandbox.harness().display()
    ));
    let left = sandbox.project().join("left");
    let right = sandbox.project().join("right");
    std::fs::create_dir_all(&left).unwrap();
    std::fs::create_dir_all(&right).unwrap();
    let output = sandbox.run_json(&json!({"tasks": [
        {"agent": "bare", "prompt": "a", "cwd": left},
        {"agent": "bare", "prompt": "b", "cwd": left, "env": {"PI_FLAVOUR": "spicy"}},
        {"agent": "bare", "prompt": "c", "cwd": right},
        {"agent": "bare", "prompt": "d", "cwd": left}
    ]}));
    assert!(output.status.success(), "{output:?}");
    let receipt = receipt(&output.stdout);
    let versions: Vec<serde_json::Value> = receipt["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|run| {
            let path = std::path::Path::new(run["run_path"].as_str().unwrap());
            let manifest: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path.join("manifest.json")).unwrap())
                    .unwrap();
            manifest["harness_version"].clone()
        })
        .collect();
    assert_eq!(
        versions,
        [
            json!("plain in left"),
            json!("spicy in left"),
            json!("plain in right"),
            json!("plain in left")
        ]
    );
    assert!(receipt["harness_version"].is_null(), "{receipt}");
    // The flavour reached the probe, but no environment value is recorded.
    for run in receipt["runs"].as_array().unwrap() {
        let path = std::path::Path::new(run["run_path"].as_str().unwrap());
        let manifest = std::fs::read_to_string(path.join("manifest.json")).unwrap();
        assert!(!manifest.contains("PI_FLAVOUR"), "{manifest}");
    }
}

#[test]
fn human_output_reports_each_run_and_makes_failures_visible() {
    let sandbox = sandbox();
    let output = sandbox
        .cmd()
        .arg("run")
        .arg(spec(&[("bare", "hello"), ("explore", "FAIL=3")]).to_string())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        serde_json::from_str::<serde_json::Value>(&stdout).is_err(),
        "{stdout}"
    );
    assert!(stdout.contains("bare"), "{stdout}");
    assert!(stdout.contains("completed"), "{stdout}");
    assert!(stdout.contains("echo: hello"), "{stdout}");
    assert!(stdout.contains("explore"), "{stdout}");
    assert!(stdout.contains("failed"), "{stdout}");
    assert!(stdout.contains("status 3"), "{stdout}");
    assert!(stdout.contains("fake harness exploded"), "{stdout}");
}
