---
status: complete
refs:
- .cue/feat-task-mode/tmp/1783930716-2c3c5d8/branch.diff
- .cue/feat-task-mode/plan/1783877957-c260da5/phase-2-head-scope.md
- .cue/master/task/task-workflow-phase-2.md
---
---
foreword: upstream
---

# Review Fixes for feat-task-mode

## Foreword

Addresses the verified code-review findings from the diff-reviewer-opus pass
(consolidated and corrected by a second consultant-opus verification). All
findings were CONFIRMED against source with two severity corrections:

- C1's "poisoned HEAD downstream" angle is largely neutralised by
  `sanitize_branch_name` on read paths; the real, unmitigated vuln is the raw
  `create_dir_all` in `switch.rs` (directory escape) + a malformed HEAD file.
- M1 (`list --branch`) was a documented deferral; the user has now decided to
  lift it and rename the flag to `--task`.

Product decisions (resolved by user):
- **m4**: `cue switch --branch <name>` with no match must BAIL — non-zero exit,
  HEAD unchanged, error to stderr. Motivation: a git post-checkout hook needs
  to detect the no-match case via exit code and decide for itself.
- **M1**: rename `cue list --branch` to `--task` now.

Implementation follows vertical-slice TDD: one test -> one implementation ->
commit. Tests run with `--test-threads=1` (known git-subprocess resource limit).

Reference diff: `.cue/feat-task-mode/tmp/1783930716-2c3c5d8/branch.diff`

## Steps

- [x] **Step 1 — C1: slug validation (Critical)**
  - Add `pub fn validate_slug(slug: &str) -> Result<()>` to
    `crates/cuelib/src/head.rs`. Require exactly one `Component::Normal` and
    nothing else (rejects `..`, `/etc/x`, `a/b`, `.`, empty after trim).
  - Export it from `cuelib` lib.rs alongside `head`.
  - Unit tests in `head.rs`: rejects `..`, `../../foo`, `/etc/x`, `a/b`, `.`;
    accepts `auth-login`, `master`, `feat-x`.
  - Call `head::validate_slug(&slug)?` in `switch.rs` after the empty-string
    check (line 39) and before `write_head` (line 42).
  - Integration guard test in `tests/switch.rs`: `cue switch ../../evil` fails
    (non-zero exit), and `.test-mem/` parent is NOT escaped. Also test an
    absolute path target.

- [x] **Step 2 — M2 (status.rs): replace extract_frontmatter_field**
  - Add `#[derive(serde::Deserialize, Default)] struct StatusFm { title:
    Option<String>, status: Option<String> }`.
  - Replace the manual read+parse block (`status.rs:34-45`) with
    `cuelib::artifact::extract_frontmatter_yaml(&task_card)` +
    `serde_yaml::from_str::<StatusFm>`.
  - Delete `extract_frontmatter_field` (`status.rs:71-86`).
  - Existing `status_json_task_with_card` test covers the happy path; add a
    test asserting indented/CRLF frontmatter still parses (regression guard).

- [x] **Step 3 — M2 + m3 + m4 (switch.rs): serde frontmatter + restructure
  find_task_for_branch + bail on no-match**
  These are intertwined in `switch.rs`; do them as one cohesive change.
  - Add `#[derive(serde::Deserialize)] struct TaskFm { #[serde(default)]
    branch: BranchField }` with an untagged enum `BranchField { One(String),
    Many(Vec<String>), #[default] None }` and a `contains(&self, &str) -> bool`
    method. serde_yaml's untagged enum handles scalar / inline-list / block-list
    uniformly.
  - Rewrite `find_task_for_branch` to signature
    `fn find_task_for_branch(cue_dir: &Path, branch_name: &str) ->
    Result<Option<String>>` (drop the `json` param; return `None` on no match).
    Use `extract_frontmatter_yaml` + `serde_yaml::from_str::<TaskFm>`.
  - Delete `branch_in_markdown` and `extract_fm`.
  - In `handle`: when `find_task_for_branch` returns `None`, bail with
    `"no task matched branch: <name>. HEAD unchanged."` (non-zero exit, no HEAD
    write). This is the m4 decision.
  - Update `switch_json_branch_no_match_emits_single_json` test: invert to
    assert FAILURE (non-zero exit, empty stdout, HEAD not written).
  - Add positive `--branch` tests covering all three `branch:` forms (scalar,
    inline `[a, b]`, multiline block list) — these also lock in the M2 serde
    migration.

- [x] **Step 4 — Test coverage: human output + directory creation**
  - Human-output tests for `switch` ("switched to task: X", "switched to global
    context") and `status` ("active context: master (global)", "active task: X"
    with title/status lines) — currently only `--json` paths are tested.
  - Directory-creation assertion: after `cue switch auth-login`, assert
    `.test-mem/auth-login/` exists.

- [x] **Step 5 — n1: remove redundant git invocation**
  - Drop `git::run_git(["rev-parse", "--git-dir"], cwd)...` in `switch.rs:16`
    and `status.rs:9`. Wrap `get_git_root(cwd)?` with
    `.context("Not in a git repository")`.

- [x] **Step 6 — m2: drop unused `_root` param from resolve_scan_paths**
  - Remove `_root: &Path` from `resolve_scan_paths` signature
    (`list/mod.rs:184-185`) and the `root` argument at the call site
    (`list/mod.rs:149`). Single caller confirmed.

- [x] **Step 7 — M1: rename `cue list --branch` to `--task`**
  - `cli.rs:80-83`: rename `branch` field to `task` with
    `#[arg(long = "task", conflicts_with = "all")]`. Update help text.
  - `main.rs:89`: map `branch_name: task` (internal field name stays
    `branch_name` to limit churn, OR rename to `scope` for clarity — prefer
    `scope` since terminology drift (n4) flags this).
  - Update `tests/list.rs:848,865`: replace `--branch` with `--task`.
  - Note: the JSON output field `"branch"` (list.rs:587,970) is a separate
    concern — leave as-is for this step; track as cosmetic follow-up.

- [x] **Step 8 — n3: standardise `.md` detection in switch.rs**
  - In `resolve_slug_from_target` (`switch.rs:73-84`), use
    `Path::extension().and_then(|e| e.to_str()) == Some("md")` instead of
    `target.ends_with(".md")` for consistency with `find_task_for_branch`.

- [x] **Step 9 — Final validation + log**
  - `cargo fmt`, `cargo clippy`, `cargo test -p cue --test-threads=1`,
    `cargo test -p cuelib`.
  - Confirm no regressions. Log milestone via `cue-log`.

- [x] **Step 10 — n4 full: rename branch/branch_dir locals to scope/scope_dir**
  - `add/mod.rs`: `branch` -> `scope`, `branch_dir` -> `scope_dir`.
  - `log/mod.rs`: same.
  - `list/mod.rs`: destructure binding `scope: branch_name` -> `scope`,
    `resolve_scan_paths` param `branch_name` -> `scope`, locals renamed to match.
  - `commands/log.rs`: `branch_name`/`branch_dir` locals renamed to
    `scope`/`scope_dir`.
  - Commits: n4 add/log (step), n4 list/commands (step).

- [x] **Step 11 — gitignore HEAD on cue init**
  - `init/mod.rs`: append `HEAD\n` to the `.gitignore` content written during
    orphan-branch init (`ensure_worktree`, line 56-61).
  - Test: extend `test_init_fresh_repo` to assert
    `gitignore.contains("HEAD")`, or add a dedicated
    `test_init_gitignore_includes_head` test.
  - Closes todo: `feat-task-mode/todo/1783925542-94dbce2/cue-head-should-be-gitignored.md`.

## Deferred (not in this plan)

- **n4 — terminology drift** (`branch`/`scope`/`slug`/`task` used
  interchangeably in `add/mod.rs`, `log/mod.rs`, `list/mod.rs` locals): a
  pervasive cosmetic rename pass. Tracked as debt; not worth the churn at
  prototype stage beyond the `list --task` rename in Step 7. Partially
  addressed post-plan: `ListOptions::branch_name` renamed to `scope`
  (commit `183d27e`). Remaining drift in `add/mod.rs`, `log/mod.rs` locals
  is still deferred.
- JSON output field name `"branch"` in `cue list --json`: rename to `"scope"`
  or `"task"` for consistency. Low priority; leave tracked.
