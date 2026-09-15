mod helpers;

use predicates::prelude::*;

/// Pin state is a directory of zero-byte marker files under the store root,
/// so a pin is a file create and an unpin is a file unlink.
#[test]
fn context_pin_writes_a_zero_byte_marker_under_the_store_state_directory() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    env.command()
        .args(["context", "pin", "data-model-spike"])
        .assert()
        .success();

    let marker = env
        .cue_store()
        .join(".state/pins/acme/widgets/data-model-spike");
    assert!(marker.is_file(), "pin should create a marker file");
    assert_eq!(std::fs::metadata(&marker)?.len(), 0);

    Ok(())
}

/// Pinning an existing entry is idempotent: the create must not fail and must
/// not rewrite the marker.
///
/// A real marker is zero-byte, so its size alone cannot tell "left untouched"
/// apart from "truncated and rewritten to empty". The test therefore writes a
/// sentinel into the marker purely as an observable; surviving the second pin
/// is what proves the create neither truncated nor rewrote the file.
#[test]
fn context_pin_is_idempotent_and_does_not_rewrite_the_marker() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    env.command()
        .args(["context", "pin", "release"])
        .assert()
        .success()
        .stdout("pinned acme/widgets/release\n");

    let marker = env.cue_store().join(".state/pins/acme/widgets/release");
    assert_eq!(std::fs::metadata(&marker)?.len(), 0);
    std::fs::write(&marker, "sentinel")?;
    let written_at = std::fs::metadata(&marker)?.modified()?;

    env.command()
        .args(["context", "pin", "release"])
        .assert()
        .success()
        .stdout("pinned acme/widgets/release\n");

    assert!(marker.is_file());
    assert_eq!(std::fs::read_to_string(&marker)?, "sentinel");
    assert_eq!(std::fs::metadata(&marker)?.modified()?, written_at);

    Ok(())
}

/// Pin resolves address shape only; it never checks that the context exists.
#[test]
fn context_pin_does_not_require_the_context_to_exist() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    assert!(!env.cue_store().join("acme/widgets").exists());

    env.command()
        .args(["context", "pin", "never-created"])
        .assert()
        .success();

    assert!(
        env.cue_store()
            .join(".state/pins/acme/widgets/never-created")
            .is_file()
    );
}

/// A full `<org>/<repo>/<slug>` address pins a context in another scope
/// without changing directory.
#[test]
fn context_pin_accepts_a_canonical_address_in_another_scope() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    env.command()
        .args(["context", "pin", "other/project/roadmap"])
        .assert()
        .success();

    assert!(
        env.cue_store()
            .join(".state/pins/other/project/roadmap")
            .is_file()
    );
    assert!(!env.cue_store().join(".state/pins/acme").exists());
}

/// Unpin unlinks the marker, leaving the now-empty scope directory in place:
/// pruning would reintroduce a write race.
#[test]
fn context_unpin_unlinks_the_marker_without_pruning_scope_directories() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "pin", "release"])
        .assert()
        .success();

    env.command()
        .args(["context", "unpin", "release"])
        .assert()
        .success();

    assert!(
        !env.cue_store()
            .join(".state/pins/acme/widgets/release")
            .exists()
    );
    assert!(
        env.cue_store().join(".state/pins/acme/widgets").is_dir(),
        "an emptied scope directory must not be pruned"
    );
}

/// Unpinning an absent entry is idempotent, including when no pin state has
/// ever been written.
#[test]
fn context_unpin_is_idempotent_for_absent_entries() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    assert!(!env.cue_store().join(".state").exists());

    env.command()
        .args(["context", "unpin", "never-pinned"])
        .assert()
        .success();

    env.command()
        .args(["context", "unpin", "other/project/never-pinned"])
        .assert()
        .success();
}

/// The default view is the current repository scope, printed as one canonical
/// context address per line, ordered alphabetically.
#[test]
fn context_pins_lists_the_current_scope_as_canonical_addresses_in_order() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for slug in ["zeta", "alpha", "middle"] {
        env.command()
            .args(["context", "pin", slug])
            .assert()
            .success();
    }
    env.command()
        .args(["context", "pin", "other/project/roadmap"])
        .assert()
        .success();

    env.command()
        .args(["context", "pins"])
        .assert()
        .success()
        .stdout("acme/widgets/alpha\nacme/widgets/middle\nacme/widgets/zeta\n");
}

/// `--scope repo` is the default, so naming it explicitly must not change
/// what is listed.
#[test]
fn context_pins_scope_repo_matches_the_default_view() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for context in ["zeta", "alpha", "other/project/roadmap"] {
        env.command()
            .args(["context", "pin", context])
            .assert()
            .success();
    }

    env.command()
        .args(["context", "pins", "--scope", "repo"])
        .assert()
        .success()
        .stdout("acme/widgets/alpha\nacme/widgets/zeta\n");
}

/// `--scope store` widens the same shape to the whole store, so no consumer
/// branches on which view produced a line.
#[test]
fn context_pins_scope_store_lists_every_scope_in_order() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for context in [
        "acme/widgets/zeta",
        "other/project/roadmap",
        "acme/widgets/alpha",
        "abacus/tools/build",
    ] {
        env.command()
            .args(["context", "pin", context])
            .assert()
            .success();
    }

    env.command()
        .args(["context", "pins", "--scope", "store"])
        .assert()
        .success()
        .stdout(
            "abacus/tools/build\nacme/widgets/alpha\nacme/widgets/zeta\nother/project/roadmap\n",
        );
}

/// Scope is a closed vocabulary of `repo|store`; anything else is rejected
/// rather than silently widened or narrowed.
#[test]
fn context_pins_rejects_an_unknown_scope_value() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    for invalid in ["", "all", "Repo", "global", "repository"] {
        env.command()
            .args(["context", "pins", "--scope", invalid])
            .assert()
            .failure()
            .stderr(predicate::str::contains("repo"))
            .stderr(predicate::str::contains("store"));
    }

    assert!(!env.cue_store().join(".state").exists());
}

/// `--all` is replaced by `--scope store` with no compatibility alias, so the
/// old flag must fail rather than keep working.
#[test]
fn context_pins_rejects_the_removed_all_flag() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    env.command()
        .args(["context", "pins", "--all"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--all"));
}

/// A missing pins directory is an empty working set, not an error.
#[test]
fn context_pins_succeeds_with_no_output_when_pin_state_is_missing() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    assert!(!env.cue_store().join(".state").exists());

    env.command()
        .args(["context", "pins"])
        .assert()
        .success()
        .stdout("");

    env.command()
        .args(["context", "pins", "--scope", "store"])
        .assert()
        .success()
        .stdout("");
}

/// Emptied scope directories are retained rather than pruned, so listing must
/// skip them in both views.
#[test]
fn context_pins_skips_empty_scope_directories() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    for context in ["release", "other/project/roadmap"] {
        env.command()
            .args(["context", "pin", context])
            .assert()
            .success();
    }
    env.command()
        .args(["context", "unpin", "other/project/roadmap"])
        .assert()
        .success();
    assert!(env.cue_store().join(".state/pins/other/project").is_dir());

    env.command()
        .args(["context", "pins", "--scope", "store"])
        .assert()
        .success()
        .stdout("acme/widgets/release\n");

    env.command()
        .args(["context", "unpin", "release"])
        .assert()
        .success();

    env.command()
        .args(["context", "pins"])
        .assert()
        .success()
        .stdout("");
}

/// A pin whose context no longer exists prints as an address that resolves to
/// nothing: cue adds no stale-pin validation or cleanup.
#[test]
fn context_pins_retains_pins_for_contexts_that_do_not_exist() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "create", "present"])
        .assert()
        .success();
    for slug in ["present", "vanished"] {
        env.command()
            .args(["context", "pin", slug])
            .assert()
            .success();
    }

    env.command()
        .args(["context", "pins"])
        .assert()
        .success()
        .stdout("acme/widgets/present\nacme/widgets/vanished\n");
}

/// Joining the scope directories and the filename is what yields an address,
/// so the whole-store view walks `<org>/<repo>/<slug>` and ignores anything
/// that does not have that shape rather than failing on it.
#[test]
fn context_pins_scope_store_ignores_entries_that_are_not_scope_directories() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    env.command()
        .args(["context", "pin", "release"])
        .assert()
        .success();
    let pins = env.cue_store().join(".state/pins");
    std::fs::write(pins.join("stray"), "")?;
    std::fs::write(pins.join("acme/stray"), "")?;

    env.command()
        .args(["context", "pins", "--scope", "store"])
        .assert()
        .success()
        .stdout("acme/widgets/release\n");

    Ok(())
}

/// A pin argument becomes a path under the store's state directory, so every
/// segment must be a single safe path segment and only the two accepted
/// shapes resolve.
#[test]
fn context_pin_rejects_unsafe_and_non_canonical_shapes() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    for invalid in [
        "",
        "..",
        "../evil",
        "../../etc/passwd",
        "/etc/passwd",
        "/acme/widgets/release",
        "~/cue/acme/widgets/release",
        "acme/widgets",
        "acme/widgets/release/spec/index.md",
        "acme/../widgets/release",
        "acme/widgets/..",
        "acme//release",
        "acme/widgets/release/",
        // Traversal that already has the accepted three-segment shape, so
        // only per-segment validation can stop it escaping the pins
        // directory.
        "../etc/passwd",
        "acme/../release",
    ] {
        for command in ["pin", "unpin"] {
            env.command()
                .args(["context", command, invalid])
                .assert()
                .failure()
                .stderr(predicate::str::contains("<org>/<repo>/<slug>"));
        }
    }

    assert!(
        !env.cue_store().join(".state").exists(),
        "a rejected pin must write no state"
    );
    assert!(!env.root().join("etc").exists());
}

/// A pin argument becomes both a path under the store's state directory and a
/// line of `pins` output, so no segment may carry a line break or consist of
/// whitespace alone, and no value may be home-relative: `~` is a shell and
/// path convention with no meaning as an address.
#[test]
fn context_pin_rejects_line_breaks_whitespace_only_segments_and_home_relative_forms() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    for invalid in [
        "rel\nease",
        "rel\rease",
        "\n",
        "acme/widgets/rel\nease",
        "acme/wid\rgets/release",
        "acme/widgets/release\n",
        " ",
        "\t",
        "acme/ /release",
        "acme/widgets/ ",
        "~release",
        "~/release",
        "~/cue/release",
        "~/acme/widgets",
    ] {
        for command in ["pin", "unpin"] {
            env.command()
                .args(["context", command, invalid])
                .assert()
                .failure()
                .stderr(predicate::str::contains("<org>/<repo>/<slug>"));
        }
    }

    assert!(
        !env.cue_store().join(".state").exists(),
        "a rejected pin must write no state"
    );
}

/// Only line breaks and whitespace-only segments are rejected: an ordinary
/// internal space is an ordinary character of the slug and must survive
/// unchanged into both the marker name and the printed address.
#[test]
fn context_pin_preserves_an_internal_space_in_a_slug() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    env.command()
        .args(["context", "pin", "my release"])
        .assert()
        .success()
        .stdout("pinned acme/widgets/my release\n");

    assert!(
        env.cue_store()
            .join(".state/pins/acme/widgets/my release")
            .is_file()
    );

    env.command()
        .args(["context", "pins"])
        .assert()
        .success()
        .stdout("acme/widgets/my release\n");
}

/// The three commands compose on their real output: a bare slug pins into the
/// current scope, `pins` prints that pin as a canonical address, and feeding
/// that exact line back to `unpin` empties the working set.
#[test]
fn context_pin_pins_and_unpin_round_trip_on_real_output() -> anyhow::Result<()> {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    env.command()
        .args(["context", "pin", "release"])
        .assert()
        .success()
        .stdout("pinned acme/widgets/release\n");

    let listed = env.command().args(["context", "pins"]).assert().success();
    let listed = String::from_utf8(listed.get_output().stdout.clone())?;
    let address = listed.strip_suffix('\n').unwrap_or(&listed);
    assert_eq!(address, "acme/widgets/release");

    env.command()
        .args(["context", "unpin", address])
        .assert()
        .success()
        .stdout("unpinned acme/widgets/release\n");

    env.command()
        .args(["context", "pins"])
        .assert()
        .success()
        .stdout("");

    Ok(())
}

/// An explicit `--store` overrides `$CUE_STORE` for pin state, which resolves
/// through the ordinary store root.
#[test]
fn context_pin_honors_the_global_store_flag() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();
    let flag_store = env.root().join("flag-home");

    env.command()
        .args(["--store"])
        .arg(&flag_store)
        .args(["context", "pin", "release"])
        .assert()
        .success();

    assert!(
        flag_store
            .join(".state/pins/acme/widgets/release")
            .is_file()
    );
    assert!(
        !env.cue_store().join(".state").exists(),
        "--store must override $CUE_STORE for pin state"
    );

    env.command()
        .args(["--store"])
        .arg(&flag_store)
        .args(["context", "pins"])
        .assert()
        .success()
        .stdout("acme/widgets/release\n");

    env.command()
        .args(["context", "pins"])
        .assert()
        .success()
        .stdout("");

    env.command()
        .args(["--store"])
        .arg(&flag_store)
        .args(["context", "unpin", "release"])
        .assert()
        .success();

    assert!(!flag_store.join(".state/pins/acme/widgets/release").exists());
}

/// The address form needs no repository scope, so it works from anywhere; the
/// whole-store view likewise resolves no scope.
#[test]
fn context_pin_by_address_needs_no_repository_scope() {
    let env = helpers::TestEnv::new();

    env.command()
        .args(["context", "pin", "other/project/roadmap"])
        .assert()
        .success();

    env.command()
        .args(["context", "pins", "--scope", "store"])
        .assert()
        .success()
        .stdout("other/project/roadmap\n");

    env.command().args(["context", "pins"]).assert().failure();
}

/// `pins` answers which contexts are in the working set; `cue context list
/// --json` answers what they are. There is no `--json` here.
#[test]
fn context_pins_has_no_json_output() {
    let env = helpers::TestEnv::new();
    env.setup_repo_with_origin();

    env.command()
        .args(["context", "pins", "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--json"));
}
