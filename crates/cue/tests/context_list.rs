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

/// Selection changes which contexts are reported and in what order; it does
/// not change what a reported context looks like. Recency metadata in JSON is
/// a separate design question, so no field is added here.
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

    let output = env
        .command()
        .args([
            "context", "list", "--sort", "recency", "--limit", "1", "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let contexts: Value = serde_json::from_slice(&output)?;
    let context = &contexts.as_array().expect("contexts should be an array")[0];
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
            "mode",
            "parent",
            "path",
            "refs",
            "scope",
            "title",
        ]
    );

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
