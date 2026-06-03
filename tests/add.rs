mod helpers;

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_add_from_file() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Initialize mem
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // Create a temporary file
    let source_file = temp.path().join("source.txt");
    fs::write(&source_file, "content from file")?;

    // Add from file with --root (stable anchor document)
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("index.md")
        .arg("--file")
        .arg(&source_file);

    cmd.assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/spec/index.md\n"));

    let file_path = temp.path().join(".test-mem/main/spec/index.md");
    assert!(file_path.exists());
    let content = fs::read_to_string(file_path)?;
    assert_eq!(content, "content from file");

    Ok(())
}

#[test]
fn test_add_clipboard_conflicts() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Conflict with inline content
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .arg("add")
        .arg("index.md")
        .arg("inline content")
        .arg("--clipboard");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));

    // Conflict with --file
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .arg("add")
        .arg("index.md")
        .arg("--file")
        .arg("some_file.txt")
        .arg("--clipboard");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));

    Ok(())
}

#[test]
fn test_add_clipboard_unsupported_format() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .arg("add")
        .arg("file.webp")
        .arg("--clipboard");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Unsupported image format"));

    Ok(())
}

#[test]
fn test_add_conflict_file_and_inline() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .arg("add")
        .arg("index.md")
        .arg("inline content")
        .arg("--file")
        .arg("some_file.txt");

    // clap should reject this
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));

    Ok(())
}

#[test]
fn test_add_spec_default() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Initialize mem
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // Add a root spec document (stable anchor)
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("index.md")
        .arg("Project scope");

    cmd.assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/spec/index.md\n"));

    let file_path = temp.path().join(".test-mem/main/spec/index.md");
    assert!(file_path.exists());
    let content = fs::read_to_string(file_path)?;
    assert_eq!(content, "Project scope");

    Ok(())
}

#[test]
fn test_add_no_content_empty_file() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Initialize mem
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // Add a file without content using --root
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("empty.txt")
        .arg("");

    cmd.assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/spec/empty.txt\n"));

    let file_path = temp.path().join(".test-mem/main/spec/empty.txt");
    assert!(file_path.exists());
    let content = fs::read_to_string(file_path)?;
    assert!(content.is_empty());

    Ok(())
}

#[test]
fn test_add_type_trace_nested_by_default() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Initialize mem
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // Add a trace file without --root: saved under trace/<ts>-<hash>/ by default
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--type")
        .arg("trace")
        .arg("error.log")
        .arg("stack trace content");

    cmd.assert()
        .success()
        .stdout(predicate::str::starts_with(".test-mem/main/trace/"));

    // File must be nested under a <ts>-<hash> subdirectory
    let trace_base = temp.path().join(".test-mem/main/trace");
    let entries = fs::read_dir(&trace_base)?;
    let mut found = false;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let file_path = path.join("error.log");
            if file_path.exists() {
                let content = fs::read_to_string(file_path)?;
                assert_eq!(content, "stack trace content");
                found = true;
                break;
            }
        }
    }
    assert!(found, "Trace file not found in any timestamped directory");

    Ok(())
}

#[test]
fn test_add_type_trace_with_root() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Initialize mem
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // Add a trace file WITH --root: saved flat at trace/<filename>
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--type")
        .arg("trace")
        .arg("--root")
        .arg("error.log")
        .arg("stack trace content");

    cmd.assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/trace/error.log\n"));

    let file_path = temp.path().join(".test-mem/main/trace/error.log");
    assert!(file_path.exists());
    let content = fs::read_to_string(file_path)?;
    assert_eq!(content, "stack trace content");

    Ok(())
}

#[test]
fn test_add_nested_by_default_for_any_type() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // spec type: default saves nested under ts-hash
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--type")
        .arg("spec")
        .arg("snapshot.md")
        .arg("nested spec");

    cmd.assert()
        .success()
        .stdout(predicate::str::starts_with(".test-mem/main/spec/"));

    // Must be nested, not at spec/snapshot.md
    let flat_path = temp.path().join(".test-mem/main/spec/snapshot.md");
    assert!(!flat_path.exists(), "File should NOT be at flat path");

    let spec_base = temp.path().join(".test-mem/main/spec");
    let entries = fs::read_dir(&spec_base)?;
    let mut found = false;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let file_path = path.join("snapshot.md");
            if file_path.exists() {
                found = true;
                break;
            }
        }
    }
    assert!(found, "Spec file not found in any timestamped directory");

    Ok(())
}

#[test]
fn test_add_unknown_type_rejected() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--type")
        .arg("unknown-type")
        .arg("file.md")
        .arg("content");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains(
            "Unknown artifact type 'unknown-type'",
        ))
        .stderr(predicate::str::contains("Valid types:"));

    Ok(())
}

#[test]
fn test_add_custom_type_via_config() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Write a mem.json that registers a custom type
    fs::write(
        temp.path().join("mem.json"),
        r#"{"artifact_types": ["spec", "trace", "tmp", "custom"]}"#,
    )?;

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // --root saves flat at custom/<filename>
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--type")
        .arg("custom")
        .arg("--root")
        .arg("notes.md")
        .arg("custom content");

    cmd.assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/custom/notes.md\n"));

    let file_path = temp.path().join(".test-mem/main/custom/notes.md");
    assert!(file_path.exists());
    assert_eq!(fs::read_to_string(file_path)?, "custom content");

    Ok(())
}

#[test]
fn test_add_type_tmp_nested_by_default() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // tmp without --root saves nested by default
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("tmp")
        .arg("session.log")
        .arg("tmp content");

    cmd.assert()
        .success()
        .stdout(predicate::str::starts_with(".test-mem/main/tmp/"));

    // Must be nested under a ts-hash dir
    let tmp_base = temp.path().join(".test-mem/main/tmp");
    let entries = fs::read_dir(&tmp_base)?;
    let mut found = false;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let file_path = path.join("session.log");
            if file_path.exists() {
                assert_eq!(fs::read_to_string(file_path)?, "tmp content");
                found = true;
                break;
            }
        }
    }
    assert!(found, "Tmp file not found in any timestamped directory");

    Ok(())
}

#[test]
fn test_add_type_tmp_with_root() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // tmp with --root saves flat
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("tmp")
        .arg("--root")
        .arg("session.log")
        .arg("tmp content");

    cmd.assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/tmp/session.log\n"));

    let file_path = temp.path().join(".test-mem/main/tmp/session.log");
    assert!(file_path.exists());
    assert_eq!(fs::read_to_string(file_path)?, "tmp content");

    Ok(())
}

#[test]
fn test_add_type_ref() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Register 'ref' as a valid type in project config
    fs::write(
        temp.path().join("mem.json"),
        r#"{"artifact_types": ["spec", "trace", "tmp", "ref"]}"#,
    )?;

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("ref")
        .arg("--root")
        .arg("doc.md")
        .arg("ref content");

    cmd.assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/ref/doc.md\n"));

    let file_path = temp.path().join(".test-mem/main/ref/doc.md");
    assert!(file_path.exists());
    assert_eq!(fs::read_to_string(file_path)?, "ref content");

    Ok(())
}

#[test]
fn test_add_type_bin() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Register 'bin' as a valid type in project config
    fs::write(
        temp.path().join("mem.json"),
        r#"{"artifact_types": ["spec", "trace", "tmp", "bin"]}"#,
    )?;

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("bin")
        .arg("--root")
        .arg("tool.sh")
        .arg("echo hello");

    cmd.assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/bin/tool.sh\n"));

    let file_path = temp.path().join(".test-mem/main/bin/tool.sh");
    assert!(file_path.exists());
    assert_eq!(fs::read_to_string(file_path)?, "echo hello");

    Ok(())
}

#[test]
fn test_add_type_doc() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Register 'doc' as a valid type in project config
    fs::write(
        temp.path().join("mem.json"),
        r#"{"artifact_types": ["spec", "trace", "tmp", "doc"]}"#,
    )?;

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("doc")
        .arg("--root")
        .arg("manual.md")
        .arg("doc content");

    cmd.assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/doc/manual.md\n"));

    let file_path = temp.path().join(".test-mem/main/doc/manual.md");
    assert!(file_path.exists());
    assert_eq!(fs::read_to_string(file_path)?, "doc content");

    Ok(())
}

#[test]
fn test_add_force_overwrite() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // 1. Create file at root
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("test.txt")
        .arg("v1");
    cmd.assert().success();

    // 2. Try overwrite without force
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("test.txt")
        .arg("v2");
    cmd.assert().failure().stderr(
        predicate::str::contains("File exists").and(predicate::str::contains("Use --force")),
    );

    // 3. Overwrite with force
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("--force")
        .arg("test.txt")
        .arg("v2");
    cmd.assert().success();

    let file_path = temp.path().join(".test-mem/main/spec/test.txt");
    assert_eq!(fs::read_to_string(file_path)?, "v2");

    Ok(())
}

#[test]
fn test_add_with_slashed_branch_name() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Create a branch with a slash
    std::process::Command::new("git")
        .args(["checkout", "-b", "feature/logic"])
        .current_dir(temp.path())
        .output()?;

    // Initialize mem
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // Add a root file
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("test.md")
        .arg("content");

    // We expect it to be in .test-mem/feature-logic/spec/test.md (NOT feature/logic)
    cmd.assert().success().stdout(predicate::str::diff(
        ".test-mem/feature-logic/spec/test.md\n",
    ));

    let file_path = temp.path().join(".test-mem/feature-logic/spec/test.md");
    assert!(file_path.exists());

    // Verify that the nested directory was NOT created
    let nested_dir = temp.path().join(".test-mem/feature/logic");
    assert!(!nested_dir.exists());

    Ok(())
}

#[test]
fn test_add_with_explicit_branch() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Initialize mem
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // Add a root file to a DIFFERENT branch than current (main)
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("--branch")
        .arg("feature/other")
        .arg("other.md")
        .arg("other branch content");

    cmd.assert().success().stdout(predicate::str::diff(
        ".test-mem/feature-other/spec/other.md\n",
    ));

    let file_path = temp.path().join(".test-mem/feature-other/spec/other.md");
    assert!(file_path.exists());
    let content = fs::read_to_string(file_path)?;
    assert_eq!(content, "other branch content");

    // Verify main branch spec doesn't have it
    let main_file = temp.path().join(".test-mem/main/spec/other.md");
    assert!(!main_file.exists());

    Ok(())
}

#[test]
fn test_add_with_explicit_branch_short() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    // Initialize mem
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // Add a root file using short flag -b
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("-b")
        .arg("short-b")
        .arg("short.md")
        .arg("short content");

    cmd.assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/short-b/spec/short.md\n"));

    let file_path = temp.path().join(".test-mem/short-b/spec/short.md");
    assert!(file_path.exists());

    Ok(())
}

#[test]
fn test_add_rejects_path_traversal() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    helpers::setup_git_repo(temp.path());

    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("init");
    cmd.assert().success();

    // Absolute path
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("/etc/passwd")
        .arg("hack");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("absolute paths are not allowed"));

    // Parent dir
    let mut cmd = Command::cargo_bin("mem")?;
    cmd.current_dir(temp.path())
        .env("MEM_BRANCH_NAME", "test-mem")
        .env("MEM_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("../outside.txt")
        .arg("hack");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("'..' is not allowed"));

    Ok(())
}
