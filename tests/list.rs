mod helpers;

use std::fs;
use tempfile::TempDir;

#[test]
fn test_list_empty_repo() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // 1. Initialize mem
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // 2. List (should be empty)
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list");

    let output = cmd.assert().success().get_output().stdout.clone();
    assert!(output.is_empty());

    Ok(())
}

// ── Frontmatter tests ────────────────────────────────────────────────────────

#[test]
fn test_list_frontmatter_flag_implies_json() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("index.md")
        .arg("no frontmatter here");
    cmd.assert().success();

    // --frontmatter without --json should still produce valid JSON
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--frontmatter");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    let json: serde_json::Value = serde_json::from_str(&output)?;
    assert!(json.is_array());

    Ok(())
}

#[test]
fn test_list_frontmatter_absent_when_no_flag() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // Write file with frontmatter directly to bypass clap argument parsing of "---"
    let artifact_path = temp.path().join(".test-mem/main/spec/index.md");
    fs::create_dir_all(artifact_path.parent().unwrap())?;
    fs::write(&artifact_path, "---\nstatus: active\n---\n# Hello")?;

    // Without --frontmatter, the field should be absent
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--json");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    let json: serde_json::Value = serde_json::from_str(&output)?;
    let item = &json.as_array().unwrap()[0];

    assert!(item.get("frontmatter").is_none());

    Ok(())
}

#[test]
fn test_list_frontmatter_parsed_correctly() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // Write file with frontmatter directly to bypass clap argument parsing of "---"
    let artifact_path = temp.path().join(".test-mem/main/spec/index.md");
    fs::create_dir_all(artifact_path.parent().unwrap())?;
    fs::write(
        &artifact_path,
        "---\nstatus: active\npriority: high\n---\n# Body here",
    )?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--frontmatter");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    let json: serde_json::Value = serde_json::from_str(&output)?;
    let item = &json.as_array().unwrap()[0];

    let fm = item.get("frontmatter").expect("frontmatter field missing");
    assert_eq!(fm["status"], "active");
    assert_eq!(fm["priority"], "high");

    Ok(())
}

#[test]
fn test_list_frontmatter_absent_for_file_without_frontmatter() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    let content = "# No frontmatter in this file";
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("index.md")
        .arg(content);
    cmd.assert().success();

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--frontmatter");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    let json: serde_json::Value = serde_json::from_str(&output)?;
    let item = &json.as_array().unwrap()[0];

    // File has no frontmatter — field should be absent
    assert!(item.get("frontmatter").is_none());

    Ok(())
}

#[test]
fn test_list_frontmatter_malformed_unclosed_fence() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // Opening fence but no closing fence — write directly to avoid clap parsing "---"
    let artifact_path = temp.path().join(".test-mem/main/spec/index.md");
    fs::create_dir_all(artifact_path.parent().unwrap())?;
    fs::write(
        &artifact_path,
        "---\nstatus: active\n# Body starts here without closing fence",
    )?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--frontmatter");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    let json: serde_json::Value = serde_json::from_str(&output)?;
    // Should succeed (not crash), with frontmatter absent
    assert!(json.is_array());
    let item = &json.as_array().unwrap()[0];
    assert!(item.get("frontmatter").is_none());

    Ok(())
}

#[test]
fn test_list_from_subdirectory() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // 1. Initialize
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // 2. Add a root file (stable anchor)
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("index.md")
        .arg("content");
    cmd.assert().success();

    // 3. Create a subdirectory and run list from there
    let sub = temp.path().join("src/nested");
    std::fs::create_dir_all(&sub)?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(&sub)
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    // Path should still be relative to git root, NOT to the subdirectory
    assert_eq!(output.trim(), ".test-mem/main/spec/index.md");

    Ok(())
}

#[test]
fn test_list_ignores_shallow_paths() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // 1. Initialize
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // 2. Create a file directly under the branch dir (invalid depth)
    let branch_dir = temp.path().join(".test-mem/main");
    std::fs::create_dir_all(&branch_dir)?;
    let invalid_file = branch_dir.join("README.md");
    std::fs::write(invalid_file, "invalid")?;

    // 3. Add a valid file
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("index.md")
        .arg("content");
    cmd.assert().success();

    // 4. List
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    assert!(output.contains("index.md"));
    assert!(!output.contains("README.md"));

    Ok(())
}

#[test]
fn test_list_excludes_ignored_types() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // 1. Initialize
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // 2. Add spec and tmp files (tmp is in ignored_types by default)
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("spec.md")
        .arg("content");
    cmd.assert().success();

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("tmp")
        .arg("tmp.log")
        .arg("content");
    cmd.assert().success();

    // 3. List without -i: tmp should be hidden
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("spec.md"));
    assert!(!output.contains("tmp.log"));

    Ok(())
}

#[test]
fn test_list_ignored_types_configurable() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Configure trace as an additional ignored type
    fs::write(
        temp.path().join("mem.json"),
        r#"{"ignored_types": ["tmp", "trace"]}"#,
    )?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("spec.md")
        .arg("content");
    cmd.assert().success();

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("trace")
        .arg("trace.log")
        .arg("content");
    cmd.assert().success();

    // List without -i: trace should be hidden because it's in ignored_types
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    assert!(output.contains("spec.md"));
    assert!(!output.contains("trace.log"));

    Ok(())
}

#[test]
fn test_list_includes_trace() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // 1. Initialize
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // 2. Add trace file
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("trace")
        .arg("trace.log")
        .arg("trace content");
    cmd.assert().success();

    // 3. List
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    assert!(output.contains("trace.log"));
    assert!(output.contains("/trace/"));

    Ok(())
}

#[test]
fn test_list_include_gitignored() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // 1. Initialize
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // 2. Add spec and tmp files
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("spec.md")
        .arg("content");
    cmd.assert().success();

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("tmp")
        .arg("tmp.log")
        .arg("content");
    cmd.assert().success();

    // 3. List with -i
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("-i");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    assert!(output.contains("spec.md"));
    assert!(output.contains("tmp.log"));

    Ok(())
}

#[test]
fn test_list_json_spec() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // 1. Initialize
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // 2. Add a root spec file (--root: stable anchor, saved flat)
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("index.md")
        .arg("content");
    cmd.assert().success();

    // 3. List with --json
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--json");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
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
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // 1. Initialize
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // 2. Add nested spec file
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("tickets/SB-1234.md")
        .arg("content");
    cmd.assert().success();

    // 3. List with --json
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--json");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
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
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // 1. Initialize
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // 2. Add nested trace file
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("trace")
        .arg("logs/app.log")
        .arg("trace content");
    cmd.assert().success();

    // 3. List with --json
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--json");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    let json: serde_json::Value = serde_json::from_str(&output)?;

    let arr = json.as_array().unwrap();
    let item = arr.iter().find(|i| i["category"] == "trace").unwrap();

    assert_eq!(item["name"], "logs/app.log");

    Ok(())
}

#[test]
fn test_list_json_trace_with_root() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // 1. Initialize
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // 2. Add trace file with --root: saved flat at trace/<filename>
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("trace")
        .arg("--root")
        .arg("trace.log")
        .arg("trace content");
    cmd.assert().success();

    // 3. List with --json
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--json");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
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
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // 1. Initialize
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // 2. Add trace file without --root: nested under ts-hash by default
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("trace")
        .arg("trace.log")
        .arg("trace content");
    cmd.assert().success();

    // 3. List with --json
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--json");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
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
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // 1. Initialize
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // 2. Add file to current branch (main)
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("main.md")
        .arg("content");
    cmd.assert().success();

    // 3. Create another branch and add file
    std::process::Command::new("git")
        .args(["checkout", "-b", "other"])
        .current_dir(temp.path())
        .output()?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("other.md")
        .arg("content");
    cmd.assert().success();

    // 4. List current branch (other)
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list");
    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    assert!(output.contains("other.md"));
    assert!(!output.contains("main.md"));

    // 5. List main branch via --branch
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--branch")
        .arg("main");
    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    assert!(output.contains("main.md"));
    assert!(!output.contains("other.md"));

    Ok(())
}

#[test]
fn test_list_all_branches() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // 1. Initialize
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // 2. Add file to main
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("main.md")
        .arg("content");
    cmd.assert().success();

    // 3. Add file to other (manually create dir to simulate other branch having data)
    let other_spec_dir = temp.path().join(".test-mem/other/spec");
    std::fs::create_dir_all(&other_spec_dir)?;
    std::fs::write(other_spec_dir.join("other.md"), "content")?;

    // 4. List --all
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--all");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    assert!(output.contains("main.md"));
    assert!(output.contains("other.md"));

    Ok(())
}

#[test]
fn test_list_all_with_slashed_branch() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // 1. Initialize
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // 2. Add file to a branch with a slash
    std::process::Command::new("git")
        .args(["checkout", "-b", "feat/slash"])
        .current_dir(temp.path())
        .output()?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("test.md")
        .arg("content");
    cmd.assert().success();

    // 3. List --all --json
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--all")
        .arg("--json");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    let json: serde_json::Value = serde_json::from_str(&output)?;

    let arr = json.as_array().unwrap();
    // Should have "feat-slash" as branch name in JSON because we replace slashes for dir names
    let item = arr.iter().find(|i| i["name"] == "test.md").unwrap();
    assert_eq!(item["branch"], "feat-slash");

    Ok(())
}

// ── Filter tests ─────────────────────────────────────────────────────────────

/// Helper: init a mem repo with two todo files, one `status: todo` and one `status: done`.
fn setup_filter_repo(temp: &tempfile::TempDir) -> anyhow::Result<()> {
    helpers::setup_git_repo(temp.path());

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // Write artifacts directly to avoid clap parsing "---"
    let todo_dir = temp.path().join(".test-mem/main/todo");
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
    let temp = TempDir::new()?;
    setup_filter_repo(&temp)?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--filter")
        .arg("status=todo");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    assert!(output.contains("pending.md"), "should include pending.md");
    assert!(
        !output.contains("finished.md"),
        "should exclude finished.md"
    );

    Ok(())
}

#[test]
fn test_list_filter_inequality() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    setup_filter_repo(&temp)?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--filter")
        .arg("status!=done");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    assert!(output.contains("pending.md"), "should include pending.md");
    assert!(
        !output.contains("finished.md"),
        "should exclude finished.md"
    );

    Ok(())
}

#[test]
fn test_list_filter_contains() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    let todo_dir = temp.path().join(".test-mem/main/spec");
    fs::create_dir_all(&todo_dir)?;
    fs::write(
        todo_dir.join("meeting.md"),
        "---\ntitle: Weekly Meeting Notes\n---\n# Body",
    )?;
    fs::write(
        todo_dir.join("review.md"),
        "---\ntitle: Code Review\n---\n# Body",
    )?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--filter")
        .arg("title~=Meeting");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    assert!(output.contains("meeting.md"), "should include meeting.md");
    assert!(!output.contains("review.md"), "should exclude review.md");

    Ok(())
}

#[test]
fn test_list_filter_multiple_anded() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    let todo_dir = temp.path().join(".test-mem/main/todo");
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

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--filter")
        .arg("status=todo")
        .arg("--filter")
        .arg("priority=high");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    assert!(output.contains("a.md"), "a.md should match both filters");
    assert!(!output.contains("b.md"), "b.md fails priority=high");
    assert!(!output.contains("c.md"), "c.md fails status=todo");

    Ok(())
}

#[test]
fn test_list_filter_missing_frontmatter_excluded_by_eq() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    setup_filter_repo(&temp)?;

    // Add a file with no frontmatter at all
    let spec_dir = temp.path().join(".test-mem/main/spec");
    fs::create_dir_all(&spec_dir)?;
    fs::write(spec_dir.join("plain.md"), "# No frontmatter here")?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--filter")
        .arg("status=todo");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    assert!(
        !output.contains("plain.md"),
        "plain.md has no frontmatter, should be excluded by ="
    );

    Ok(())
}

#[test]
fn test_list_filter_missing_frontmatter_included_by_neq() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    let spec_dir = temp.path().join(".test-mem/main/spec");
    fs::create_dir_all(&spec_dir)?;
    fs::write(spec_dir.join("plain.md"), "# No frontmatter here")?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--filter")
        .arg("status!=done");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    assert!(
        output.contains("plain.md"),
        "plain.md has no status key, should pass !="
    );

    Ok(())
}

#[test]
fn test_list_filter_with_json_output() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    setup_filter_repo(&temp)?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--json")
        .arg("--filter")
        .arg("status=todo");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
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
    let temp = TempDir::new()?;
    setup_filter_repo(&temp)?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--frontmatter")
        .arg("--filter")
        .arg("status!=done");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
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
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    let spec_dir = temp.path().join(".test-mem/main/spec");
    fs::create_dir_all(&spec_dir)?;
    fs::write(
        spec_dir.join("high.md"),
        "---\nmeta:\n  priority: high\n---",
    )?;
    fs::write(spec_dir.join("low.md"), "---\nmeta:\n  priority: low\n---")?;

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--filter")
        .arg("meta.priority=high");

    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    assert!(output.contains("high.md"));
    assert!(!output.contains("low.md"));

    Ok(())
}

#[test]
fn test_list_not_initialized() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path()).arg("list");

    cmd.assert().failure().stderr(predicates::str::contains(
        "directory does not exist. Run `mem init` first.",
    ));

    Ok(())
}

#[test]
fn test_list_type_filter() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Register 'trace' as a type (already default, but being explicit for clarity)
    // doc is not a default type; add it to config
    fs::write(
        temp.path().join("mem.json"),
        r#"{"artifact_types": ["spec", "trace", "tmp", "doc"]}"#,
    )?;

    // 1. Initialize
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // 2. Add different types of files
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("spec.md")
        .arg("content");
    cmd.assert().success();

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("doc")
        .arg("doc.md")
        .arg("content");
    cmd.assert().success();

    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("tmp")
        .arg("tmp.log")
        .arg("content");
    cmd.assert().success();

    // 3. List with --type spec
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("-t")
        .arg("spec");
    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    assert!(output.contains("spec.md"));
    assert!(!output.contains("doc.md"));
    assert!(!output.contains("tmp.log"));

    // 4. List with --type doc
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("-t")
        .arg("doc");
    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    assert!(!output.contains("spec.md"));
    assert!(output.contains("doc.md"));
    assert!(!output.contains("tmp.log"));

    // 5. List with --type tmp (should work even if ignored by default)
    let mut cmd = helpers::mem_cmd();
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("-t")
        .arg("tmp");
    let output = String::from_utf8(cmd.assert().success().get_output().stdout.clone())?;
    assert!(!output.contains("spec.md"));
    assert!(!output.contains("doc.md"));
    assert!(output.contains("tmp.log"));

    Ok(())
}
