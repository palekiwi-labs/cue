mod helpers;

use predicates::prelude::*;
use serde_json::Value;
use std::path::Path;
use std::process::Command;

/// A second repository scope, so whole-store breadth has more than one scope
/// to widen to. Only an origin remote is needed: the store scope is derived
/// from it, and listing reads no revision.
fn setup_scope_repo(path: &Path, origin: &str) {
    std::fs::create_dir_all(path).expect("Failed to create scope repo dir");
    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(path)
        .output()
        .expect("Failed to init scope repo");
    helpers::setup_origin(path, origin);
}

/// Write a log entry stamped with an exact timestamp, which `log add` cannot
/// produce: it stamps the current nanosecond, so two entries never tie.
fn plant_log_entry(context_dir: &Path, timestamp: u64) {
    let log_dir = context_dir.join("log");
    std::fs::create_dir_all(&log_dir).expect("Failed to create log dir");
    std::fs::write(
        log_dir.join(format!("{timestamp:020}.json")),
        format!(
            "{{\"timestamp\":{timestamp},\"title\":\"planted\",\"trace\":null,\
             \"found\":[],\"decided\":[],\"open\":[]}}\n"
        ),
    )
    .expect("Failed to write log entry");
}

/// Plant a context whose metadata cannot be parsed, so any listing that
/// enumerates it fails. `kind` and `created_at` are required, so omitting
/// them is enough.
fn plant_unreadable_context(scope_dir: &Path, slug: &str) {
    let context_dir = scope_dir.join(slug);
    std::fs::create_dir_all(&context_dir).expect("Failed to create context dir");
    std::fs::write(context_dir.join("context.md"), "---\ntitle: broken\n---\n")
        .expect("Failed to write context.md");
}

/// Run a JSON listing and hand back its contexts as an array.
fn list_json(env: &helpers::TestEnv, args: &[&str]) -> anyhow::Result<Vec<Value>> {
    let output = env
        .command()
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let contexts: Value = serde_json::from_slice(&output)?;
    Ok(contexts
        .as_array()
        .expect("contexts should be an array")
        .clone())
}

#[test]
fn context_list_json_reports_central_context_metadata() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["roadmap", "architecture"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    env.command()
        .args([
            "context",
            "create",
            "release",
            "--title",
            "Release cue",
            "--kind",
            "coord",
            "--mode",
            "build",
            "--description",
            "Coordinate the release",
            "--parent",
            "acme/widgets/roadmap",
            "--ref",
            "acme/widgets/architecture",
        ])
        .assert()
        .success();

    let output = env
        .command()
        .args(["context", "list", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let contexts: Value = serde_json::from_slice(&output)?;
    // The referenced contexts must exist before they can be referenced, so
    // they are listed alongside the one under test.
    assert_eq!(contexts.as_array().map(Vec::len), Some(3));
    let context = contexts
        .as_array()
        .and_then(|contexts| {
            contexts
                .iter()
                .find(|context| context["context"] == "release")
        })
        .expect("release context should be listed");

    assert_eq!(context["context"], "release");
    assert_eq!(context["scope"], "acme/widgets");
    assert_eq!(context["title"], "Release cue");
    assert_eq!(context["kind"], "coord");
    assert_eq!(context["mode"], "build");
    assert_eq!(context["description"], "Coordinate the release");
    assert!(context["created_at"].as_u64().is_some());
    assert_eq!(context["parent"], "acme/widgets/roadmap");
    assert_eq!(context["refs"][0], "acme/widgets/architecture");

    Ok(())
}

#[test]
fn context_list_succeeds_with_no_contexts_when_repository_scope_is_missing() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    assert!(!env.cue_store().join("acme/widgets").exists());

    env.command()
        .args(["context", "list"])
        .assert()
        .success()
        .stdout("");

    env.command()
        .args(["context", "list", "--json"])
        .assert()
        .success()
        .stdout("[]\n");

    Ok(())
}

#[test]
fn context_list_prints_only_contexts_in_slug_order() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "zeta"])
        .assert()
        .success();
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    std::fs::create_dir(env.cue_store().join("acme/widgets/not-a-context"))?;

    env.command()
        .args(["context", "list"])
        .assert()
        .success()
        .stdout("alpha\nzeta\n");

    Ok(())
}

/// `--scope repo` is the default, so naming it explicitly must not change
/// what is listed, and must not widen to contexts in another scope.
#[test]
fn context_list_scope_repo_matches_the_default_view() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let other = env.root().join("other-repo");
    setup_scope_repo(&other, "https://github.com/other/project.git");
    for slug in ["zeta", "alpha"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    env.command()
        .args(["-C"])
        .arg(&other)
        .args(["context", "create", "roadmap"])
        .assert()
        .success();

    env.command()
        .args(["context", "list", "--scope", "repo"])
        .assert()
        .success()
        .stdout("alpha\nzeta\n");
}

/// `--scope store` widens to every repository scope. A bare slug no longer
/// identifies a context once the query spans scopes, so each line is the
/// canonical `<org>/<repo>/<slug>` address, ordered by that address.
#[test]
fn context_list_scope_store_lists_every_scope_as_canonical_addresses() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let other = env.root().join("other-repo");
    setup_scope_repo(&other, "https://github.com/other/project.git");
    let abacus = env.root().join("abacus-repo");
    setup_scope_repo(&abacus, "https://github.com/abacus/tools.git");

    for slug in ["zeta", "alpha"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    env.command()
        .args(["-C"])
        .arg(&other)
        .args(["context", "create", "roadmap"])
        .assert()
        .success();
    env.command()
        .args(["-C"])
        .arg(&abacus)
        .args(["context", "create", "build"])
        .assert()
        .success();

    env.command()
        .args(["context", "list", "--scope", "store"])
        .assert()
        .success()
        .stdout(
            "abacus/tools/build\nacme/widgets/alpha\nacme/widgets/zeta\nother/project/roadmap\n",
        );
}

/// JSON already carries `scope` per entry, so widening adds entries rather
/// than changing their shape.
#[test]
fn context_list_scope_store_json_reports_every_scope() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let other = env.root().join("other-repo");
    setup_scope_repo(&other, "https://github.com/other/project.git");
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    env.command()
        .args(["-C"])
        .arg(&other)
        .args(["context", "create", "roadmap"])
        .assert()
        .success();

    let output = env
        .command()
        .args(["context", "list", "--scope", "store", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let contexts: Value = serde_json::from_slice(&output)?;
    let contexts = contexts.as_array().expect("contexts should be an array");

    assert_eq!(contexts.len(), 2);
    assert_eq!(contexts[0]["scope"], "acme/widgets");
    assert_eq!(contexts[0]["context"], "alpha");
    assert_eq!(contexts[1]["scope"], "other/project");
    assert_eq!(contexts[1]["context"], "roadmap");

    Ok(())
}

/// Scope is a closed vocabulary of `repo|store`; anything else is rejected
/// rather than silently widened or narrowed.
#[test]
fn context_list_rejects_an_unknown_scope_value() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    for invalid in ["", "all", "Repo", "global", "repository"] {
        env.command()
            .args(["context", "list", "--scope", invalid])
            .assert()
            .failure()
            .stderr(predicate::str::contains("repo"))
            .stderr(predicate::str::contains("store"));
    }
}

/// The whole-store view resolves no repository scope, so it works from
/// anywhere, including outside a Git repository. The default view still
/// requires an origin remote.
#[test]
fn context_list_scope_store_needs_no_repository_scope() {
    let env = helpers::TestEnv::new();
    let other = env.root().join("other-repo");
    setup_scope_repo(&other, "https://github.com/other/project.git");
    env.command()
        .args(["-C"])
        .arg(&other)
        .args(["context", "create", "roadmap"])
        .assert()
        .success();

    env.command()
        .args(["context", "list", "--scope", "store"])
        .assert()
        .success()
        .stdout("other/project/roadmap\n");

    env.command().args(["context", "list"]).assert().failure();
}

/// Only `<org>/<repo>` directories are scopes. Store-internal state lives in
/// dot-directories under the store root, so the whole-store walk must skip
/// them rather than mistaking their contents for contexts.
#[test]
fn context_list_scope_store_skips_store_internal_state() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    env.command()
        .args(["context", "pin", "release"])
        .assert()
        .success();
    assert!(
        env.cue_store()
            .join(".state/pins/acme/widgets/release")
            .is_file()
    );

    // A decoy shaped exactly like a context, planted inside store-internal
    // state: only skipping dot-directories keeps it out of the listing.
    let decoy = env.cue_store().join(".state/decoy/scope/imposter");
    std::fs::create_dir_all(&decoy)?;
    std::fs::write(
        decoy.join("context.md"),
        "---\nkind: work\ncreated_at: 1\n---\n",
    )?;

    env.command()
        .args(["context", "list", "--scope", "store"])
        .assert()
        .success()
        .stdout("acme/widgets/release\n");

    Ok(())
}

/// A scope directory holding no context is not an error, and contributes no
/// lines to the whole-store view.
#[test]
fn context_list_scope_store_skips_scopes_without_contexts() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "release"])
        .assert()
        .success();
    std::fs::create_dir_all(env.cue_store().join("empty/scope"))?;
    std::fs::write(env.cue_store().join("acme/stray"), "")?;

    env.command()
        .args(["context", "list", "--scope", "store"])
        .assert()
        .success()
        .stdout("acme/widgets/release\n");

    Ok(())
}

/// Recency orders by the newest log entry, so the context worked on last
/// leads regardless of where its slug falls alphabetically.
#[test]
fn context_list_sort_recency_orders_by_latest_log_entry() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["alpha", "zeta"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    for slug in ["alpha", "zeta"] {
        env.command()
            .args(["log", "add", "--context", slug, "--title", "worked"])
            .assert()
            .success();
    }

    env.command()
        .args(["context", "list", "--sort", "recency"])
        .assert()
        .success()
        .stdout("zeta\nalpha\n");
}

/// Recency is activity, not age: the order the contexts were created in does
/// not survive into the listing when the logs say otherwise. The expected
/// order here matches neither creation order nor its reverse.
#[test]
fn context_list_sort_recency_ignores_context_creation_order() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["alpha", "beta", "gamma"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    for slug in ["gamma", "alpha", "beta"] {
        env.command()
            .args(["log", "add", "--context", slug, "--title", "worked"])
            .assert()
            .success();
    }

    env.command()
        .args(["context", "list", "--sort", "recency"])
        .assert()
        .success()
        .stdout("beta\nalpha\ngamma\n");
}

/// A context with no log entry has no activity to order by, so it sorts
/// behind every context that has one, and keeps canonical order among its
/// peers rather than being dropped or ordered arbitrarily.
#[test]
fn context_list_sort_recency_places_contexts_without_logs_last() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["alpha", "beta", "zeta"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    env.command()
        .args(["log", "add", "--context", "zeta", "--title", "worked"])
        .assert()
        .success();

    env.command()
        .args(["context", "list", "--sort", "recency"])
        .assert()
        .success()
        .stdout("zeta\nalpha\nbeta\n");
}

/// Contexts sharing a newest timestamp fall back on canonical address order,
/// which spans scopes, so the tiebreak is the whole address and not the slug.
#[test]
fn context_list_sort_recency_breaks_ties_by_canonical_address() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let other = env.root().join("other-repo");
    setup_scope_repo(&other, "https://github.com/other/project.git");
    for slug in ["alpha", "zeta"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    env.command()
        .args(["-C"])
        .arg(&other)
        .args(["context", "create", "roadmap"])
        .assert()
        .success();

    plant_log_entry(&env.cue_store().join("acme/widgets/alpha"), 200);
    plant_log_entry(&env.cue_store().join("other/project/roadmap"), 200);
    plant_log_entry(&env.cue_store().join("acme/widgets/zeta"), 100);

    env.command()
        .args(["context", "list", "--scope", "store", "--sort", "recency"])
        .assert()
        .success()
        .stdout("acme/widgets/alpha\nother/project/roadmap\nacme/widgets/zeta\n");
}

/// Omitting `--sort` keeps the canonical ordering, even where logs exist that
/// would order the listing differently.
#[test]
fn context_list_without_sort_keeps_canonical_order_despite_logs() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["alpha", "zeta"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    for slug in ["alpha", "zeta"] {
        env.command()
            .args(["log", "add", "--context", slug, "--title", "worked"])
            .assert()
            .success();
    }

    env.command()
        .args(["context", "list"])
        .assert()
        .success()
        .stdout("alpha\nzeta\n");
}

/// `--limit` is independent of ordering: it keeps the leading rows of
/// whatever order the listing is already in.
#[test]
fn context_list_limit_keeps_the_leading_rows() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["alpha", "beta", "zeta"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }

    env.command()
        .args(["context", "list", "--limit", "2"])
        .assert()
        .success()
        .stdout("alpha\nbeta\n");
}

/// A limit of zero selects no contexts. That is an empty listing rather than
/// an error or an ignored limit, in both output forms.
#[test]
fn context_list_limit_zero_selects_no_contexts() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();

    env.command()
        .args(["context", "list", "--limit", "0"])
        .assert()
        .success()
        .stdout("");

    env.command()
        .args(["context", "list", "--limit", "0", "--json"])
        .assert()
        .success()
        .stdout("[]\n");
}

/// A limit beyond the number of contexts is not an error and selects them
/// all, so a caller need not know the count in advance.
#[test]
fn context_list_limit_beyond_the_listing_selects_every_context() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["alpha", "zeta"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }

    env.command()
        .args(["context", "list", "--limit", "99"])
        .assert()
        .success()
        .stdout("alpha\nzeta\n");
}

/// A limit is a count, so anything that is not an unsigned integer is
/// rejected rather than rounded, clamped, or ignored.
#[test]
fn context_list_rejects_a_limit_that_is_not_an_unsigned_integer() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    for invalid in ["", "-1", "1.5", "two", "1e3"] {
        env.command()
            .args(["context", "list", "--limit", invalid])
            .assert()
            .failure()
            .stdout("");
    }
}

/// Ordering happens before truncation, so a limited recency listing is the
/// most recently active contexts and not an arbitrary subset reordered.
#[test]
fn context_list_sorts_before_limiting() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["alpha", "beta", "zeta"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    for slug in ["alpha", "beta", "zeta"] {
        env.command()
            .args(["log", "add", "--context", slug, "--title", "worked"])
            .assert()
            .success();
    }

    env.command()
        .args(["context", "list", "--sort", "recency", "--limit", "2"])
        .assert()
        .success()
        .stdout("zeta\nbeta\n");
}

/// Both output forms are the same selection rendered differently, so a sorted
/// and limited JSON listing carries exactly the rows the plain listing prints,
/// in the same order.
#[test]
fn context_list_json_matches_the_plain_selection() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["alpha", "beta", "zeta"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    for slug in ["beta", "zeta"] {
        env.command()
            .args(["log", "add", "--context", slug, "--title", "worked"])
            .assert()
            .success();
    }

    let output = env
        .command()
        .args([
            "context", "list", "--sort", "recency", "--limit", "2", "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let contexts: Value = serde_json::from_slice(&output)?;
    let slugs: Vec<&str> = contexts
        .as_array()
        .expect("contexts should be an array")
        .iter()
        .map(|context| {
            context["context"]
                .as_str()
                .expect("slug should be a string")
        })
        .collect();

    assert_eq!(slugs, ["zeta", "beta"]);

    env.command()
        .args(["context", "list", "--sort", "recency", "--limit", "2"])
        .assert()
        .success()
        .stdout("zeta\nbeta\n");

    Ok(())
}

/// Selection is scope-independent: the whole-store view resolves no
/// repository scope, so recency and limits work from outside a repository
/// just as they do inside one.
#[test]
fn context_list_scope_store_sorts_and_limits_outside_a_repository() {
    let env = helpers::TestEnv::new();
    let other = env.root().join("other-repo");
    setup_scope_repo(&other, "https://github.com/other/project.git");
    let abacus = env.root().join("abacus-repo");
    setup_scope_repo(&abacus, "https://github.com/abacus/tools.git");
    for (dir, slug) in [(&other, "roadmap"), (&abacus, "build")] {
        env.command()
            .args(["-C"])
            .arg(dir)
            .args(["context", "create", slug])
            .assert()
            .success();
    }

    plant_log_entry(&env.cue_store().join("abacus/tools/build"), 100);
    plant_log_entry(&env.cue_store().join("other/project/roadmap"), 200);

    env.command()
        .args([
            "context", "list", "--scope", "store", "--sort", "recency", "--limit", "1",
        ])
        .assert()
        .success()
        .stdout("other/project/roadmap\n");
}

/// A reported context carries the same fields whatever selection produced
/// it, `last_logged_at` among them: it describes the context, not the
/// ordering that was asked for.
#[test]
fn context_list_json_fields_are_unchanged_by_selection() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    env.command()
        .args(["log", "add", "--context", "alpha", "--title", "worked"])
        .assert()
        .success();

    for args in [
        vec!["context", "list", "--json"],
        vec![
            "context", "list", "--sort", "recency", "--limit", "1", "--json",
        ],
    ] {
        let contexts = list_json(&env, &args)?;
        let mut fields: Vec<&str> = contexts[0]
            .as_object()
            .expect("a context should be an object")
            .keys()
            .map(String::as_str)
            .collect();
        fields.sort_unstable();

        assert_eq!(
            fields,
            [
                "context",
                "created_at",
                "description",
                "kind",
                "last_logged_at",
                "mode",
                "parent",
                "path",
                "pinned",
                "refs",
                "scope",
                "title",
            ]
        );
    }

    Ok(())
}

/// Log activity is reported in whole Unix seconds. Entries are named for the
/// nanosecond they were written at, so the reported value is that stamp
/// truncated to a second: nanosecond precision exists to order entries, and
/// is not what a reader is handed.
#[test]
fn context_list_json_reports_last_logged_at_in_seconds() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    plant_log_entry(
        &env.cue_store().join("acme/widgets/alpha"),
        1_700_000_000_987_654_321,
    );

    for args in [
        vec!["context", "list", "--json"],
        vec!["context", "list", "--sort", "recency", "--json"],
    ] {
        let contexts = list_json(&env, &args)?;
        assert_eq!(contexts[0]["last_logged_at"], 1_700_000_000_u64);
    }

    Ok(())
}

/// The newest entry is what is reported, not the first or the last one the
/// directory listing happened to hand back.
#[test]
fn context_list_json_reports_the_newest_log_entry() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    let context_dir = env.cue_store().join("acme/widgets/alpha");
    for timestamp in [
        1_700_000_000_000_000_000,
        1_800_000_000_000_000_000,
        1_600_000_000_000_000_000,
    ] {
        plant_log_entry(&context_dir, timestamp);
    }

    let contexts = list_json(&env, &["context", "list", "--json"])?;
    assert_eq!(contexts[0]["last_logged_at"], 1_800_000_000_u64);

    Ok(())
}

/// A context with no log entry has no activity to report. The field is still
/// present and holds null, so a reader distinguishes "never logged" from
/// "logged at some second" without treating a missing key as a third case.
#[test]
fn context_list_json_reports_null_last_logged_at_without_logs() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();

    let contexts = list_json(&env, &["context", "list", "--json"])?;
    let context = contexts[0]
        .as_object()
        .expect("a context should be an object");

    assert_eq!(context.get("last_logged_at"), Some(&Value::Null));

    Ok(())
}

/// The reported second is not what orders a recency listing. Two contexts
/// logged within the same second are ordered by the nanosecond stamps their
/// entries are named for, while both report that same second.
#[test]
fn context_list_sort_recency_orders_within_one_reported_second() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["alpha", "zeta"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    plant_log_entry(
        &env.cue_store().join("acme/widgets/alpha"),
        1_700_000_000_000_000_100,
    );
    plant_log_entry(
        &env.cue_store().join("acme/widgets/zeta"),
        1_700_000_000_900_000_000,
    );

    env.command()
        .args(["context", "list", "--sort", "recency"])
        .assert()
        .success()
        .stdout("zeta\nalpha\n");

    let contexts = list_json(&env, &["context", "list", "--sort", "recency", "--json"])?;
    assert_eq!(contexts[0]["context"], "zeta");
    assert_eq!(contexts[0]["last_logged_at"], 1_700_000_000_u64);
    assert_eq!(contexts[1]["context"], "alpha");
    assert_eq!(contexts[1]["last_logged_at"], 1_700_000_000_u64);

    Ok(())
}

/// Activity is read per context, so widening the query reports each context's
/// own log rather than the scope's newest.
#[test]
fn context_list_scope_store_json_reports_last_logged_at_per_context() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let other = env.root().join("other-repo");
    setup_scope_repo(&other, "https://github.com/other/project.git");
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    env.command()
        .args(["-C"])
        .arg(&other)
        .args(["context", "create", "roadmap"])
        .assert()
        .success();
    plant_log_entry(
        &env.cue_store().join("other/project/roadmap"),
        1_700_000_000_000_000_000,
    );

    let contexts = list_json(&env, &["context", "list", "--scope", "store", "--json"])?;
    assert_eq!(contexts[0]["context"], "alpha");
    assert_eq!(contexts[0]["last_logged_at"], Value::Null);
    assert_eq!(contexts[1]["context"], "roadmap");
    assert_eq!(contexts[1]["last_logged_at"], 1_700_000_000_u64);

    Ok(())
}

/// The plain listing reports no activity, so it reads no log and a log path
/// that cannot be listed cannot affect it. JSON does read every log, so the
/// same store is an error there rather than a listing that reports a context
/// as never logged because its log could not be read.
#[test]
fn context_list_json_fails_when_a_log_cannot_be_read() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    // A regular file where the log directory belongs: reading it as a
    // directory fails with something other than "not found".
    std::fs::write(env.cue_store().join("acme/widgets/alpha/log"), "")?;

    env.command()
        .args(["context", "list"])
        .assert()
        .success()
        .stdout("alpha\n");

    env.command()
        .args(["context", "list", "--json"])
        .assert()
        .failure();

    Ok(())
}

/// Recency is read from log filenames, so it costs one directory listing per
/// context and never opens an entry. An entry whose contents cannot be parsed
/// still orders the context it belongs to, and the default listing, which
/// reads no log at all, is unaffected either way.
#[test]
fn context_list_sort_recency_reads_log_filenames_not_payloads() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["alpha", "zeta"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    plant_log_entry(&env.cue_store().join("acme/widgets/alpha"), 100);
    let log_dir = env.cue_store().join("acme/widgets/zeta/log");
    std::fs::create_dir_all(&log_dir)?;
    std::fs::write(log_dir.join("00000000000000000300.json"), "{not json")?;

    env.command()
        .args(["context", "list", "--sort", "recency"])
        .assert()
        .success()
        .stdout("zeta\nalpha\n");

    env.command()
        .args(["context", "list"])
        .assert()
        .success()
        .stdout("alpha\nzeta\n");

    Ok(())
}

/// A log directory holds entries named for their timestamp; anything else in
/// it is not an entry. A name that is not exactly the stamp is ignored, and so
/// is a directory wearing an entry's name, so neither can be mistaken for
/// activity. A context whose log holds only such things has no activity, which
/// is the same standing as a context with no log directory at all.
#[test]
fn context_list_sort_recency_ignores_names_that_are_not_log_entries() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["alpha", "beta", "gamma"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    plant_log_entry(&env.cue_store().join("acme/widgets/beta"), 100);

    // gamma is left with no log directory. alpha gets a log directory holding
    // nothing that is an entry: a foreign extension, a stamp that is too
    // short, one that is too long, one that is not all digits, and a
    // directory named exactly as an entry would be.
    let log_dir = env.cue_store().join("acme/widgets/alpha/log");
    std::fs::create_dir_all(&log_dir)?;
    for name in [
        "00000000000000000900.md",
        "900.json",
        "000000000000000000900.json",
        "0000000000000000090a.json",
    ] {
        std::fs::write(log_dir.join(name), "{}")?;
    }
    std::fs::create_dir_all(log_dir.join("00000000000000000900.json"))?;

    env.command()
        .args(["context", "list", "--sort", "recency"])
        .assert()
        .success()
        .stdout("beta\nalpha\ngamma\n");

    Ok(())
}

/// The working set is a set of contexts, so listing it reports the same rows
/// the unnarrowed listing reports, in the same canonical order.
#[test]
fn context_list_pinned_reports_only_the_working_set() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["alpha", "beta", "zeta"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    for slug in ["zeta", "alpha"] {
        env.command()
            .args(["context", "pin", slug])
            .assert()
            .success();
    }

    env.command()
        .args(["context", "list", "--pinned"])
        .assert()
        .success()
        .stdout("alpha\nzeta\n");
}

/// Pin state is held per scope, so whole-store breadth reports the working
/// set across every scope. A bare slug stops identifying a context once the
/// query spans scopes, so those lines are canonical addresses, exactly as an
/// unnarrowed whole-store listing prints them.
#[test]
fn context_list_pinned_scope_store_prints_canonical_addresses() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let other = env.root().join("other-repo");
    setup_scope_repo(&other, "https://github.com/other/project.git");
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    env.command()
        .args(["-C"])
        .arg(&other)
        .args(["context", "create", "roadmap"])
        .assert()
        .success();
    env.command()
        .args(["context", "pin", "alpha"])
        .assert()
        .success();
    env.command()
        .args(["context", "pin", "other/project/roadmap"])
        .assert()
        .success();

    env.command()
        .args(["context", "list", "--pinned", "--scope", "store"])
        .assert()
        .success()
        .stdout("acme/widgets/alpha\nother/project/roadmap\n");

    // The default breadth sees only this repository's pins.
    env.command()
        .args(["context", "list", "--pinned"])
        .assert()
        .success()
        .stdout("alpha\n");
}

/// A pin outlives the context it names, and a context that does not exist has
/// no row to report. Such a pin is skipped rather than reported as an empty
/// row, and it keeps its place in the working set: `pins` is what answers
/// what is pinned, including pins a listing cannot render.
#[test]
fn context_list_pinned_skips_pins_without_a_context() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    for context in ["alpha", "ghost", "other/project/roadmap"] {
        env.command()
            .args(["context", "pin", context])
            .assert()
            .success();
    }

    env.command()
        .args(["context", "list", "--pinned", "--scope", "store"])
        .assert()
        .success()
        .stdout("acme/widgets/alpha\n");

    env.command()
        .args(["context", "pins", "--scope", "store"])
        .assert()
        .success()
        .stdout("acme/widgets/alpha\nacme/widgets/ghost\nother/project/roadmap\n");
}

/// `--pinned` selects what is enumerated rather than narrowing an enumerated
/// listing. A context that cannot be read is fatal to a listing that visits
/// it, so a store holding one proves the narrowed listing never visits the
/// contexts outside the working set.
#[test]
fn context_list_pinned_reads_only_pinned_contexts() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    env.command()
        .args(["context", "pin", "alpha"])
        .assert()
        .success();
    plant_unreadable_context(&env.cue_store().join("acme/widgets"), "broken");

    for args in [
        vec!["context", "list"],
        vec!["context", "list", "--json"],
        vec!["context", "list", "--scope", "store"],
    ] {
        env.command().args(&args).assert().failure();
    }

    env.command()
        .args(["context", "list", "--pinned"])
        .assert()
        .success()
        .stdout("alpha\n");

    env.command()
        .args(["context", "list", "--pinned", "--scope", "store"])
        .assert()
        .success()
        .stdout("acme/widgets/alpha\n");
}

/// The same pushdown holds for the per-context log scan that JSON and recency
/// ordering perform: a log outside the working set is never listed, so a log
/// that cannot be read is fatal only to the listing that visits it.
#[test]
fn context_list_pinned_reads_only_pinned_logs() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["alpha", "noisy"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    env.command()
        .args(["context", "pin", "alpha"])
        .assert()
        .success();
    // A regular file where the log directory belongs: reading it as a
    // directory fails with something other than "not found".
    std::fs::write(env.cue_store().join("acme/widgets/noisy/log"), "")?;

    env.command()
        .args(["context", "list", "--json"])
        .assert()
        .failure();
    env.command()
        .args(["context", "list", "--sort", "recency"])
        .assert()
        .failure();

    let contexts = list_json(&env, &["context", "list", "--pinned", "--json"])?;
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0]["context"], "alpha");

    env.command()
        .args(["context", "list", "--pinned", "--sort", "recency"])
        .assert()
        .success()
        .stdout("alpha\n");

    Ok(())
}

/// The narrowed listing carries the full row shape, so a client renders the
/// working set from one query. Narrowing is a selection, so it adds no field
/// and removes none.
#[test]
fn context_list_pinned_json_reports_full_rows() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args([
            "context",
            "create",
            "release",
            "--title",
            "Release cue",
            "--kind",
            "coord",
            "--mode",
            "build",
            "--description",
            "Coordinate the release",
        ])
        .assert()
        .success();
    env.command()
        .args(["context", "pin", "release"])
        .assert()
        .success();
    plant_log_entry(
        &env.cue_store().join("acme/widgets/release"),
        1_700_000_000_000_000_000,
    );

    let contexts = list_json(&env, &["context", "list", "--pinned", "--json"])?;
    assert_eq!(contexts.len(), 1);
    let context = &contexts[0];

    assert_eq!(context["context"], "release");
    assert_eq!(context["scope"], "acme/widgets");
    assert_eq!(context["title"], "Release cue");
    assert_eq!(context["kind"], "coord");
    assert_eq!(context["mode"], "build");
    assert_eq!(context["description"], "Coordinate the release");
    assert_eq!(context["last_logged_at"], 1_700_000_000_u64);
    assert_eq!(
        context["path"],
        Value::from(
            env.cue_store()
                .join("acme/widgets/release/context.md")
                .to_str()
                .expect("store path should be valid UTF-8")
        )
    );

    let mut fields: Vec<&str> = context
        .as_object()
        .expect("a context should be an object")
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        [
            "context",
            "created_at",
            "description",
            "kind",
            "last_logged_at",
            "mode",
            "parent",
            "path",
            "pinned",
            "refs",
            "scope",
            "title",
        ]
    );

    Ok(())
}

/// Narrowing to the working set is independent of ordering and truncation, so
/// the three compose. The excluded context is the most recently active one,
/// so it would lead the listing if narrowing were not applied first.
#[test]
fn context_list_pinned_composes_with_sort_and_limit() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["alpha", "beta", "gamma", "zeta"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    for slug in ["alpha", "beta", "zeta"] {
        env.command()
            .args(["context", "pin", slug])
            .assert()
            .success();
    }
    let scope_dir = env.cue_store().join("acme/widgets");
    plant_log_entry(&scope_dir.join("alpha"), 300);
    plant_log_entry(&scope_dir.join("beta"), 200);
    plant_log_entry(&scope_dir.join("zeta"), 100);
    plant_log_entry(&scope_dir.join("gamma"), 400);

    env.command()
        .args(["context", "list", "--pinned", "--sort", "recency"])
        .assert()
        .success()
        .stdout("alpha\nbeta\nzeta\n");

    env.command()
        .args([
            "context", "list", "--pinned", "--sort", "recency", "--limit", "2",
        ])
        .assert()
        .success()
        .stdout("alpha\nbeta\n");
}

/// An empty working set is an empty listing rather than an error, in both
/// output forms, including when the store holds no pin state at all.
#[test]
fn context_list_pinned_succeeds_with_no_pins() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    assert!(!env.cue_store().join(".state").exists());

    env.command()
        .args(["context", "list", "--pinned"])
        .assert()
        .success()
        .stdout("");

    env.command()
        .args(["context", "list", "--pinned", "--json"])
        .assert()
        .success()
        .stdout("[]\n");
}

/// Whole-store breadth resolves no repository scope, so the working set is
/// listable from anywhere, including outside a Git repository. The default
/// breadth still requires an origin remote.
#[test]
fn context_list_pinned_scope_store_needs_no_repository_scope() {
    let env = helpers::TestEnv::new();
    let other = env.root().join("other-repo");
    setup_scope_repo(&other, "https://github.com/other/project.git");
    env.command()
        .args(["-C"])
        .arg(&other)
        .args(["context", "create", "roadmap"])
        .assert()
        .success();
    env.command()
        .args(["-C"])
        .arg(&other)
        .args(["context", "pin", "roadmap"])
        .assert()
        .success();

    env.command()
        .args(["context", "list", "--pinned", "--scope", "store"])
        .assert()
        .success()
        .stdout("other/project/roadmap\n");

    env.command()
        .args(["context", "list", "--pinned"])
        .assert()
        .failure();
}

/// Membership in the working set is a fact about a context, so an
/// unnarrowed JSON listing reports it per row. This is what a client
/// rendering every context reads to mark the pinned ones, instead of joining
/// `cue context pins` against the listing itself.
#[test]
fn context_list_json_reports_pin_membership() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["alpha", "beta", "zeta"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    env.command()
        .args(["context", "pin", "beta"])
        .assert()
        .success();

    let contexts = list_json(&env, &["context", "list", "--json"])?;
    assert_eq!(contexts.len(), 3);
    assert_eq!(contexts[0]["context"], "alpha");
    assert_eq!(contexts[0]["pinned"], false);
    assert_eq!(contexts[1]["context"], "beta");
    assert_eq!(contexts[1]["pinned"], true);
    assert_eq!(contexts[2]["context"], "zeta");
    assert_eq!(contexts[2]["pinned"], false);

    // Unpinning is reflected without any other change to the row.
    env.command()
        .args(["context", "unpin", "beta"])
        .assert()
        .success();
    let contexts = list_json(&env, &["context", "list", "--json"])?;
    assert!(contexts.iter().all(|context| context["pinned"] == false));

    Ok(())
}

/// Pin state is held per scope, so whole-store breadth evaluates membership
/// against the scope each row belongs to rather than against one repository's
/// pins.
#[test]
fn context_list_scope_store_json_reports_pin_membership_per_scope() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let other = env.root().join("other-repo");
    setup_scope_repo(&other, "https://github.com/other/project.git");
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    env.command()
        .args(["-C"])
        .arg(&other)
        .args(["context", "create", "roadmap"])
        .assert()
        .success();
    env.command()
        .args(["context", "pin", "other/project/roadmap"])
        .assert()
        .success();

    let contexts = list_json(&env, &["context", "list", "--scope", "store", "--json"])?;
    assert_eq!(contexts.len(), 2);
    assert_eq!(contexts[0]["scope"], "acme/widgets");
    assert_eq!(contexts[0]["pinned"], false);
    assert_eq!(contexts[1]["scope"], "other/project");
    assert_eq!(contexts[1]["pinned"], true);

    // The default breadth sees only this repository's pin state, where the
    // other scope's pin is not a member.
    let contexts = list_json(&env, &["context", "list", "--json"])?;
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0]["context"], "alpha");
    assert_eq!(contexts[0]["pinned"], false);

    Ok(())
}

/// A row carries the same fields whatever selection produced it, so the
/// narrowed listing reports membership too, where it is always true. A client
/// therefore reads `pinned` unconditionally rather than branching on which
/// query it ran.
#[test]
fn context_list_pinned_json_reports_pinned_rows() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["alpha", "beta"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
        env.command()
            .args(["context", "pin", slug])
            .assert()
            .success();
    }

    let contexts = list_json(&env, &["context", "list", "--pinned", "--json"])?;
    assert_eq!(contexts.len(), 2);
    assert!(contexts.iter().all(|context| context["pinned"] == true));

    Ok(())
}

/// Plain output is one identifier per line, and that line is valid input to
/// `context switch` and `context unpin`. Pin state changes nothing about it.
#[test]
fn context_list_plain_output_is_unchanged_by_pin_state() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["alpha", "beta"] {
        env.command()
            .args(["context", "create", slug])
            .assert()
            .success();
    }
    env.command()
        .args(["context", "pin", "beta"])
        .assert()
        .success();

    env.command()
        .args(["context", "list"])
        .assert()
        .success()
        .stdout("alpha\nbeta\n");
}

/// Pin state is read only when JSON reports it, matching the gate on the
/// per-context log scan. Unreadable pin state is fatal to a listing that
/// consults it, so a store holding some proves the plain listing does not.
#[test]
fn context_list_reads_pin_state_only_for_json() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    // A regular file where a scope's pin directory belongs: reading it as a
    // directory fails with something other than "not found".
    let pins_dir = env.cue_store().join(".state/pins/acme");
    std::fs::create_dir_all(&pins_dir).expect("Failed to create pin state dir");
    std::fs::write(pins_dir.join("widgets"), "").expect("Failed to write pin state file");

    env.command()
        .args(["context", "list"])
        .assert()
        .success()
        .stdout("alpha\n");
    env.command()
        .args(["context", "list", "--sort", "recency"])
        .assert()
        .success()
        .stdout("alpha\n");

    env.command()
        .args(["context", "list", "--json"])
        .assert()
        .failure();
}

/// A pin outlives the context it names. Such a pin marks no row, and marks
/// no other context by accident, so it changes nothing about the listing.
#[test]
fn context_list_json_ignores_pins_without_a_context() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();
    env.command()
        .args(["context", "pin", "ghost"])
        .assert()
        .success();

    let contexts = list_json(&env, &["context", "list", "--json"])?;
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0]["context"], "alpha");
    assert_eq!(contexts[0]["pinned"], false);

    Ok(())
}

/// Reporting membership must not introduce a repository lookup where the
/// requested breadth needs none: whole-store JSON remains listable from
/// outside a Git repository.
#[test]
fn context_list_scope_store_json_needs_no_repository_scope() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    let other = env.root().join("other-repo");
    setup_scope_repo(&other, "https://github.com/other/project.git");
    env.command()
        .args(["-C"])
        .arg(&other)
        .args(["context", "create", "roadmap"])
        .assert()
        .success();
    env.command()
        .args(["-C"])
        .arg(&other)
        .args(["context", "pin", "roadmap"])
        .assert()
        .success();

    let contexts = list_json(&env, &["context", "list", "--scope", "store", "--json"])?;
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0]["context"], "roadmap");
    assert_eq!(contexts[0]["pinned"], true);

    Ok(())
}

// ── Metadata filtering ───────────────────────────────────────────────────────

/// The motivating query: the direct children of one context, across every
/// scope in the store. Children are derived by matching the canonical parent
/// address, so a grandchild and a context that merely references the parent
/// are not children and must not be returned.
#[test]
fn context_list_filter_selects_direct_children_across_scopes() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let other = env.root().join("other-repo");
    setup_scope_repo(&other, "https://github.com/other/project.git");

    env.command()
        .args(["context", "create", "program"])
        .assert()
        .success();
    env.command()
        .args([
            "context",
            "create",
            "near-child",
            "--parent",
            "acme/widgets/program",
        ])
        .assert()
        .success();
    env.command()
        .args([
            "context",
            "create",
            "cousin",
            "--ref",
            "acme/widgets/program",
        ])
        .assert()
        .success();
    env.command()
        .args([
            "context",
            "create",
            "grandchild",
            "--parent",
            "acme/widgets/near-child",
        ])
        .assert()
        .success();
    env.command()
        .args(["-C"])
        .arg(&other)
        .args([
            "context",
            "create",
            "far-child",
            "--parent",
            "acme/widgets/program",
        ])
        .assert()
        .success();

    let contexts = list_json(
        &env,
        &[
            "context",
            "list",
            "--scope",
            "store",
            "--json",
            "--filter",
            "parent=acme/widgets/program",
        ],
    )?;

    assert_eq!(contexts.len(), 2);
    assert_eq!(contexts[0]["context"], "near-child");
    assert_eq!(contexts[0]["scope"], "acme/widgets");
    assert_eq!(contexts[1]["context"], "far-child");
    assert_eq!(contexts[1]["scope"], "other/project");

    Ok(())
}

/// Filtering is a property of the query, not of its output format: the plain
/// repository listing narrows on the same expressions, and repeated filters
/// are ANDed as they are in `cue list`.
#[test]
fn context_list_filters_plain_output_on_every_predicate() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for (slug, kind, mode) in [
        ("alpha", "work", "build"),
        ("beta", "work", "review"),
        ("gamma", "coord", "build"),
        ("delta", "work", "build"),
    ] {
        env.command()
            .args(["context", "create", slug, "--kind", kind, "--mode", mode])
            .assert()
            .success();
    }

    env.command()
        .args([
            "context",
            "list",
            "--filter",
            "kind=work",
            "--filter",
            "mode=build",
        ])
        .assert()
        .success()
        .stdout("alpha\ndelta\n");
}

/// Optional context metadata is absent from `context.md` rather than written
/// as null, so a filter must treat it the way `cue list` treats a missing
/// frontmatter field: `=` and `~=` reject it, `!=` accepts it.
#[test]
fn context_list_filter_treats_absent_metadata_as_no_value() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "bare"])
        .assert()
        .success();
    env.command()
        .args([
            "context",
            "create",
            "described",
            "--mode",
            "review",
            "--title",
            "Quarterly release review",
        ])
        .assert()
        .success();

    env.command()
        .args(["context", "list", "--filter", "mode=review"])
        .assert()
        .success()
        .stdout("described\n");

    env.command()
        .args(["context", "list", "--filter", "mode!=review"])
        .assert()
        .success()
        .stdout("bare\n");

    env.command()
        .args(["context", "list", "--filter", "title~=release"])
        .assert()
        .success()
        .stdout("described\n");
}

/// An expression cue cannot parse is a mistake in the query, not a predicate
/// that matches nothing: it is rejected before any context is read.
#[test]
fn context_list_rejects_an_unparseable_filter() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "alpha"])
        .assert()
        .success();

    env.command()
        .args(["context", "list", "--filter", "kindwork"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("=, !=, ~="));

    env.command()
        .args(["context", "list", "--filter", "=work"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("filter key cannot be empty"));
}

/// Selection precedes truncation, so a limit keeps the leading matches rather
/// than filtering whatever survived an arbitrary cut of the unfiltered
/// listing.
#[test]
fn context_list_filters_before_applying_the_limit() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for (slug, kind) in [
        ("alpha", "work"),
        ("beta", "work"),
        ("gamma", "coord"),
        ("omega", "coord"),
    ] {
        env.command()
            .args(["context", "create", slug, "--kind", kind])
            .assert()
            .success();
    }

    // Both matches sort behind two non-matching contexts, so a limit of one
    // over the unfiltered listing would report nothing.
    env.command()
        .args(["context", "list", "--filter", "kind=coord", "--limit", "1"])
        .assert()
        .success()
        .stdout("gamma\n");
}

/// `--pinned` selects what is enumerated and a filter selects what is kept, so
/// the two compose: the query narrows the working set instead of replacing it.
#[test]
fn context_list_filters_the_pinned_working_set() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for (slug, kind) in [("alpha", "work"), ("beta", "reference"), ("gamma", "work")] {
        env.command()
            .args(["context", "create", slug, "--kind", kind])
            .assert()
            .success();
    }
    for slug in ["alpha", "beta"] {
        env.command()
            .args(["context", "pin", slug])
            .assert()
            .success();
    }

    // gamma matches the filter but is unpinned; beta is pinned but does not
    // match.
    env.command()
        .args(["context", "list", "--pinned", "--filter", "kind=work"])
        .assert()
        .success()
        .stdout("alpha\n");

    let contexts = list_json(
        &env,
        &[
            "context",
            "list",
            "--pinned",
            "--json",
            "--filter",
            "kind=work",
        ],
    )?;
    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0]["context"], "alpha");
    assert_eq!(contexts[0]["pinned"], true);

    Ok(())
}

/// Ordering applies to the matches, so a filtered listing is still ordered by
/// the requested key rather than falling back to canonical address order.
#[test]
fn context_list_orders_filtered_contexts_by_recency() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for (slug, kind) in [("alpha", "work"), ("beta", "work"), ("gamma", "reference")] {
        env.command()
            .args(["context", "create", slug, "--kind", kind])
            .assert()
            .success();
    }
    let scope_dir = env.cue_store().join("acme/widgets");
    plant_log_entry(&scope_dir.join("alpha"), 1_000);
    plant_log_entry(&scope_dir.join("beta"), 2_000);
    plant_log_entry(&scope_dir.join("gamma"), 3_000);

    env.command()
        .args([
            "context",
            "list",
            "--filter",
            "kind=work",
            "--sort",
            "recency",
        ])
        .assert()
        .success()
        .stdout("beta\nalpha\n");
}
