mod helpers;

use std::fs;

#[test]
fn test_list_empty_repo() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // 1. Initialize mem
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 2. List (should be empty)
    let output = env
        .command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("list")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(output.is_empty());

    Ok(())
}

// ── Frontmatter tests ────────────────────────────────────────────────────────

#[test]
fn test_list_frontmatter_flag_implies_json() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("index.md")
        .arg("no frontmatter here")
        .assert()
        .success();

    // --frontmatter without --json should still produce valid JSON
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--frontmatter")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: serde_json::Value = serde_json::from_str(&output)?;
    assert!(json.is_array());

    Ok(())
}

#[test]
fn test_list_frontmatter_absent_when_no_flag() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Write file with frontmatter directly to bypass clap argument parsing of "---"
    let artifact_path = env.root().join(".test-mem/main/spec/index.md");
    fs::create_dir_all(artifact_path.parent().unwrap())?;
    fs::write(&artifact_path, "---\nstatus: active\n---\n# Hello")?;

    // Without --frontmatter, the field should be absent
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--json")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: serde_json::Value = serde_json::from_str(&output)?;
    let item = &json.as_array().unwrap()[0];

    assert!(item.get("frontmatter").is_none());

    Ok(())
}

#[test]
fn test_list_frontmatter_parsed_correctly() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Write file with frontmatter directly to bypass clap argument parsing of "---"
    let artifact_path = env.root().join(".test-mem/main/spec/index.md");
    fs::create_dir_all(artifact_path.parent().unwrap())?;
    fs::write(
        &artifact_path,
        "---\nstatus: active\npriority: high\n---\n# Body here",
    )?;

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--frontmatter")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: serde_json::Value = serde_json::from_str(&output)?;
    let item = &json.as_array().unwrap()[0];

    let fm = item.get("frontmatter").expect("frontmatter field missing");
    assert_eq!(fm["status"], "active");
    assert_eq!(fm["priority"], "high");

    Ok(())
}

#[test]
fn test_list_frontmatter_absent_for_file_without_frontmatter() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    let content = "# No frontmatter in this file";
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("index.md")
        .arg(content)
        .assert()
        .success();

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--frontmatter")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: serde_json::Value = serde_json::from_str(&output)?;
    let item = &json.as_array().unwrap()[0];

    // File has no frontmatter — field should be absent
    assert!(item.get("frontmatter").is_none());

    Ok(())
}

#[test]
fn test_list_frontmatter_malformed_unclosed_fence() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Opening fence but no closing fence — write directly to avoid clap parsing "---"
    let artifact_path = env.root().join(".test-mem/main/spec/index.md");
    fs::create_dir_all(artifact_path.parent().unwrap())?;
    fs::write(
        &artifact_path,
        "---\nstatus: active\n# Body starts here without closing fence",
    )?;

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--frontmatter")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: serde_json::Value = serde_json::from_str(&output)?;
    // Should succeed (not crash), with frontmatter absent
    assert!(json.is_array());
    let item = &json.as_array().unwrap()[0];
    assert!(item.get("frontmatter").is_none());

    Ok(())
}

#[test]
fn test_list_from_subdirectory() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // 1. Initialize
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 2. Add a root file (stable anchor)
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("index.md")
        .arg("content")
        .assert()
        .success();

    // 3. Create a subdirectory and run list from there
    let sub = env.root().join("src/nested");
    std::fs::create_dir_all(&sub)?;

    let output = String::from_utf8(
        env.command()
            .current_dir(&sub)
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    // Path should still be relative to git root, NOT to the subdirectory
    assert_eq!(output.trim(), ".test-mem/main/spec/index.md");

    Ok(())
}

#[test]
fn test_list_ignores_shallow_paths() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // 1. Initialize
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 2. Create a file directly under the branch dir (invalid depth)
    let branch_dir = env.root().join(".test-mem/main");
    std::fs::create_dir_all(&branch_dir)?;
    let invalid_file = branch_dir.join("README.md");
    std::fs::write(invalid_file, "invalid")?;

    // 3. Add a valid file
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("index.md")
        .arg("content")
        .assert()
        .success();

    // 4. List
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(output.contains("index.md"));
    assert!(!output.contains("README.md"));

    Ok(())
}

#[test]
fn test_list_excludes_ignored_types() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // 1. Initialize
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 2. Add spec and tmp files (tmp is in ignored_types by default)
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("spec.md")
        .arg("content")
        .assert()
        .success();

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("tmp")
        .arg("tmp.log")
        .arg("content")
        .assert()
        .success();

    // 3. List without -i: tmp should be hidden
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("spec.md"));
    assert!(!output.contains("tmp.log"));

    Ok(())
}

#[test]
fn test_list_ignored_types_configurable() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Configure trace as an additional ignored type
    fs::write(
        env.root().join("cue.json"),
        r#"{"ignored_types": ["tmp", "trace"]}"#,
    )?;

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("spec.md")
        .arg("content")
        .assert()
        .success();

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("trace")
        .arg("trace.log")
        .arg("content")
        .assert()
        .success();

    // List without -i: trace should be hidden because it's in ignored_types
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(output.contains("spec.md"));
    assert!(!output.contains("trace.log"));

    Ok(())
}

#[test]
fn test_list_includes_trace() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // 1. Initialize
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 2. Add trace file
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("trace")
        .arg("trace.log")
        .arg("trace content")
        .assert()
        .success();

    // 3. List
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(output.contains("trace.log"));
    assert!(output.contains("/trace/"));

    Ok(())
}

#[test]
fn test_list_include_gitignored() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // 1. Initialize
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 2. Add spec and tmp files
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("spec.md")
        .arg("content")
        .assert()
        .success();

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("tmp")
        .arg("tmp.log")
        .arg("content")
        .assert()
        .success();

    // 3. List with -i
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("-i")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(output.contains("spec.md"));
    assert!(output.contains("tmp.log"));

    Ok(())
}

#[test]
fn test_list_json_spec() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // 1. Initialize
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 2. Add a root spec file (--root: stable anchor, saved flat)
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("index.md")
        .arg("content")
        .assert()
        .success();

    // 3. List with --json
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--json")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: serde_json::Value = serde_json::from_str(&output)?;

    assert!(json.is_array());
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);

    let item = &arr[0];
    assert_eq!(item["name"], "index.md");
    assert_eq!(item["category"], "spec");
    assert_eq!(item["branch"], "main"); // default git branch in setup_git_repo is main
    // Root artifact: no hash or timestamp
    assert!(item["hash"].is_null());
    assert!(item["commit_hash"].is_null());
    assert_eq!(item["commit_timestamp"], 0);

    Ok(())
}

#[test]
fn test_list_json_nested_spec() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // 1. Initialize
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 2. Add nested spec file
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("tickets/SB-1234.md")
        .arg("content")
        .assert()
        .success();

    // 3. List with --json
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--json")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: serde_json::Value = serde_json::from_str(&output)?;

    let arr = json.as_array().unwrap();
    let item = arr
        .iter()
        .find(|i| i["path"].as_str().unwrap().contains("SB-1234.md"))
        .unwrap();

    // This is what we want to fix: it should be "tickets/SB-1234.md"
    assert_eq!(item["name"], "tickets/SB-1234.md");

    Ok(())
}

#[test]
fn test_list_json_nested_trace() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // 1. Initialize
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 2. Add nested trace file
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("trace")
        .arg("logs/app.log")
        .arg("trace content")
        .assert()
        .success();

    // 3. List with --json
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--json")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: serde_json::Value = serde_json::from_str(&output)?;

    let arr = json.as_array().unwrap();
    let item = arr.iter().find(|i| i["category"] == "trace").unwrap();

    assert_eq!(item["name"], "logs/app.log");

    Ok(())
}

#[test]
fn test_list_json_trace_with_root() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // 1. Initialize
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 2. Add trace file with --root: saved flat at trace/<filename>
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("trace")
        .arg("--root")
        .arg("trace.log")
        .arg("trace content")
        .assert()
        .success();

    // 3. List with --json
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--json")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: serde_json::Value = serde_json::from_str(&output)?;

    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);

    let item = &arr[0];
    assert_eq!(item["name"], "trace.log");
    assert_eq!(item["category"], "trace");

    // Root (flat) artifact: no hash or timestamp
    assert!(item["hash"].is_null());
    assert!(item["commit_hash"].is_null());
    assert_eq!(item["commit_timestamp"], 0);

    Ok(())
}

#[test]
fn test_list_json_trace_nested_by_default() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // 1. Initialize
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 2. Add trace file without --root: nested under ts-hash by default
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("trace")
        .arg("trace.log")
        .arg("trace content")
        .assert()
        .success();

    // 3. List with --json
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--json")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: serde_json::Value = serde_json::from_str(&output)?;

    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);

    let item = &arr[0];
    assert_eq!(item["name"], "trace.log");
    assert_eq!(item["category"], "trace");

    // Nested artifact: has non-null hash and non-zero timestamp
    assert!(item["hash"].is_string());
    assert!(item["commit_hash"].is_string());
    assert!(item["commit_timestamp"].as_u64().unwrap() > 0);

    Ok(())
}

#[test]
fn test_list_branch_flag() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // 1. Initialize
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 2. Add file to current branch (main)
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("main.md")
        .arg("content")
        .assert()
        .success();

    // 3. Create another branch and add file
    std::process::Command::new("git")
        .args(["checkout", "-b", "other"])
        .current_dir(env.root())
        .output()?;

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("other.md")
        .arg("content")
        .assert()
        .success();

    // 4. List current branch (other)
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(output.contains("other.md"));
    assert!(!output.contains("main.md"));

    // 5. List main branch via --branch
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--branch")
            .arg("main")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(output.contains("main.md"));
    assert!(!output.contains("other.md"));

    Ok(())
}

#[test]
fn test_list_all_branches() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // 1. Initialize
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 2. Add file to main
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("main.md")
        .arg("content")
        .assert()
        .success();

    // 3. Add file to other (manually create dir to simulate other branch having data)
    let other_spec_dir = env.root().join(".test-mem/other/spec");
    std::fs::create_dir_all(&other_spec_dir)?;
    std::fs::write(other_spec_dir.join("other.md"), "content")?;

    // 4. List --all
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--all")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(output.contains("main.md"));
    assert!(output.contains("other.md"));

    Ok(())
}

#[test]
fn test_list_all_with_slashed_branch() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // 1. Initialize
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 2. Add file to a branch with a slash
    std::process::Command::new("git")
        .args(["checkout", "-b", "feat/slash"])
        .current_dir(env.root())
        .output()?;

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("test.md")
        .arg("content")
        .assert()
        .success();

    // 3. List --all --json
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--all")
            .arg("--json")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: serde_json::Value = serde_json::from_str(&output)?;

    let arr = json.as_array().unwrap();
    // Should have "feat-slash" as branch name in JSON because we replace slashes for dir names
    let item = arr.iter().find(|i| i["name"] == "test.md").unwrap();
    assert_eq!(item["branch"], "feat-slash");

    Ok(())
}

// ── Filter tests ─────────────────────────────────────────────────────────────

/// Helper: init a mem repo with two todo files, one `status: todo` and one `status: done`.
fn setup_filter_repo(env: &helpers::TestEnv) -> anyhow::Result<()> {
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Write artifacts directly to avoid clap parsing "---"
    let todo_dir = env.root().join(".test-mem/main/todo");
    fs::create_dir_all(&todo_dir)?;
    fs::write(
        todo_dir.join("pending.md"),
        "---\nstatus: todo\n---\n# Pending",
    )?;
    fs::write(
        todo_dir.join("finished.md"),
        "---\nstatus: done\n---\n# Finished",
    )?;

    Ok(())
}

#[test]
fn test_list_filter_equality() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    setup_filter_repo(&env)?;

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--filter")
            .arg("status=todo")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(output.contains("pending.md"), "should include pending.md");
    assert!(
        !output.contains("finished.md"),
        "should exclude finished.md"
    );

    Ok(())
}

#[test]
fn test_list_filter_inequality() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    setup_filter_repo(&env)?;

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--filter")
            .arg("status!=done")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(output.contains("pending.md"), "should include pending.md");
    assert!(
        !output.contains("finished.md"),
        "should exclude finished.md"
    );

    Ok(())
}

#[test]
fn test_list_filter_contains() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    let todo_dir = env.root().join(".test-mem/main/spec");
    fs::create_dir_all(&todo_dir)?;
    fs::write(
        todo_dir.join("meeting.md"),
        "---\ntitle: Weekly Meeting Notes\n---\n# Body",
    )?;
    fs::write(
        todo_dir.join("review.md"),
        "---\ntitle: Code Review\n---\n# Body",
    )?;

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--filter")
            .arg("title~=Meeting")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(output.contains("meeting.md"), "should include meeting.md");
    assert!(!output.contains("review.md"), "should exclude review.md");

    Ok(())
}

#[test]
fn test_list_filter_multiple_anded() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    let todo_dir = env.root().join(".test-mem/main/todo");
    fs::create_dir_all(&todo_dir)?;
    fs::write(
        todo_dir.join("a.md"),
        "---\nstatus: todo\npriority: high\n---",
    )?;
    fs::write(
        todo_dir.join("b.md"),
        "---\nstatus: todo\npriority: low\n---",
    )?;
    fs::write(
        todo_dir.join("c.md"),
        "---\nstatus: done\npriority: high\n---",
    )?;

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--filter")
            .arg("status=todo")
            .arg("--filter")
            .arg("priority=high")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(output.contains("a.md"), "a.md should match both filters");
    assert!(!output.contains("b.md"), "b.md fails priority=high");
    assert!(!output.contains("c.md"), "c.md fails status=todo");

    Ok(())
}

#[test]
fn test_list_filter_missing_frontmatter_excluded_by_eq() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    setup_filter_repo(&env)?;

    // Add a file with no frontmatter at all
    let spec_dir = env.root().join(".test-mem/main/spec");
    fs::create_dir_all(&spec_dir)?;
    fs::write(spec_dir.join("plain.md"), "# No frontmatter here")?;

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--filter")
            .arg("status=todo")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(
        !output.contains("plain.md"),
        "plain.md has no frontmatter, should be excluded by ="
    );

    Ok(())
}

#[test]
fn test_list_filter_missing_frontmatter_included_by_neq() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    let spec_dir = env.root().join(".test-mem/main/spec");
    fs::create_dir_all(&spec_dir)?;
    fs::write(spec_dir.join("plain.md"), "# No frontmatter here")?;

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--filter")
            .arg("status!=done")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(
        output.contains("plain.md"),
        "plain.md has no status key, should pass !="
    );

    Ok(())
}

#[test]
fn test_list_filter_with_json_output() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    setup_filter_repo(&env)?;

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--json")
            .arg("--filter")
            .arg("status=todo")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: serde_json::Value = serde_json::from_str(&output)?;
    let arr = json.as_array().unwrap();

    assert_eq!(arr.len(), 1, "only one item should match status=todo");
    assert!(arr[0]["path"].as_str().unwrap().contains("pending.md"));
    // frontmatter field not in output unless --frontmatter passed
    assert!(arr[0].get("frontmatter").is_none());

    Ok(())
}

#[test]
fn test_list_filter_with_frontmatter_output() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    setup_filter_repo(&env)?;

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--frontmatter")
            .arg("--filter")
            .arg("status!=done")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    let json: serde_json::Value = serde_json::from_str(&output)?;
    let arr = json.as_array().unwrap();

    assert_eq!(arr.len(), 1);
    assert!(arr[0]["path"].as_str().unwrap().contains("pending.md"));
    // --frontmatter was passed so field should appear
    assert_eq!(arr[0]["frontmatter"]["status"], "todo");

    Ok(())
}

#[test]
fn test_list_filter_nested_key() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    let spec_dir = env.root().join(".test-mem/main/spec");
    fs::create_dir_all(&spec_dir)?;
    fs::write(
        spec_dir.join("high.md"),
        "---\nmeta:\n  priority: high\n---",
    )?;
    fs::write(spec_dir.join("low.md"), "---\nmeta:\n  priority: low\n---")?;

    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("--filter")
            .arg("meta.priority=high")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(output.contains("high.md"));
    assert!(!output.contains("low.md"));

    Ok(())
}

#[test]
fn test_list_not_initialized() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .arg("list")
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "directory does not exist. Run `cue init` first.",
        ));

    Ok(())
}

#[test]
fn test_list_type_filter() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Register 'trace' as a type (already default, but being explicit for clarity)
    // doc is not a default type; add it to config
    fs::write(
        env.root().join("cue.json"),
        r#"{"artifact_types": ["spec", "trace", "tmp", "doc"]}"#,
    )?;

    // 1. Initialize
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 2. Add different types of files
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("spec.md")
        .arg("content")
        .assert()
        .success();

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("doc")
        .arg("doc.md")
        .arg("content")
        .assert()
        .success();

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("tmp")
        .arg("tmp.log")
        .arg("content")
        .assert()
        .success();

    // 3. List with --type spec
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("-t")
            .arg("spec")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(output.contains("spec.md"));
    assert!(!output.contains("doc.md"));
    assert!(!output.contains("tmp.log"));

    // 4. List with --type doc
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("-t")
            .arg("doc")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(!output.contains("spec.md"));
    assert!(output.contains("doc.md"));
    assert!(!output.contains("tmp.log"));

    // 5. List with --type tmp (should work even if ignored by default)
    let output = String::from_utf8(
        env.command()
            .env("CUE_BRANCH_NAME", "test-mem")
            .env("CUE_DIR_NAME", ".test-mem")
            .arg("list")
            .arg("-t")
            .arg("tmp")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )?;
    assert!(!output.contains("spec.md"));
    assert!(!output.contains("doc.md"));
    assert!(output.contains("tmp.log"));

    Ok(())
}
