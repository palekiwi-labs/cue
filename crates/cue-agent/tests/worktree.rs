//! New-worktree execution: creation, ownership, retention and cleanup, using
//! disposable repositories and the real `git worktree` commands.

mod helpers;

use helpers::{Sandbox, branches, git, init_repo, receipt, sorted, worktrees, write_executable};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const MANIFEST: &str = r#"{"agents": {"alpha": {}, "beta": {}}}"#;

struct Fixture {
    sandbox: Sandbox,
    /// The sandbox directory with symlinks resolved, so paths compare equal to
    /// what Git and the harness report.
    root: PathBuf,
}

impl Fixture {
    fn new(manifest: &str) -> Self {
        let sandbox = Sandbox::new();
        sandbox.global_manifest(manifest);
        let root = sandbox.dir.path().canonicalize().unwrap();
        Self { sandbox, root }
    }

    fn repo(&self, name: &str) -> PathBuf {
        let dir = self.root.join("repos").join(name);
        init_repo(&dir, &format!("readme of {name}\n"));
        dir
    }

    fn cmd(&self) -> Command {
        let mut cmd = self.sandbox.cmd();
        cmd.env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1");
        cmd
    }

    fn run(&self, spec: &Value) -> Output {
        self.cmd()
            .args(["run", "--json"])
            .arg(spec.to_string())
            .output()
            .expect("run cue-agent")
    }

    /// The working directory the harness for `run` started in.
    fn launched_in(&self, run: &Value) -> PathBuf {
        PathBuf::from(self.sandbox.recorded_cwd(run["run_id"].as_str().unwrap()))
    }
}

fn runs_of(output: &Output) -> Vec<Value> {
    receipt(&output.stdout)["runs"].as_array().unwrap().clone()
}

/// The request record written before launch.
fn manifest_of(run: &Value) -> Value {
    let path = Path::new(run["run_path"].as_str().unwrap()).join("manifest.json");
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn error_of(run: &Value) -> String {
    run["error"].as_str().unwrap_or_default().to_string()
}

#[test]
fn an_ephemeral_worktree_runs_at_its_checkout_root_and_is_removed_dirty() {
    let fx = Fixture::new(MANIFEST);
    let repo = fx.repo("one");
    // A relative path resolves against the task's cwd (a subdirectory), not
    // against the Git root: from the root it would land one level higher.
    let output = fx.run(&json!({"tasks": [{
        "agent": "alpha", "prompt": "DIRTY", "cwd": repo.join("sub"),
        "worktree": {"base": "main", "ephemeral": true, "path": "../../rel-wt"}
    }]}));

    assert!(output.status.success(), "{output:?}");
    let runs = runs_of(&output);
    let run = &runs[0];
    assert_eq!(run["outcome"], "completed", "{run}");
    let checkout = fx.root.join("repos").join("rel-wt");
    assert_eq!(
        fx.launched_in(run),
        checkout,
        "runs at the new checkout root"
    );
    assert_eq!(run["response"], format!("dirtied {}", checkout.display()));
    assert!(
        run.get("worktree").is_none(),
        "no details for disposed work: {run}"
    );
    assert_eq!(run["cleanup_errors"], json!([]));

    let recorded = manifest_of(run);
    let branch = recorded["worktree"]["branch"].as_str().unwrap().to_string();
    assert!(branch.starts_with("cue-agent/"), "{recorded}");
    assert_eq!(recorded["cwd"], json!(checkout));

    assert!(!checkout.exists(), "the dirty checkout is deleted");
    assert_eq!(branches(&repo), ["main"], "the created branch is deleted");
    assert_eq!(worktrees(&repo), sorted(&[&repo]));
    assert_eq!(
        git(&repo, &["status", "--porcelain"]),
        "",
        "source untouched"
    );
}

#[test]
fn persistent_work_is_retained_and_only_generated_identifiers_are_returned() {
    let fx = Fixture::new(r#"{"worktree_root": "../trees", "agents": {"alpha": {}}}"#);
    let repo = fx.repo("one");
    let explicit = fx.root.join("explicit-wt");
    let output = fx.run(&json!({
        "defaults": {"cwd": repo},
        "tasks": [
            {"agent": "alpha", "prompt": "DIRTY",
             "worktree": {"base": "main", "ephemeral": false}},
            {"agent": "alpha", "prompt": "DIRTY",
             "worktree": {"base": "main", "ephemeral": false,
                          "path": explicit, "branch": "named"}}
        ]
    }));

    assert!(output.status.success(), "{output:?}");
    let runs = runs_of(&output);
    // A relative root resolves against the target directory.
    let generated = &runs[0]["worktree"];
    let path = PathBuf::from(generated["path"].as_str().unwrap());
    let branch = generated["branch"].as_str().unwrap().to_string();
    assert_eq!(path.parent().unwrap(), fx.root.join("repos").join("trees"));
    assert_eq!(fx.launched_in(&runs[0]), path);
    assert_eq!(
        std::fs::read_to_string(path.join("README.md")).unwrap(),
        "changed\n"
    );
    assert!(
        path.join("untracked.txt").exists(),
        "retained work survives"
    );

    // Caller-chosen identifiers are not echoed back.
    assert!(runs[1].get("worktree").is_none(), "{}", runs[1]);
    assert!(explicit.join("untracked.txt").exists());

    assert_eq!(branches(&repo), {
        let mut expected = vec![branch, "main".to_string(), "named".to_string()];
        expected.sort();
        expected
    });
    assert_eq!(worktrees(&repo), sorted(&[&repo, &path, &explicit]));
}

#[test]
fn absolute_roots_serve_several_repositories_with_distinct_generated_names() {
    let fx = Fixture::new(MANIFEST);
    let trees = fx.root.join("abs-trees");
    fx.sandbox.global_manifest(&format!(
        r#"{{"worktree_root": {}, "agents": {{"alpha": {{}}}}}}"#,
        json!(trees)
    ));
    let one = fx.repo("one");
    let two = fx.repo("two");
    let task = |cwd: &Path| {
        json!({"agent": "alpha", "prompt": "hello", "cwd": cwd,
               "worktree": {"base": "main", "ephemeral": false}})
    };
    let output = fx.run(&json!({"tasks": [task(&one), task(&two), task(&one)]}));

    assert!(output.status.success(), "{output:?}");
    let runs = runs_of(&output);
    let paths: Vec<PathBuf> = runs
        .iter()
        .map(|run| PathBuf::from(run["worktree"]["path"].as_str().unwrap()))
        .collect();
    for path in &paths {
        assert_eq!(path.parent().unwrap(), trees);
    }
    assert_ne!(paths[0], paths[2], "repeated tasks get distinct checkouts");
    assert_eq!(
        std::fs::read_to_string(paths[1].join("README.md")).unwrap(),
        "readme of two\n",
        "each checkout comes from its own target repository"
    );
    assert_eq!(branches(&one).len(), 3, "{:?}", branches(&one));
    assert_eq!(branches(&two).len(), 2, "{:?}", branches(&two));
    assert_eq!(worktrees(&one), sorted(&[&one, &paths[0], &paths[2]]));
    assert_eq!(worktrees(&two), sorted(&[&two, &paths[1]]));
}

#[test]
fn collisions_and_bad_requests_fail_only_their_task_and_touch_nothing_preexisting() {
    let fx = Fixture::new(MANIFEST);
    let repo = fx.repo("one");
    git(&repo, &["branch", "taken"]);
    let taken = git(&repo, &["rev-parse", "taken"]);
    let occupied = fx.root.join("occupied");
    std::fs::create_dir_all(&occupied).unwrap();
    std::fs::write(occupied.join("keep.txt"), "mine").unwrap();
    let not_git = fx.root.join("plain");
    std::fs::create_dir_all(&not_git).unwrap();
    let wt = |extra: Value| {
        let mut worktree = json!({"base": "main", "ephemeral": false});
        worktree
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        worktree
    };
    let output = fx.run(&json!({
        "defaults": {"cwd": repo},
        "tasks": [
            {"agent": "alpha", "prompt": "a",
             "worktree": wt(json!({"branch": "taken", "path": fx.root.join("p0")}))},
            {"agent": "alpha", "prompt": "b", "worktree": wt(json!({"path": occupied}))},
            {"agent": "beta", "prompt": "c",
             "worktree": wt(json!({"branch": "dup", "path": fx.root.join("p2")}))},
            {"agent": "beta", "prompt": "d",
             "worktree": wt(json!({"branch": "dup", "path": fx.root.join("p3")}))}
        ]
    }));

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let runs = runs_of(&output);
    assert_eq!(runs[0]["outcome"], "failed");
    assert!(error_of(&runs[0]).contains("taken"), "{}", runs[0]);
    assert_eq!(runs[1]["outcome"], "failed");
    assert!(error_of(&runs[1]).contains("already exists"), "{}", runs[1]);
    assert_eq!(runs[2]["outcome"], "completed", "unaffected tasks continue");
    assert_eq!(runs[3]["outcome"], "failed");
    assert!(error_of(&runs[3]).contains("dup"), "{}", runs[3]);
    for run in [&runs[0], &runs[1], &runs[3]] {
        assert_eq!(run["cleanup_errors"], json!([]), "{run}");
        assert!(run.get("worktree").is_none(), "{run}");
    }
    assert_eq!(
        fx.sandbox.recorded_sessions().len(),
        1,
        "only one harness ran"
    );

    assert_eq!(
        git(&repo, &["rev-parse", "taken"]),
        taken,
        "pre-existing branch untouched"
    );
    assert_eq!(
        std::fs::read_to_string(occupied.join("keep.txt")).unwrap(),
        "mine"
    );
    assert!(!fx.root.join("p0").exists() && !fx.root.join("p3").exists());
    assert!(
        fx.root.join("p2").exists(),
        "the first claimant keeps its checkout"
    );
    assert_eq!(branches(&repo), ["dup", "main", "taken"]);
    assert_eq!(worktrees(&repo), sorted(&[&repo, &fx.root.join("p2")]));

    // An unknown base and a target outside any repository are also per-task
    // operational failures, not admission errors.
    let output = fx.run(&json!({"tasks": [
        {"agent": "alpha", "prompt": "e", "cwd": repo,
         "worktree": {"base": "no-such-rev", "ephemeral": true, "path": fx.root.join("p4")}},
        {"agent": "alpha", "prompt": "f", "cwd": not_git,
         "worktree": {"base": "main", "ephemeral": true, "path": fx.root.join("p5")}},
        {"agent": "beta", "prompt": "g", "cwd": repo}
    ]}));
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let runs = runs_of(&output);
    assert_eq!(runs[0]["outcome"], "failed");
    assert!(error_of(&runs[0]).contains("no-such-rev"), "{}", runs[0]);
    assert_eq!(runs[1]["outcome"], "failed");
    assert_eq!(runs[2]["outcome"], "completed");
    assert!(!fx.root.join("p4").exists() && !fx.root.join("p5").exists());
    assert_eq!(branches(&repo), ["dup", "main", "taken"]);
}

#[test]
fn a_failure_after_branch_creation_removes_what_the_task_created() {
    let fx = Fixture::new(MANIFEST);
    let repo = fx.repo("one");
    // The destination's parent is a regular file, so the checkout cannot be
    // created after the branch already has been.
    let blocker = fx.root.join("blocker");
    std::fs::write(&blocker, "file").unwrap();

    let hooked = fx.repo("hooked");
    let hooks = fx.root.join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    // The checkout is registered and populated, then `git worktree add`
    // reports failure: everything it made belongs to the task.
    write_executable(
        &hooks.join("post-checkout"),
        "#!/usr/bin/env bash\nexit 3\n",
    );
    git(
        &hooked,
        &["config", "core.hooksPath", hooks.to_str().unwrap()],
    );
    let made = fx.root.join("made").join("deep").join("wt");

    let output = fx.run(&json!({"tasks": [
        {"agent": "alpha", "prompt": "a", "cwd": repo,
         "worktree": {"base": "main", "ephemeral": false, "path": blocker.join("wt")}},
        {"agent": "alpha", "prompt": "b", "cwd": hooked,
         "worktree": {"base": "main", "ephemeral": false, "path": made}},
        {"agent": "beta", "prompt": "c", "cwd": repo,
         "worktree": {"base": "main", "ephemeral": true, "path": fx.root.join("fine")}}
    ]}));

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let runs = runs_of(&output);
    for run in &runs[..2] {
        assert_eq!(run["outcome"], "failed", "{run}");
        assert!(error_of(run).contains("worktree"), "{run}");
        assert_eq!(run["cleanup_errors"], json!([]), "{run}");
        assert!(
            run.get("worktree").is_none(),
            "not presented as retained: {run}"
        );
    }
    assert_eq!(runs[2]["outcome"], "completed", "{}", runs[2]);
    assert_eq!(fx.sandbox.recorded_sessions().len(), 1);

    assert_eq!(branches(&repo), ["main"], "the partial branch is deleted");
    assert_eq!(std::fs::read_to_string(&blocker).unwrap(), "file");
    assert_eq!(branches(&hooked), ["main"]);
    assert_eq!(worktrees(&hooked), sorted(&[&hooked]));
    assert!(
        !fx.root.join("made").exists(),
        "directories created for the destination are removed"
    );
    assert!(!fx.root.join("fine").exists());
}

#[test]
fn cleanup_failure_is_reported_with_survivors_and_keeps_the_result() {
    let fx = Fixture::new(MANIFEST);
    let repo = fx.repo("one");
    let checkout = fx.root.join("broken-wt");
    let spec = json!({"tasks": [{
        "agent": "alpha", "prompt": "BREAK_GIT", "cwd": repo,
        "worktree": {"base": "main", "ephemeral": true, "path": checkout}
    }]});
    let output = fx.run(&spec);

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let runs = runs_of(&output);
    let run = &runs[0];
    assert_eq!(
        run["outcome"], "completed",
        "execution outcome is independent"
    );
    assert_eq!(run["response"], "broke the checkout");
    let errors = run["cleanup_errors"].as_array().unwrap();
    let text = errors
        .iter()
        .map(|error| error.as_str().unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    let branch = manifest_of(run)["worktree"]["branch"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(text.contains(checkout.to_str().unwrap()), "{text}");
    assert!(text.contains(&branch), "{text}");
    assert!(checkout.exists());
    assert!(branches(&repo).contains(&branch));

    // Human output reports it too rather than implying success.
    std::fs::write(checkout.join(".git"), "gitdir: /nonexistent\n").unwrap();
    let checkout2 = fx.root.join("broken-wt-2");
    let mut spec = spec;
    spec["tasks"][0]["worktree"]["path"] = json!(checkout2);
    let output = fx.cmd().arg("run").arg(spec.to_string()).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cleanup error"), "{stdout}");
    assert!(stdout.contains(checkout2.to_str().unwrap()), "{stdout}");
}

#[test]
fn a_timed_out_run_still_removes_its_ephemeral_worktree() {
    let fx = Fixture::new(MANIFEST);
    let repo = fx.repo("one");
    let checkout = fx.root.join("slow-wt");
    let output = fx
        .cmd()
        .args(["run", "--json", "--timeout", "1"])
        .arg(
            json!({"tasks": [{
                "agent": "alpha", "prompt": "SLEEP=30", "cwd": repo,
                "worktree": {"base": "main", "ephemeral": true, "path": checkout}
            }]})
            .to_string(),
        )
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(runs_of(&output)[0]["outcome"], "timeout");
    assert!(!checkout.exists());
    assert_eq!(branches(&repo), ["main"]);
}

/// Wait until `ready` holds, panicking after a few seconds.
fn wait_for(what: &str, ready: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn an_interrupt_removes_ephemeral_work_and_retains_persistent_work() {
    let fx = Fixture::new(r#"{"worktree_root": "../trees", "agents": {"alpha": {}}}"#);
    let repo = fx.repo("one");
    let ephemeral = fx.root.join("eph-wt");
    let child = fx
        .cmd()
        .args(["run", "--json"])
        .arg(
            json!({"defaults": {"cwd": repo, "prompt": "SLEEP=30"}, "tasks": [
                {"agent": "alpha",
                 "worktree": {"base": "main", "ephemeral": true, "path": ephemeral}},
                {"agent": "alpha", "worktree": {"base": "main", "ephemeral": false}}
            ]})
            .to_string(),
        )
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    wait_for("both harnesses", || {
        fx.sandbox.recorded_sessions().len() == 2
    });
    unsafe { libc::kill(child.id() as i32, libc::SIGINT) };
    let output = child.wait_with_output().unwrap();

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let runs = runs_of(&output);
    assert_eq!(runs[0]["outcome"], "aborted");
    assert_eq!(runs[1]["outcome"], "aborted");
    assert!(!ephemeral.exists(), "ephemeral work is disposed of");
    let retained = PathBuf::from(runs[1]["worktree"]["path"].as_str().unwrap());
    let branch = runs[1]["worktree"]["branch"].as_str().unwrap();
    assert!(retained.exists(), "persistent work survives an interrupt");
    let mut expected = vec![branch.to_string(), "main".to_string()];
    expected.sort();
    assert_eq!(branches(&repo), expected);
}

#[test]
fn an_interrupt_during_preparation_launches_nothing_and_removes_what_it_created() {
    let fx = Fixture::new(MANIFEST);
    let repo = fx.repo("one");
    let hooks = fx.root.join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let marker = fx.root.join("hook-started");
    write_executable(
        &hooks.join("post-checkout"),
        &format!(
            "#!/usr/bin/env bash\n: >{}\nsleep 1\nexit 0\n",
            marker.display()
        ),
    );
    git(
        &repo,
        &["config", "core.hooksPath", hooks.to_str().unwrap()],
    );
    let persistent = fx.root.join("persistent-wt");
    let ephemeral = fx.root.join("ephemeral-wt");

    use std::os::unix::process::CommandExt;
    let child = fx
        .cmd()
        .args(["run", "--json"])
        .arg(
            json!({"defaults": {"cwd": repo}, "tasks": [
                {"agent": "alpha", "prompt": "a",
                 "worktree": {"base": "main", "ephemeral": false, "path": persistent}},
                {"agent": "alpha", "prompt": "b",
                 "worktree": {"base": "main", "ephemeral": true, "path": ephemeral}}
            ]})
            .to_string(),
        )
        .stdout(Stdio::piped())
        // Its own group, so the interrupt below reaches everything in it, as
        // a terminal's Ctrl-C reaches the whole foreground job.
        .process_group(0)
        .spawn()
        .unwrap();
    wait_for("the checkout hook", || marker.exists());
    unsafe { libc::kill(-(child.id() as i32), libc::SIGINT) };
    let output = child.wait_with_output().unwrap();

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let runs = runs_of(&output);
    for run in &runs {
        assert_eq!(run["outcome"], "aborted", "{run}");
        assert!(run.get("worktree").is_none(), "{run}");
        assert_eq!(run["cleanup_errors"], json!([]), "{run}");
    }
    assert!(
        fx.sandbox.recorded_sessions().is_empty(),
        "no harness launched"
    );
    assert!(!persistent.exists() && !ephemeral.exists());
    assert_eq!(branches(&repo), ["main"]);
    assert_eq!(worktrees(&repo), sorted(&[&repo]));
}

/// Install `hooks/<name>` with `body` and point the repository at it.
fn install_hook(fx: &Fixture, repo: &Path, name: &str, body: &str) {
    let hooks = fx.root.join(format!(
        "hooks-{}",
        repo.file_name().unwrap().to_string_lossy()
    ));
    std::fs::create_dir_all(&hooks).unwrap();
    write_executable(&hooks.join(name), body);
    git(repo, &["config", "core.hooksPath", hooks.to_str().unwrap()]);
}

#[test]
fn a_registered_destination_is_never_claimed_or_removed() {
    let fx = Fixture::new(MANIFEST);
    let repo = fx.repo("one");
    // A locked worktree whose directory is gone (unmounted media, say) is
    // still registered: its path is not free.
    let locked = fx.root.join("locked-wt");
    git(&repo, &["branch", "other"]);
    git(
        &repo,
        &["worktree", "add", locked.to_str().unwrap(), "other"],
    );
    git(&repo, &["worktree", "lock", locked.to_str().unwrap()]);
    std::fs::remove_dir_all(&locked).unwrap();
    // A destination that would contain a registered worktree overlaps it.
    let outer = fx.root.join("outer");
    let inner = outer.join("inner");
    git(
        &repo,
        &["worktree", "add", "--detach", inner.to_str().unwrap()],
    );
    std::fs::remove_dir_all(&outer).unwrap();

    let output = fx.run(&json!({"defaults": {"cwd": repo}, "tasks": [
        {"agent": "alpha", "prompt": "a",
         "worktree": {"base": "main", "ephemeral": true, "path": locked}},
        {"agent": "alpha", "prompt": "b",
         "worktree": {"base": "main", "ephemeral": true, "path": outer}}
    ]}));

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let runs = runs_of(&output);
    for run in &runs {
        assert_eq!(run["outcome"], "failed", "{run}");
        assert!(error_of(run).contains("registered"), "{run}");
        assert_eq!(run["cleanup_errors"], json!([]), "{run}");
    }
    assert!(fx.sandbox.recorded_sessions().is_empty());
    assert_eq!(worktrees(&repo), sorted(&[&repo, &locked, &inner]));
    assert!(
        git(&repo, &["worktree", "list", "--porcelain"]).contains("locked"),
        "the lock survives"
    );
    assert_eq!(branches(&repo), ["main", "other"]);
    assert!(!locked.exists() && !outer.exists());
}

#[test]
fn a_registration_appearing_during_preparation_is_not_claimed() {
    let fx = Fixture::new(MANIFEST);
    let repo = fx.repo("one");
    git(&repo, &["branch", "other"]);
    let dest = fx.root.join("raced-wt");
    let mark = fx.root.join("raced-mark");
    // While cue-agent creates its branch, someone else registers a checkout
    // of another branch at the destination, locks it and removes its
    // directory: `git worktree add` then fails on a registration that is
    // not this task's.
    install_hook(
        &fx,
        &repo,
        "reference-transaction",
        &format!(
            "#!/usr/bin/env bash\ncat >/dev/null\n[ -e {mark} ] && exit 0\n: >{mark}\n\
             unset $(git rev-parse --local-env-vars)\n\
             git -C {repo} worktree add -q {dest} other && \
             git -C {repo} worktree lock {dest} && rm -rf {dest}\nexit 0\n",
            mark = mark.display(),
            repo = repo.display(),
            dest = dest.display()
        ),
    );

    let output = fx.run(&json!({"tasks": [
        {"agent": "alpha", "prompt": "a", "cwd": repo,
         "worktree": {"base": "main", "ephemeral": true, "path": dest}}
    ]}));

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let run = &runs_of(&output)[0];
    assert_eq!(run["outcome"], "failed", "{run}");
    assert_eq!(run["cleanup_errors"], json!([]), "{run}");
    assert!(mark.exists(), "the hook ran");
    assert_eq!(
        worktrees(&repo),
        sorted(&[&repo, &dest]),
        "registration kept"
    );
    assert_eq!(branches(&repo), ["main", "other"]);
    assert!(
        !dest.exists(),
        "only the empty directory this task made is gone"
    );
}

#[test]
fn a_destination_created_during_preparation_is_not_adopted() {
    let fx = Fixture::new(MANIFEST);
    let repo = fx.repo("one");
    let dest = fx.root.join("appeared");
    let mark = fx.root.join("appeared-mark");
    // The directory appears after the existence check, while the branch is
    // being created. `git worktree add` would accept it because it is empty.
    install_hook(
        &fx,
        &repo,
        "reference-transaction",
        &format!(
            "#!/usr/bin/env bash\ncat >/dev/null\n[ -e {mark} ] && exit 0\n: >{mark}\n\
             mkdir {dest}\nexit 0\n",
            mark = mark.display(),
            dest = dest.display()
        ),
    );

    let output = fx.run(&json!({"tasks": [
        {"agent": "alpha", "prompt": "a", "cwd": repo,
         "worktree": {"base": "main", "ephemeral": true, "path": dest}}
    ]}));

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let run = &runs_of(&output)[0];
    assert_eq!(run["outcome"], "failed", "{run}");
    assert!(error_of(run).contains("already exists"), "{run}");
    assert_eq!(run["cleanup_errors"], json!([]), "{run}");
    assert!(fx.sandbox.recorded_sessions().is_empty());
    assert!(dest.is_dir(), "the other actor's directory survives");
    assert_eq!(branches(&repo), ["main"]);
    assert_eq!(worktrees(&repo), sorted(&[&repo]));
}

#[test]
fn nested_destinations_in_one_batch_are_rejected_including_aliases() {
    let fx = Fixture::new(MANIFEST);
    let one = fx.repo("one");
    let two = fx.repo("two");
    let outer = fx.root.join("outer-wt");
    let alias = fx.root.join("alias");
    std::os::unix::fs::symlink(&fx.root, &alias).unwrap();
    let fine = fx.root.join("fine-wt");
    let output = fx.run(&json!({"tasks": [
        {"agent": "alpha", "prompt": "a", "cwd": one,
         "worktree": {"base": "main", "ephemeral": true, "path": outer}},
        {"agent": "alpha", "prompt": "DIRTY", "cwd": one,
         "worktree": {"base": "main", "ephemeral": false, "path": outer.join("nested")}},
        {"agent": "alpha", "prompt": "DIRTY", "cwd": two,
         "worktree": {"base": "main", "ephemeral": false,
                      "path": alias.join("outer-wt").join("inner")}},
        {"agent": "beta", "prompt": "b", "cwd": one,
         "worktree": {"base": "main", "ephemeral": false, "path": fine, "branch": "fine"}}
    ]}));

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let runs = runs_of(&output);
    assert_eq!(runs[0]["outcome"], "completed", "{}", runs[0]);
    for run in &runs[1..3] {
        assert_eq!(run["outcome"], "failed", "{run}");
        assert!(error_of(run).contains("overlaps"), "{run}");
        assert_eq!(run["cleanup_errors"], json!([]), "{run}");
        assert!(run.get("worktree").is_none(), "{run}");
    }
    assert_eq!(runs[3]["outcome"], "completed", "{}", runs[3]);
    assert!(!outer.exists());
    assert_eq!(branches(&one), ["fine", "main"]);
    assert_eq!(branches(&two), ["main"]);
    assert_eq!(worktrees(&one), sorted(&[&one, &fine]));
    assert_eq!(worktrees(&two), sorted(&[&two]));
}

#[test]
fn inherited_git_repository_variables_do_not_reroute_worktree_commands() {
    let fx = Fixture::new(MANIFEST);
    let repo = fx.repo("one");
    let decoy = fx.repo("decoy");
    let kept = fx.root.join("kept-wt");
    let gone = fx.root.join("gone-wt");
    let decoy_git = decoy.join(".git");
    let output = fx
        .cmd()
        .env("GIT_DIR", &decoy_git)
        .env("GIT_WORK_TREE", &decoy)
        .env("GIT_INDEX_FILE", decoy_git.join("index"))
        .env("GIT_COMMON_DIR", &decoy_git)
        .env("GIT_OBJECT_DIRECTORY", decoy_git.join("objects"))
        .args(["run", "--json"])
        .arg(
            json!({"defaults": {"cwd": repo}, "tasks": [
                {"agent": "alpha", "prompt": "a",
                 "worktree": {"base": "main", "ephemeral": false, "path": kept,
                              "branch": "kept"}},
                {"agent": "alpha", "prompt": "b",
                 "worktree": {"base": "main", "ephemeral": true, "path": gone}}
            ]})
            .to_string(),
        )
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let runs = runs_of(&output);
    for run in &runs {
        assert_eq!(run["outcome"], "completed", "{run}");
        assert_eq!(run["cleanup_errors"], json!([]), "{run}");
    }
    assert_eq!(branches(&repo), ["kept", "main"]);
    assert_eq!(worktrees(&repo), sorted(&[&repo, &kept]));
    assert_eq!(branches(&decoy), ["main"], "the decoy is untouched");
    assert_eq!(worktrees(&decoy), sorted(&[&decoy]));
    assert_eq!(git(&decoy, &["status", "--porcelain"]), "");
    assert!(!gone.exists());
    assert_eq!(
        std::fs::read_to_string(kept.join("README.md")).unwrap(),
        "readme of one\n"
    );
}

#[test]
fn an_unregistered_partial_checkout_is_removed_and_an_unclaimed_one_reported() {
    let fx = Fixture::new(MANIFEST);
    // The checkout is populated, then its registration disappears before
    // `git worktree add` fails: the directory and contents are the task's.
    let unregistered = fx.repo("unregistered");
    install_hook(
        &fx,
        &unregistered,
        "post-checkout",
        "#!/usr/bin/env bash\nrm -rf \"$(git rev-parse --absolute-git-dir)\"\n\
         printf 'partial\\n' >partial.txt\nexit 3\n",
    );
    // The checkout stays registered, but no longer on the task's branch, so
    // the task cannot prove the registration is its own: it is reported.
    let detached = fx.repo("detached");
    install_hook(
        &fx,
        &detached,
        "post-checkout",
        "#!/usr/bin/env bash\ngit update-ref --no-deref HEAD HEAD\nexit 3\n",
    );
    let partial = fx.root.join("partial-wt");
    let unclaimed = fx.root.join("unclaimed-wt");

    let output = fx.run(&json!({"tasks": [
        {"agent": "alpha", "prompt": "a", "cwd": unregistered,
         "worktree": {"base": "main", "ephemeral": false, "path": partial}},
        {"agent": "alpha", "prompt": "b", "cwd": detached,
         "worktree": {"base": "main", "ephemeral": false, "path": unclaimed}}
    ]}));

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let runs = runs_of(&output);
    assert_eq!(runs[0]["outcome"], "failed", "{}", runs[0]);
    assert_eq!(runs[0]["cleanup_errors"], json!([]), "{}", runs[0]);
    assert!(!partial.exists(), "the owned partial checkout is removed");
    assert_eq!(branches(&unregistered), ["main"]);
    assert_eq!(worktrees(&unregistered), sorted(&[&unregistered]));

    assert_eq!(runs[1]["outcome"], "failed", "{}", runs[1]);
    let errors = runs[1]["cleanup_errors"].as_array().unwrap();
    assert!(
        errors.iter().any(|error| error
            .as_str()
            .unwrap()
            .contains(unclaimed.to_str().unwrap())),
        "{}",
        runs[1]
    );
    assert!(
        unclaimed.exists(),
        "an unproven registration is not deleted"
    );
    assert_eq!(worktrees(&detached), sorted(&[&detached, &unclaimed]));
}

#[test]
fn a_branch_committed_before_an_interrupted_creation_was_stopped_is_removed() {
    let fx = Fixture::new(MANIFEST);
    let repo = fx.repo("one");
    let mark = fx.root.join("committed-mark");
    // Git commits the new ref, then waits for this hook, whose status it
    // ignores: stopping git here leaves the branch created.
    install_hook(
        &fx,
        &repo,
        "reference-transaction",
        &format!(
            "#!/usr/bin/env bash\ncat >/dev/null\n[ \"$1\" = committed ] || exit 0\n\
             : >{mark}\nsleep 30\n",
            mark = mark.display()
        ),
    );
    let dest = fx.root.join("hung-wt");
    let started = Instant::now();
    let child = fx
        .cmd()
        .env("CUE_AGENT_GRACE_MS", "300")
        .args(["run", "--json"])
        .arg(
            json!({"tasks": [{"agent": "alpha", "prompt": "a", "cwd": repo,
                "worktree": {"base": "main", "ephemeral": true, "path": dest,
                             "branch": "named"}}]})
            .to_string(),
        )
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    wait_for("the committed hook", || mark.exists());
    unsafe { libc::kill(child.id() as i32, libc::SIGINT) };
    let output = child.wait_with_output().unwrap();

    assert!(
        started.elapsed() < Duration::from_secs(20),
        "neither creation nor cleanup waits for the hook"
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let run = &runs_of(&output)[0];
    assert_eq!(run["outcome"], "failed", "{run}");
    assert!(error_of(run).contains("named"), "{run}");
    assert_eq!(run["cleanup_errors"], json!([]), "{run}");
    assert!(fx.sandbox.recorded_sessions().is_empty());
    assert_eq!(branches(&repo), ["main"], "the committed branch is removed");
    assert_eq!(worktrees(&repo), sorted(&[&repo]));
    assert!(!dest.exists());
}

#[test]
fn a_ref_lock_left_by_a_stopped_branch_creation_is_reported_and_kept() {
    let fx = Fixture::new(MANIFEST);
    let repo = fx.repo("one");
    let mark = fx.root.join("prepared-mark");
    // Creating the task's nested branch hangs while git holds the ref lock:
    // stopping git leaves the lock behind, with no ref written.
    install_hook(
        &fx,
        &repo,
        "reference-transaction",
        &format!(
            "#!/usr/bin/env bash\ninput=$(cat)\n[ \"$1\" = prepared ] || exit 0\n\
             case \"$input\" in \"0000000000000000000000000000000000000000 \"*\" refs/heads/nested/locked\")\n\
             : >{mark}; sleep 30;;\nesac\nexit 0\n",
            mark = mark.display()
        ),
    );
    let dest = fx.root.join("locked-wt");
    let started = Instant::now();
    let child = fx
        .cmd()
        .env("CUE_AGENT_GRACE_MS", "300")
        .args(["run", "--json"])
        .arg(
            json!({"tasks": [{"agent": "alpha", "prompt": "a", "cwd": repo,
                "worktree": {"base": "main", "ephemeral": true, "path": dest,
                             "branch": "nested/locked"}}]})
            .to_string(),
        )
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    wait_for("the prepared hook", || mark.exists());
    unsafe { libc::kill(child.id() as i32, libc::SIGINT) };
    let output = child.wait_with_output().unwrap();

    assert!(
        started.elapsed() < Duration::from_secs(20),
        "creation does not wait for the hook"
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let run = &runs_of(&output)[0];
    assert_eq!(run["outcome"], "failed", "{run}");
    let lock = repo
        .join(".git")
        .join("refs")
        .join("heads")
        .join("nested")
        .join("locked.lock");
    let errors = run["cleanup_errors"].as_array().unwrap();
    assert!(
        errors.iter().any(|error| error
            .as_str()
            .unwrap()
            .contains(&format!("lock file {} ", lock.display()))),
        "the lock is reported by its exact path: {run}"
    );
    assert!(lock.exists(), "a lock of uncertain ownership is kept");
    assert!(fx.sandbox.recorded_sessions().is_empty());
    assert_eq!(branches(&repo), ["main"]);
    assert_eq!(worktrees(&repo), sorted(&[&repo]));
    assert!(!dest.exists());
}

#[test]
fn a_hanging_hook_cannot_hold_cleanup_and_the_surviving_branch_is_reported() {
    let fx = Fixture::new(MANIFEST);
    let repo = fx.repo("one");
    let mark = fx.root.join("delete-mark");
    // Deleting the task's branch hangs before the ref is removed.
    install_hook(
        &fx,
        &repo,
        "reference-transaction",
        &format!(
            "#!/usr/bin/env bash\ninput=$(cat)\n[ \"$1\" = prepared ] || exit 0\n\
             case \"$input\" in *\"0000000000000000000000000000000000000000 refs/heads/held\"*)\n\
             : >{mark}; sleep 30;;\nesac\nexit 0\n",
            mark = mark.display()
        ),
    );
    let dest = fx.root.join("held-wt");
    let started = Instant::now();
    let child = fx
        .cmd()
        .env("CUE_AGENT_GRACE_MS", "300")
        .args(["run", "--json"])
        .arg(
            json!({"tasks": [{"agent": "alpha", "prompt": "SLEEP=30", "cwd": repo,
                "worktree": {"base": "main", "ephemeral": true, "path": dest,
                             "branch": "held"}}]})
            .to_string(),
        )
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    wait_for("the harness", || fx.sandbox.recorded_sessions().len() == 1);
    unsafe { libc::kill(child.id() as i32, libc::SIGINT) };
    let output = child.wait_with_output().unwrap();

    assert!(
        started.elapsed() < Duration::from_secs(20),
        "cleanup is bounded after an interrupt"
    );
    assert!(mark.exists(), "the deletion hook ran");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let run = &runs_of(&output)[0];
    assert_eq!(run["outcome"], "aborted", "the result is kept: {run}");
    let errors = run["cleanup_errors"].as_array().unwrap();
    assert!(
        errors
            .iter()
            .any(|error| error.as_str().unwrap().contains("'held'")),
        "the surviving branch is reported: {run}"
    );
    assert!(!dest.exists(), "the checkout itself is removed");
    assert!(branches(&repo).contains(&"held".to_string()));
}

#[test]
fn the_final_head_is_recorded_in_the_manifest_before_the_checkout_is_removed() {
    let fx = Fixture::new(MANIFEST);
    let repo = fx.repo("one");
    let dest = fx.root.join("commit-wt");
    let output = fx.run(&json!({"tasks": [{
        "agent": "alpha", "prompt": "COMMIT", "cwd": repo,
        "worktree": {"base": "main", "ephemeral": true, "path": dest}
    }]}));

    assert!(output.status.success(), "{output:?}");
    let run = &runs_of(&output)[0];
    let head = run["response"]
        .as_str()
        .unwrap()
        .strip_prefix("committed ")
        .unwrap()
        .to_string();
    let recorded = manifest_of(run);
    assert_eq!(recorded["worktree"]["head"], json!(head), "{recorded}");
    assert_ne!(recorded["worktree"]["base_commit"], json!(head));
    assert!(!dest.exists());
    assert_eq!(branches(&repo), ["main"]);
}
