mod helpers;

use predicates::prelude::*;
use std::fs;

#[test]
fn test_add_from_file() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Create a temporary file
    let source_file = env.root().join("source.txt");
    fs::write(&source_file, "content from file")?;

    // Add from file with --root (stable anchor document)
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("index.md")
        .arg("--file")
        .arg(&source_file)
        .assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/spec/index.md\n"));

    let file_path = env.root().join(".test-mem/main/spec/index.md");
    assert!(file_path.exists());
    let content = fs::read_to_string(file_path)?;
    assert_eq!(content, "content from file");

    Ok(())
}

#[test]
fn test_add_clipboard_conflicts() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Conflict with inline content
    env.command()
        .arg("add")
        .arg("index.md")
        .arg("inline content")
        .arg("--clipboard")
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));

    // Conflict with --file
    env.command()
        .arg("add")
        .arg("index.md")
        .arg("--file")
        .arg("some_file.txt")
        .arg("--clipboard")
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));

    Ok(())
}

#[test]
fn test_add_clipboard_unsupported_format() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .arg("add")
        .arg("file.webp")
        .arg("--clipboard")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unsupported image format"));

    Ok(())
}

#[test]
fn test_add_conflict_file_and_inline() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .arg("add")
        .arg("index.md")
        .arg("inline content")
        .arg("--file")
        .arg("some_file.txt")
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));

    Ok(())
}

#[test]
fn test_add_spec_default() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Add a root spec document (stable anchor)
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("index.md")
        .arg("Project scope")
        .assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/spec/index.md\n"));

    let file_path = env.root().join(".test-mem/main/spec/index.md");
    assert!(file_path.exists());
    let content = fs::read_to_string(file_path)?;
    assert_eq!(content, "Project scope");

    Ok(())
}

#[test]
fn test_add_no_content_empty_file() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Add a file without content using --root
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("empty.txt")
        .arg("")
        .assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/spec/empty.txt\n"));

    let file_path = env.root().join(".test-mem/main/spec/empty.txt");
    assert!(file_path.exists());
    let content = fs::read_to_string(file_path)?;
    assert!(content.is_empty());

    Ok(())
}

#[test]
fn test_add_type_trace_nested_by_default() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Add a trace file without --root: saved under trace/<ts>-<hash>/ by default
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--type")
        .arg("trace")
        .arg("error.log")
        .arg("stack trace content")
        .assert()
        .success()
        .stdout(predicate::str::starts_with(".test-mem/main/trace/"));

    // File must be nested under a <ts>-<hash> subdirectory
    let trace_base = env.root().join(".test-mem/main/trace");
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
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Add a trace file WITH --root: saved flat at trace/<filename>
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--type")
        .arg("trace")
        .arg("--root")
        .arg("error.log")
        .arg("stack trace content")
        .assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/trace/error.log\n"));

    let file_path = env.root().join(".test-mem/main/trace/error.log");
    assert!(file_path.exists());
    let content = fs::read_to_string(file_path)?;
    assert_eq!(content, "stack trace content");

    Ok(())
}

#[test]
fn test_add_nested_by_default_for_any_type() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // spec type: default saves nested under ts-hash
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--type")
        .arg("spec")
        .arg("snapshot.md")
        .arg("nested spec")
        .assert()
        .success()
        .stdout(predicate::str::starts_with(".test-mem/main/spec/"));

    // Must be nested, not at spec/snapshot.md
    let flat_path = env.root().join(".test-mem/main/spec/snapshot.md");
    assert!(!flat_path.exists(), "File should NOT be at flat path");

    let spec_base = env.root().join(".test-mem/main/spec");
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
        .arg("--type")
        .arg("unknown-type")
        .arg("file.md")
        .arg("content")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Unknown artifact type 'unknown-type'",
        ))
        .stderr(predicate::str::contains("Valid types:"));

    Ok(())
}

#[test]
fn test_add_custom_type_via_config() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Write a cue.json that registers a custom type
    fs::write(
        env.root().join("cue.json"),
        r#"{"artifact_types": ["spec", "trace", "tmp", "custom"]}"#,
    )?;

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // --root saves flat at custom/<filename>
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--type")
        .arg("custom")
        .arg("--root")
        .arg("notes.md")
        .arg("custom content")
        .assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/custom/notes.md\n"));

    let file_path = env.root().join(".test-mem/main/custom/notes.md");
    assert!(file_path.exists());
    assert_eq!(fs::read_to_string(file_path)?, "custom content");

    Ok(())
}

#[test]
fn test_add_type_tmp_nested_by_default() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // tmp without --root saves nested by default
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("tmp")
        .arg("session.log")
        .arg("tmp content")
        .assert()
        .success()
        .stdout(predicate::str::starts_with(".test-mem/main/tmp/"));

    // Must be nested under a ts-hash dir
    let tmp_base = env.root().join(".test-mem/main/tmp");
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
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // tmp with --root saves flat
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("-t")
        .arg("tmp")
        .arg("--root")
        .arg("session.log")
        .arg("tmp content")
        .assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/tmp/session.log\n"));

    let file_path = env.root().join(".test-mem/main/tmp/session.log");
    assert!(file_path.exists());
    assert_eq!(fs::read_to_string(file_path)?, "tmp content");

    Ok(())
}

#[test]
fn test_add_type_ref() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Register 'ref' as a valid type in project config
    fs::write(
        env.root().join("cue.json"),
        r#"{"artifact_types": ["spec", "trace", "tmp", "ref"]}"#,
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
        .arg("-t")
        .arg("ref")
        .arg("--root")
        .arg("doc.md")
        .arg("ref content")
        .assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/ref/doc.md\n"));

    let file_path = env.root().join(".test-mem/main/ref/doc.md");
    assert!(file_path.exists());
    assert_eq!(fs::read_to_string(file_path)?, "ref content");

    Ok(())
}

#[test]
fn test_add_type_bin() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Register 'bin' as a valid type in project config
    fs::write(
        env.root().join("cue.json"),
        r#"{"artifact_types": ["spec", "trace", "tmp", "bin"]}"#,
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
        .arg("-t")
        .arg("bin")
        .arg("--root")
        .arg("tool.sh")
        .arg("echo hello")
        .assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/bin/tool.sh\n"));

    let file_path = env.root().join(".test-mem/main/bin/tool.sh");
    assert!(file_path.exists());
    assert_eq!(fs::read_to_string(file_path)?, "echo hello");

    Ok(())
}

#[test]
fn test_add_type_doc() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Register 'doc' as a valid type in project config
    fs::write(
        env.root().join("cue.json"),
        r#"{"artifact_types": ["spec", "trace", "tmp", "doc"]}"#,
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
        .arg("-t")
        .arg("doc")
        .arg("--root")
        .arg("manual.md")
        .arg("doc content")
        .assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/main/doc/manual.md\n"));

    let file_path = env.root().join(".test-mem/main/doc/manual.md");
    assert!(file_path.exists());
    assert_eq!(fs::read_to_string(file_path)?, "doc content");

    Ok(())
}

#[test]
fn test_add_force_overwrite() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // 1. Create file at root
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("test.txt")
        .arg("v1")
        .assert()
        .success();

    // 2. Try overwrite without force
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("test.txt")
        .arg("v2")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("File exists").and(predicate::str::contains("Use --force")),
        );

    // 3. Overwrite with force
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("--force")
        .arg("test.txt")
        .arg("v2")
        .assert()
        .success();

    let file_path = env.root().join(".test-mem/main/spec/test.txt");
    assert_eq!(fs::read_to_string(file_path)?, "v2");

    Ok(())
}

#[test]
fn test_add_with_slashed_branch_name() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Create a branch with a slash
    std::process::Command::new("git")
        .args(["checkout", "-b", "feature/logic"])
        .current_dir(env.root())
        .output()?;

    // Initialize mem
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Add a root file
    // We expect it to be in .test-mem/feature-logic/spec/test.md (NOT feature/logic)
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("test.md")
        .arg("content")
        .assert()
        .success()
        .stdout(predicate::str::diff(
            ".test-mem/feature-logic/spec/test.md\n",
        ));

    let file_path = env.root().join(".test-mem/feature-logic/spec/test.md");
    assert!(file_path.exists());

    // Verify that the nested directory was NOT created
    let nested_dir = env.root().join(".test-mem/feature/logic");
    assert!(!nested_dir.exists());

    Ok(())
}

#[test]
fn test_add_with_explicit_branch() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Add a root file to a DIFFERENT branch than current (main)
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("--branch")
        .arg("feature/other")
        .arg("other.md")
        .arg("other branch content")
        .assert()
        .success()
        .stdout(predicate::str::diff(
            ".test-mem/feature-other/spec/other.md\n",
        ));

    let file_path = env.root().join(".test-mem/feature-other/spec/other.md");
    assert!(file_path.exists());
    let content = fs::read_to_string(file_path)?;
    assert_eq!(content, "other branch content");

    // Verify main branch spec doesn't have it
    let main_file = env.root().join(".test-mem/main/spec/other.md");
    assert!(!main_file.exists());

    Ok(())
}

#[test]
fn test_add_with_explicit_branch_short() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    // Initialize mem
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Add a root file using short flag -b
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("-b")
        .arg("short-b")
        .arg("short.md")
        .arg("short content")
        .assert()
        .success()
        .stdout(predicate::str::diff(".test-mem/short-b/spec/short.md\n"));

    let file_path = env.root().join(".test-mem/short-b/spec/short.md");
    assert!(file_path.exists());

    Ok(())
}

#[test]
fn test_add_with_single_frontmatter_field() -> anyhow::Result<()> {
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
        .arg("note.md")
        .arg("body text")
        .arg("-f")
        .arg("status=todo")
        .assert()
        .success();

    let file_path = env.root().join(".test-mem/main/spec/note.md");
    let content = fs::read_to_string(file_path)?;
    assert!(content.starts_with("---\n"), "File should start with ---");
    assert!(
        content.contains("status: todo"),
        "File should contain 'status: todo'"
    );
    assert!(
        content.contains("body text"),
        "File should contain the body"
    );

    Ok(())
}

#[test]
fn test_add_with_multiple_frontmatter_fields() -> anyhow::Result<()> {
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
        .arg("note.md")
        .arg("body text")
        .arg("-f")
        .arg("title=Hello")
        .arg("-f")
        .arg("priority=high")
        .assert()
        .success();

    let file_path = env.root().join(".test-mem/main/spec/note.md");
    let content = fs::read_to_string(file_path)?;
    assert!(content.starts_with("---\n"), "File should start with ---");
    assert!(
        content.contains("title: Hello"),
        "File should contain 'title: Hello'"
    );
    assert!(
        content.contains("priority: high"),
        "File should contain 'priority: high'"
    );
    assert!(
        content.contains("body text"),
        "File should contain the body"
    );

    Ok(())
}

#[test]
fn test_add_frontmatter_type_coercion() -> anyhow::Result<()> {
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
        .arg("note.md")
        .arg("")
        .arg("-f")
        .arg("done=true")
        .arg("-f")
        .arg("count=3")
        .assert()
        .success();

    let file_path = env.root().join(".test-mem/main/spec/note.md");
    let content = fs::read_to_string(file_path)?;
    // Booleans and integers must not be quoted in YAML output
    assert!(
        content.contains("done: true"),
        "bool should be unquoted: got:\n{}",
        content
    );
    assert!(
        content.contains("count: 3"),
        "integer should be unquoted: got:\n{}",
        content
    );

    Ok(())
}

#[test]
fn test_add_frontmatter_roundtrip_with_list() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Create artifact with frontmatter
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("--root")
        .arg("note.md")
        .arg("content")
        .arg("-f")
        .arg("status=active")
        .assert()
        .success();

    // List with --frontmatter --json and check the parsed field
    let output = env
        .command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("list")
        .arg("--frontmatter")
        .arg("--json")
        .output()?;
    assert!(output.status.success());

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let artifacts = json.as_array().expect("Expected JSON array");
    let note = artifacts
        .iter()
        .find(|a| a["name"].as_str() == Some("note.md"))
        .expect("note.md not found in list output");

    assert_eq!(
        note["frontmatter"]["status"].as_str(),
        Some("active"),
        "frontmatter.status should be 'active'"
    );

    Ok(())
}

#[test]
fn test_add_frontmatter_colon_in_string_value() -> anyhow::Result<()> {
    // A title containing ": " must be written as a quoted YAML string, not
    // parsed as a mapping. Covers AC #1 and #2 of fix-title-yaml-quoting.
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
        .arg("note.md")
        .arg("body text")
        .arg("-f")
        .arg("title=foo: bar baz")
        .arg("-f")
        .arg("branch=feature/foo: wip")
        .assert()
        .success();

    let file_path = env.root().join(".test-mem/main/spec/note.md");
    let raw = fs::read_to_string(&file_path)?;

    // AC #1: the raw file must contain a quoted string, not a bare mapping
    assert!(
        !raw.contains("title:\n") && !raw.contains("title: foo:"),
        "title must not be written as an unquoted mapping; got:\n{}",
        raw
    );
    assert!(
        !raw.contains("branch:\n") && !raw.contains("branch: feature/foo:"),
        "branch must not be written as an unquoted mapping; got:\n{}",
        raw
    );

    // AC #2: round-trip — YAML parse of the frontmatter must yield strings
    let fm_end = raw.find("---\n").and_then(|_| {
        let after = &raw[4..];
        after.find("---\n").map(|i| i + 4)
    });
    let fm_str = fm_end
        .map(|end| &raw[4..end])
        .expect("frontmatter not found");
    let parsed: serde_yaml::Value =
        serde_yaml::from_str(fm_str).expect("frontmatter must be valid YAML");

    assert_eq!(
        parsed["title"].as_str(),
        Some("foo: bar baz"),
        "title must round-trip as a string; got: {:?}",
        parsed["title"]
    );
    assert_eq!(
        parsed["branch"].as_str(),
        Some("feature/foo: wip"),
        "branch must round-trip as a string; got: {:?}",
        parsed["branch"]
    );

    Ok(())
}

#[test]
fn test_add_frontmatter_invalid_format_rejected() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .arg("add")
        .arg("note.md")
        .arg("body")
        .arg("-f")
        .arg("no-equals-sign")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Expected key=value"));

    Ok(())
}

#[test]
fn test_add_rejects_path_traversal() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    helpers::setup_git_repo(env.root());

    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("init")
        .assert()
        .success();

    // Absolute path
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("/etc/passwd")
        .arg("hack")
        .assert()
        .failure()
        .stderr(predicate::str::contains("absolute paths are not allowed"));

    // Parent dir
    env.command()
        .env("CUE_BRANCH_NAME", "test-mem")
        .env("CUE_DIR_NAME", ".test-mem")
        .arg("add")
        .arg("../outside.txt")
        .arg("hack")
        .assert()
        .failure()
        .stderr(predicate::str::contains("'..' is not allowed"));

    Ok(())
}
