mod helpers;

use serde_json::Value;

const FOO: &str = "palekiwi-labs/cue/mutation-api/task/foo.md";
const IMPL: &str = "palekiwi-labs/cue/mutation-api/plan/impl.md";
const IDEA: &str = "palekiwi-labs/cue/mutation-api/note/idea.md";
const DUP: &str = "palekiwi-labs/cue/mutation-api/note/dup.md";
const HANDOFF: &str = "palekiwi-labs/cue/mutation-api/trace/handoff.md";
const DIFF: &str = "palekiwi-labs/cue/mutation-api/tmp/1-abc/feat.diff";
const CTX: &str = "palekiwi-labs/cue/mutation-api";
const LOG: &str = "palekiwi-labs/cue/mutation-api/log/1.json";

fn write(env: &helpers::TestEnv, address: &str, text: &str) {
    let path = env.cue_store().join(address);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn exists(env: &helpers::TestEnv, address: &str) -> bool {
    env.cue_store().join(address).exists()
}

fn fixture() -> helpers::TestEnv {
    let env = helpers::TestEnv::new();
    write(
        &env,
        &format!("{CTX}/context.md"),
        "---\nkind: coord\n---\n",
    );
    write(
        &env,
        FOO,
        "---\nstatus: open\nrefs:\n- palekiwi-labs/cue/mutation-api/note/idea.md\n---\n",
    );
    write(
        &env,
        IMPL,
        "---\nparent: palekiwi-labs/cue/mutation-api/task/foo.md\n---\n",
    );
    write(&env, IDEA, "---\nstatus: open\n---\n");
    write(
        &env,
        DUP,
        "---\nstatus: open\nrefs:\n- palekiwi-labs/cue/mutation-api/note/dup.md\n---\n",
    );
    write(&env, HANDOFF, "---\nrepo_id: x\n---\n");
    write(
        &env,
        LOG,
        "{\"title\": \"t\", \"trace\": \"palekiwi-labs/cue/mutation-api/trace/handoff.md\"}",
    );
    write(&env, DIFF, "diff");
    env
}

fn delete(env: &helpers::TestEnv, args: &[&str]) -> assert_cmd::assert::Assert {
    env.command().arg("delete").args(args).assert()
}

fn stderr(assert: assert_cmd::assert::Assert) -> String {
    String::from_utf8(assert.get_output().stderr.clone()).unwrap()
}

#[test]
fn an_unlinked_artifact_is_deleted() {
    let env = fixture();
    let output = delete(&env, &[DUP]).success();
    assert_eq!(
        String::from_utf8(output.get_output().stdout.clone()).unwrap(),
        format!("{DUP} deleted\n")
    );
    assert!(!exists(&env, DUP));
}

#[test]
fn a_dry_run_deletes_nothing() {
    let env = fixture();
    let output = delete(&env, &[DUP, "--dry-run"]).success();
    assert!(
        String::from_utf8(output.get_output().stdout.clone())
            .unwrap()
            .contains("would be deleted")
    );
    assert!(exists(&env, DUP));
}

#[test]
fn a_parent_is_refused_and_its_children_listed() {
    let env = fixture();
    let err = stderr(delete(&env, &[FOO]).failure());
    assert!(err.contains(&format!("{IMPL} (parent)")), "{err}");
    assert!(exists(&env, FOO));
}

#[test]
fn a_referenced_artifact_is_refused() {
    let env = fixture();
    let err = stderr(delete(&env, &[IDEA, "--dry-run"]).failure());
    assert!(err.contains(&format!("{FOO} (refs)")), "{err}");
    assert!(exists(&env, IDEA));
}

#[test]
fn links_from_contexts_and_review_json_block() {
    let env = fixture();
    write(
        &env,
        "palekiwi/palekiwi/other/context.md",
        &format!("---\nparent: {DUP}\n---\n"),
    );
    write(
        &env,
        "palekiwi/palekiwi/other/review/r.json",
        &format!("{{\"refs\": [\"{HANDOFF}\"]}}"),
    );
    let err = stderr(delete(&env, &[DUP]).failure());
    assert!(err.contains("palekiwi/palekiwi/other (parent)"), "{err}");
    let err = stderr(delete(&env, &[HANDOFF]).failure());
    assert!(
        err.contains("palekiwi/palekiwi/other/review/r.json (refs)"),
        "{err}"
    );
}

#[test]
fn a_log_trace_warns_but_does_not_block() {
    let env = fixture();
    let output = delete(&env, &[HANDOFF, "--json"]).success();
    let json: Value = serde_json::from_slice(&output.get_output().stdout).unwrap();
    assert_eq!(json["address"], HANDOFF);
    assert_eq!(json["warnings"][0], LOG);
    assert!(
        json["mutation_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
    );
    assert!(!exists(&env, HANDOFF));
    assert!(exists(&env, LOG));
}

#[test]
fn a_tmp_file_is_deleted_with_its_empty_directory() {
    let env = fixture();
    delete(&env, &[DIFF]).success();
    assert!(!exists(&env, "palekiwi-labs/cue/mutation-api/tmp/1-abc"));
}

#[test]
fn invalid_targets_are_refused() {
    let env = fixture();
    for (address, message) in [
        (CTX, "contexts cannot be deleted"),
        (LOG, "log entries cannot be deleted"),
        ("palekiwi-labs/cue/mutation-api/tmp/1-abc", "not a file"),
        ("palekiwi-labs/cue/mutation-api/note/nope.md", "no note at"),
        (
            "palekiwi-labs/cue/mutation-api/widget/x.md",
            "unknown artifact type",
        ),
    ] {
        let err = stderr(delete(&env, &[address]).failure());
        assert!(err.contains(message), "{address}: {err}");
    }
}
