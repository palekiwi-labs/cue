mod helpers;

use predicates::prelude::*;

#[test]
fn exclusions_subtract_from_positive_union_regardless_of_order() {
    let env = helpers::TestEnv::new();
    for address in ["acme/one/alpha", "acme/two/beta", "other/project/gamma"] {
        let dir = env.cue_store().join(address);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("context.md"),
            "---\nkind: work\ncreated_at: 1\n---\n",
        )
        .unwrap();
        env.command()
            .args(["context", "pin", address])
            .assert()
            .success();
    }
    for (matchers, expected) in [
        (
            vec!["acme/*", "!acme/two", "other/*"],
            "acme/one/alpha\nother/project/gamma\n",
        ),
        (
            vec!["!acme/two", "acme/*", "other/*", "acme/two"],
            "acme/one/alpha\nother/project/gamma\n",
        ),
        (
            vec!["acme/one", "!acme/*", "other/*"],
            "other/project/gamma\n",
        ),
        (
            vec!["!acme/*", "acme/one", "other/*"],
            "other/project/gamma\n",
        ),
        (vec!["!acme/*"], ""),
        (
            vec!["acme/*", "!missing/*"],
            "acme/one/alpha\nacme/two/beta\n",
        ),
    ] {
        let config = serde_json::json!({"scopes":{"team":matchers},"default_scope":"team"});
        std::fs::write(env.config_dir.join("cue.json"), config.to_string()).unwrap();
        for args in [
            vec!["context", "list"],
            vec!["context", "pins"],
            vec!["context", "list", "--pinned"],
        ] {
            env.command().args(args).assert().success().stdout(expected);
        }
    }
}

#[test]
fn named_default_does_not_redirect_singular_operations() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    std::fs::write(
        env.config_dir.join("cue.json"),
        r#"{
        "scopes":{"team":["other/*"]},"default_scope":"team"
    }"#,
    )
    .unwrap();
    env.command()
        .args(["context", "create", "local"])
        .assert()
        .success();
    env.command()
        .args(["context", "pin", "local"])
        .assert()
        .success()
        .stdout("pinned acme/widgets/local\n");
    env.command()
        .args([
            "add",
            "--context",
            "local",
            "--type",
            "note",
            "example.md",
            "local note",
        ])
        .assert()
        .success();
    assert!(
        env.cue_store()
            .join("acme/widgets/local/note/example.md")
            .is_file()
    );
    env.command()
        .args(["status", "--context", "local", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("acme/widgets/local"));
    env.command()
        .args(["context", "unpin", "local"])
        .assert()
        .success()
        .stdout("unpinned acme/widgets/local\n");
}

#[test]
fn unmatched_and_unknown_defaults_are_distinct() {
    let env = helpers::TestEnv::new();
    std::fs::write(
        env.config_dir.join("cue.json"),
        r#"{
        "scopes":{"empty":["absent/*"]},"default_scope":"empty"
    }"#,
    )
    .unwrap();
    for command in ["list", "pins"] {
        env.command()
            .args(["context", command])
            .assert()
            .success()
            .stdout("");
        env.command()
            .args(["context", command, "--scope", "missing"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("unknown query scope 'missing'"));
    }
    std::fs::write(
        env.config_dir.join("cue.json"),
        r#"{"default_scope":"missing"}"#,
    )
    .unwrap();
    env.command()
        .args(["context", "pins"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown query scope 'missing'"));
    env.command()
        .args(["context", "pins", "--scope", "store"])
        .assert()
        .success();
}

#[test]
fn named_pinned_listing_filters_before_limit_without_reading_unpinned_contexts() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for (slug, kind) in [("alpha", "work"), ("beta", "coord"), ("gamma", "coord")] {
        env.command()
            .args(["context", "create", slug, "--kind", kind])
            .assert()
            .success();
        env.command()
            .args(["context", "pin", slug])
            .assert()
            .success();
    }
    let bad = env.cue_store().join("acme/widgets/unpinned");
    std::fs::create_dir_all(&bad).unwrap();
    std::fs::write(bad.join("context.md"), "not valid metadata").unwrap();
    std::fs::write(
        env.config_dir.join("cue.json"),
        r#"{
        "scopes":{"team":["acme/*"]},"default_scope":"team"
    }"#,
    )
    .unwrap();
    env.command()
        .args([
            "context",
            "list",
            "--pinned",
            "--filter",
            "kind=coord",
            "--limit",
            "1",
        ])
        .assert()
        .success()
        .stdout("acme/widgets/beta\n");
}

#[test]
fn default_global_path_is_home_config_and_legacy_project_keys_fail() {
    let env = helpers::TestEnv::new();
    let home = env.root().join("home");
    std::fs::create_dir_all(home.join(".config/cue")).unwrap();
    std::fs::write(
        home.join(".config/cue/cue.json"),
        r#"{"default_scope":"store"}"#,
    )
    .unwrap();
    env.command()
        .env_remove("CUE_CONFIG_DIR")
        .env("HOME", &home)
        .args(["context", "pins"])
        .assert()
        .success();
    env.setup_repo_with_origin();
    std::fs::write(
        env.root().join("cue.json"),
        r#"{"context":{"master":{"artifacts":[]}}}"#,
    )
    .unwrap();
    env.command()
        .args(["context", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown field"));
}

#[test]
fn project_config_merges_names_replaces_arrays_and_shadows_builtins() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for address in ["acme/widgets/alpha", "other/project/beta"] {
        std::fs::create_dir_all(env.cue_store().join(address)).unwrap();
        env.command()
            .args(["context", "pin", address])
            .assert()
            .success();
    }
    std::fs::write(
        env.config_dir.join("cue.json"),
        r#"{
        "scopes":{"team":["acme/*"],"retained":["acme/*"]},
        "default_scope":"retained"
    }"#,
    )
    .unwrap();
    std::fs::write(
        env.root().join("cue.json"),
        r#"{
        "scopes":{"team":["other/*"],"repo":["other/*"],"store":[]},
        "default_scope":"team"
    }"#,
    )
    .unwrap();
    let nested = env.root().join("nested/child");
    std::fs::create_dir_all(&nested).unwrap();
    for (scope, expected) in [
        (None, "other/project/beta\n"),
        (Some("retained"), "acme/widgets/alpha\n"),
        (Some("repo"), "other/project/beta\n"),
        (Some("store"), ""),
    ] {
        let mut command = env.command();
        command.current_dir(&nested).args(["context", "pins"]);
        if let Some(scope) = scope {
            command.args(["--scope", scope]);
        }
        command.assert().success().stdout(expected);
    }
}

#[test]
fn malformed_matchers_fail_instead_of_silently_matching_nothing() {
    let env = helpers::TestEnv::new();
    for matcher in [
        "acme",
        "*/repo",
        "acme/part*",
        "acme/../repo",
        "../repo",
        "/repo",
        "acme/",
        "acme/repo/extra",
        ".state/*",
        "acme/\nrepo",
        "acme/\\repo",
    ] {
        let config = serde_json::json!({"scopes":{"bad":[matcher]},"default_scope":"bad"});
        std::fs::write(env.config_dir.join("cue.json"), config.to_string()).unwrap();
        env.command()
            .args(["context", "pins"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("matcher"));
    }
}

#[test]
fn named_scope_unions_existing_scopes_without_duplicates() {
    let env = helpers::TestEnv::new();
    for address in [
        "acme/widgets/alpha",
        "acme/tools/beta",
        "other/project/gamma",
        "excluded/repo/delta",
    ] {
        let dir = env.cue_store().join(address);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("context.md"),
            "---\nkind: work\ncreated_at: 1\n---\n",
        )
        .unwrap();
        env.command()
            .args(["context", "pin", address])
            .assert()
            .success();
    }
    env.command()
        .args(["context", "pin", "missing/repo/stale"])
        .assert()
        .success();
    std::fs::write(
        env.config_dir.join("cue.json"),
        r#"{
        "scopes": {"team": ["acme/*", "acme/widgets", "other/project", "missing/*"]},
        "default_scope": "team"
    }"#,
    )
    .unwrap();
    for args in [
        vec!["context", "list"],
        vec!["context", "pins"],
        vec!["context", "list", "--pinned"],
        vec!["context", "list", "--scope", "team"],
    ] {
        env.command()
            .args(args)
            .assert()
            .success()
            .stdout("acme/tools/beta\nacme/widgets/alpha\nother/project/gamma\n");
    }
}

#[test]
fn configured_default_scope_and_explicit_override() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for address in ["acme/widgets/alpha", "other/project/beta"] {
        env.command()
            .args(["context", "pin", address])
            .assert()
            .success();
    }
    std::fs::write(
        env.config_dir.join("cue.json"),
        r#"{"default_scope":"store"}"#,
    )
    .unwrap();
    env.command()
        .args(["context", "pins"])
        .assert()
        .success()
        .stdout("acme/widgets/alpha\nother/project/beta\n");
    env.command()
        .args(["context", "pins", "--scope", "repo"])
        .assert()
        .success()
        .stdout("acme/widgets/alpha\n");
}

#[test]
fn collections_reject_unknown_config_but_singular_commands_ignore_it() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    std::fs::write(env.config_dir.join("cue.json"), r#"{"branch_name":"old"}"#).unwrap();
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    env.command()
        .args(["status", "--context", "alpha"])
        .assert()
        .success();
    for command in ["list", "pins"] {
        env.command()
            .args(["context", command])
            .assert()
            .failure()
            .stderr(predicate::str::contains("branch_name"));
    }
}
