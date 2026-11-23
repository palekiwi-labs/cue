mod helpers;

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const FOO: &str = "palekiwi-labs/cue/mutation-api/task/foo.md";
const IMPL: &str = "palekiwi-labs/cue/mutation-api/plan/impl.md";
const IDEA: &str = "palekiwi-labs/cue/mutation-api/note/idea.md";
const CTX: &str = "palekiwi-labs/cue/mutation-api";
const CHILD: &str = "palekiwi-labs/cue/design-frontmatter-edit-api";
const HOOKS: &str = "palekiwi/palekiwi/cue-hooks";
const INBOX: &str = "palekiwi/palekiwi/inbox-coordinator";

const FOO_TEXT: &str = "---\ntitle: Foo\nstatus: open\npriority: normal\nrefs:\n\
- palekiwi/palekiwi/inbox-coordinator\n- palekiwi/palekiwi/cue-hooks\n\
created_at: 1\n---\n# Foo\n\n---\nbody: kept\n";

fn write(env: &helpers::TestEnv, address: &str, text: &str) {
    let path = env.cue_store().join(address);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn read(env: &helpers::TestEnv, address: &str) -> String {
    std::fs::read_to_string(env.cue_store().join(address)).unwrap()
}

fn fixture() -> helpers::TestEnv {
    let env = helpers::TestEnv::new();
    write(
        &env,
        &format!("{CTX}/context.md"),
        "---\ntitle: Mutation API work\nkind: coord\nmode: design\n\
parent: palekiwi/palekiwi/session-events\ncreated_at: 1\n---\nbriefing\n",
    );
    write(
        &env,
        &format!("{CHILD}/context.md"),
        "---\nkind: work\nparent: palekiwi-labs/cue/mutation-api\ncreated_at: 1\n---\n",
    );
    for context in [HOOKS, INBOX, "palekiwi/palekiwi/session-events"] {
        write(
            &env,
            &format!("{context}/context.md"),
            "---\nkind: work\n---\n",
        );
    }
    write(&env, FOO, FOO_TEXT);
    write(
        &env,
        IMPL,
        "---\nstatus: open\ncreated_at: 1\n---\n# Impl\n",
    );
    write(
        &env,
        IDEA,
        "---\nstatus: open\nrefs:\n- palekiwi/palekiwi/cue-hooks\n---\n# Idea\n",
    );
    write(
        &env,
        "palekiwi-labs/cue/mutation-api/trace/handoff.md",
        "---\nrepo_id: x\n---\n",
    );
    env
}

fn update(env: &helpers::TestEnv, args: &[&str]) -> assert_cmd::assert::Assert {
    env.command().arg("update").args(args).assert()
}

fn stdout(env: &helpers::TestEnv, args: &[&str]) -> String {
    let output = update(env, args).success();
    String::from_utf8(output.get_output().stdout.clone()).unwrap()
}

fn snapshot(env: &helpers::TestEnv) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(dir: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, files);
            } else {
                files.insert(path.clone(), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(env.cue_store(), &mut files);
    files.retain(|path, _| !path.starts_with(env.cue_store().join(".state")));
    files
}

fn fails(env: &helpers::TestEnv, args: &[&str], message: &str) {
    let before = snapshot(env);
    let output = update(env, args).failure();
    let stderr = String::from_utf8(output.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains(message),
        "expected '{message}' in stderr:\n{stderr}"
    );
    assert_eq!(snapshot(env), before);
}

#[test]
fn completing_a_task_edits_only_its_status() {
    let env = fixture();
    let out = stdout(&env, &[FOO, "--status", "complete"]);
    assert_eq!(out, format!("{FOO} status: open -> complete\n"));
    assert_eq!(
        read(&env, FOO),
        FOO_TEXT.replace("status: open", "status: complete")
    );
}

#[test]
fn priority_and_title_change_in_one_write() {
    let env = fixture();
    let out = stdout(
        &env,
        &[
            FOO,
            "--priority",
            "high",
            "--title",
            "Ship the update command",
        ],
    );
    assert_eq!(
        out,
        format!("{FOO} title: Foo -> Ship the update command\n{FOO} priority: normal -> high\n")
    );
    let text = read(&env, FOO);
    assert!(
        text.starts_with("---\ntitle: Ship the update command\nstatus: open\npriority: high\n")
    );
    assert!(text.ends_with("---\n# Foo\n\n---\nbody: kept\n"));
}

#[test]
fn a_plan_is_attached_to_a_task() {
    let env = fixture();
    let out = stdout(&env, &[IMPL, "--parent", FOO]);
    assert_eq!(out, format!("{IMPL} parent: (none) -> {FOO}\n"));
    assert_eq!(
        read(&env, IMPL),
        format!("---\nstatus: open\ncreated_at: 1\nparent: {FOO}\n---\n# Impl\n")
    );
}

#[test]
fn refs_are_added_and_removed_keeping_the_rest() {
    let env = fixture();
    stdout(&env, &[FOO, "--add-ref", IDEA, "--remove-ref", INBOX]);
    assert!(read(&env, FOO).contains(&format!("refs:\n- {HOOKS}\n- {IDEA}\ncreated_at: 1\n")));
}

#[test]
fn refs_are_replaced_with_exactly_the_given_entries() {
    let env = fixture();
    stdout(&env, &[FOO, "--set-refs", IDEA, "--set-refs", HOOKS]);
    assert!(read(&env, FOO).contains(&format!("refs:\n- {IDEA}\n- {HOOKS}\ncreated_at: 1\n")));
}

#[test]
fn a_note_is_closed_and_its_refs_dropped() {
    let env = fixture();
    let out = stdout(&env, &[IDEA, "--status", "closed", "--clear-refs"]);
    assert_eq!(
        out,
        format!("{IDEA} status: open -> closed\n{IDEA} refs: [{HOOKS}] -> (none)\n")
    );
    assert_eq!(read(&env, IDEA), "---\nstatus: closed\n---\n# Idea\n");
}

#[test]
fn dry_run_reports_without_writing() {
    let env = fixture();
    let out = stdout(&env, &[FOO, "--status", "closed", "--dry-run"]);
    assert_eq!(out, format!("{FOO} status: open -> closed\n"));
    assert_eq!(read(&env, FOO), FOO_TEXT);
}

#[test]
fn a_context_is_reparented() {
    let env = fixture();
    stdout(&env, &[CHILD, "--no-parent"]);
    let out = stdout(&env, &[CHILD, "--parent", CTX]);
    assert_eq!(out, format!("{CHILD} parent: (none) -> {CTX}\n"));
    assert!(read(&env, &format!("{CHILD}/context.md")).contains(&format!("parent: {CTX}\n")));
}

#[test]
fn context_mode_and_title_change() {
    let env = fixture();
    stdout(&env, &[CTX, "--mode", "build", "--title", "Mutation API"]);
    let text = read(&env, &format!("{CTX}/context.md"));
    assert!(text.starts_with("---\ntitle: Mutation API\nkind: coord\nmode: build\n"));
    assert!(text.ends_with("---\nbriefing\n"));
}

#[test]
fn a_context_is_made_a_root() {
    let env = fixture();
    let out = stdout(&env, &[CTX, "--no-parent"]);
    assert_eq!(
        out,
        format!("{CTX} parent: palekiwi/palekiwi/session-events -> (none)\n")
    );
    assert!(!read(&env, &format!("{CTX}/context.md")).contains("parent"));
}

#[test]
fn json_output_carries_the_mutation() {
    let env = fixture();
    let out = stdout(&env, &[FOO, "--status", "complete", "--json"]);
    let json: Value = serde_json::from_str(&out).unwrap();
    assert!(
        json["mutation_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
    );
    assert_eq!(json["address"], FOO);
    assert_eq!(
        json["changes"],
        serde_json::json!([{ "field": "status", "old": "open", "new": "complete" }])
    );
}

#[test]
fn an_update_that_changes_nothing_writes_nothing() {
    let env = fixture();
    let path = env.cue_store().join(FOO);
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    let out = stdout(&env, &[FOO, "--status", "open", "--add-ref", HOOKS]);
    assert_eq!(out, "");
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        modified
    );
    let json: Value =
        serde_json::from_str(&stdout(&env, &[FOO, "--status", "open", "--json"])).unwrap();
    assert_eq!(json["changes"], serde_json::json!([]));
}

#[test]
fn priority_is_not_a_field_of_plan() {
    fails(
        &fixture(),
        &[IMPL, "--priority", "high"],
        "--priority is not a field of plan",
    );
}

#[test]
fn status_is_not_a_field_of_context() {
    fails(
        &fixture(),
        &[CTX, "--status", "complete"],
        "--status is not a field of context",
    );
}

#[test]
fn unknown_status_values_are_rejected() {
    let env = fixture();
    fails(
        &env,
        &[FOO, "--status", "done"],
        "invalid value 'done' for '--status",
    );
    fails(
        &env,
        &[FOO, "--status", "done"],
        "[possible values: open, in-progress, complete, closed]",
    );
}

#[test]
fn a_missing_parent_is_rejected() {
    let missing = "palekiwi-labs/cue/mutation-api/task/missing.md";
    fails(
        &fixture(),
        &[FOO, "--parent", missing],
        &format!("parent not found: {missing}"),
    );
}

#[test]
fn a_plan_cannot_be_parented_to_a_note() {
    fails(
        &fixture(),
        &[IMPL, "--parent", IDEA],
        "a plan's parent must be a task or a context",
    );
}

#[test]
fn context_parent_cycles_are_rejected() {
    fails(
        &fixture(),
        &[CTX, "--parent", CHILD],
        &format!("cycle: {CHILD} is below {CTX}"),
    );
}

#[test]
fn a_context_cannot_be_parented_to_an_artifact() {
    fails(
        &fixture(),
        &[CTX, "--parent", FOO],
        "a context's parent must be a context",
    );
}

#[test]
fn partial_addresses_are_rejected() {
    fails(
        &fixture(),
        &["task/foo.md", "--status", "complete"],
        "expected a canonical address\n  (<org>/<repo>/<context> or <org>/<repo>/<context>/<type>/<name>)",
    );
}

#[test]
fn additive_and_replacing_refs_flags_do_not_mix() {
    fails(
        &fixture(),
        &[FOO, "--add-ref", HOOKS, "--set-refs", IDEA],
        "--add-ref cannot be used with --set-refs",
    );
    fails(
        &fixture(),
        &[FOO, "--set-refs", IDEA, "--clear-refs"],
        "--set-refs cannot be used with --clear-refs",
    );
}

#[test]
fn added_refs_must_exist() {
    let missing = "palekiwi-labs/cue/mutation-api/note/missing.md";
    fails(
        &fixture(),
        &[FOO, "--add-ref", missing],
        &format!("ref not found: {missing}"),
    );
}

#[test]
fn required_and_stamped_fields_have_no_flags() {
    let env = fixture();
    fails(
        &env,
        &[FOO, "--no-status"],
        "unexpected argument '--no-status'",
    );
    fails(
        &env,
        &[FOO, "--created-at", "0"],
        "unexpected argument '--created-at'",
    );
}

#[test]
fn parent_cannot_be_set_and_cleared_together() {
    fails(
        &fixture(),
        &[FOO, "--parent", CTX, "--no-parent"],
        "cannot be used with",
    );
}

#[test]
fn trace_artifacts_cannot_be_updated() {
    fails(
        &fixture(),
        &[
            "palekiwi-labs/cue/mutation-api/trace/handoff.md",
            "--status",
            "complete",
        ],
        "trace artifacts cannot be updated",
    );
}

#[test]
fn refs_that_push_frontmatter_past_the_read_cap_are_refused() {
    let env = fixture();
    let long = "palekiwi-labs/cue/mutation-api/note/long.md";
    let refs: String = (0..62)
        .map(|i| format!("- palekiwi/palekiwi/gone-{i}\n"))
        .collect();
    write(&env, long, &format!("---\nrefs:\n{refs}---\n# Long\n"));
    fails(
        &env,
        &[long, "--add-ref", HOOKS],
        "frontmatter would exceed 63 lines; artifact would be hidden from listings",
    );
}
