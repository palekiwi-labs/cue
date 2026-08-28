---
status: complete
refs:
- .cue/master/spec/cue/task-mode.md
- .cue/feat-task-mode/plan/index.md
- .cue/master/task/task-workflow-phase-2.md
---
# Phase 2: HEAD-driven context scope

## Foreword

Implements Part 2 of the task-based workflow spec. The goal is to replace
git-branch-derived scope with `.cue/HEAD`-derived scope across all write paths.
This session starts from the `feat-task-mode` branch. Phase 1 (flat task-card
layout) is assumed to be handled separately; this plan focuses entirely on the
`resolve_scope()` abstraction, `read_head`/`write_head`, and the new CLI surface.

Steps 1-8 are complete (commits `90fbbc5`, `94dbce2`, `0ca1947`, `9267c28`).
Manual QA passed. Steps 9-10 remain.

## Steps

- [x] **Step 1 — `cuelib/src/head.rs`**: add `read_head(cue_dir: &Path) ->
  Option<String>`, `write_head(cue_dir: &Path, slug: &str) -> Result<()>`, and
  `resolve_scope(cue_dir: &Path) -> Result<String>`. Export from `lib.rs` as
  `pub mod head;`. Add unit tests (HEAD absent -> "master"; HEAD contains slug ->
  slug; HEAD contains "master" -> "master").
  Done in commit `90fbbc5`.

- [x] **Step 2 — migrate write paths**: replace `git::get_current_branch(root)`
  with `cuelib::head::resolve_scope(&cue_path)` (where `cue_path =
  root.join(&config.dir_name)`) in:
  - `crates/cue/src/add/mod.rs`
  - `crates/cue/src/log/mod.rs`
  - `crates/cue/src/context/mod.rs`
  Removed the error context message about "have you made your first commit"
  since scope resolution no longer requires git.
  Done in commit `94dbce2`.

- [x] **Step 3 — migrate read/render paths**: replace `get_current_branch` calls
  with `resolve_scope` in:
  - `crates/cue/src/commands/context.rs` (`handle_show`, `handle_profiles`,
    `handle_path`)
  - `crates/cue/src/commands/log.rs` (`LogCommands::List`)
  - `crates/cue/src/list/mod.rs`
  Done in commit `94dbce2`.

- [x] **Step 4 — rename `--branch` to `--task` on write commands** in
  `crates/cue/src/cli.rs`:
  - `Commands::Add`: renamed `branch: Option<String>` to `task: Option<String>`,
    help text updated to "Override active task scope for this invocation".
  - `LogCommands::Add`: same rename.
  Propagated field rename through `commands/add.rs`, `commands/log.rs`,
  `add/mod.rs` (`AddOptions.branch_name` -> `scope_name`), and `log/mod.rs`
  (`LogAddOptions.branch_name` -> `scope_name`).
  **Deviation from original plan**: `LogCommands::List --branch` was NOT left
  unchanged. It was removed entirely; `--task` is now the sole scope override
  for reading logs (commit `9267c28`). `cue list --branch` remains for now.
  Done in commit `94dbce2`.

- [x] **Step 5 — `cue switch` subcommand**:
  - Add `Switch` variant to `Commands` in `cli.rs`:
    - positional `target: Option<String>` (slug or filepath)
    - `--branch <name>`: resolve a task whose `branch:` list contains the
      given branch name (requires an argument; no implicit current-branch
      detection).
  - New `crates/cue/src/commands/switch.rs`:
    - `handle(cwd, target, branch)` resolves the slug:
      - if `--branch <name>`: scan `master/task/*.md` for any card whose
        `branch:` YAML list contains `<name>`; switch to that slug or print
        "no task matched branch: <name>" and fall back to master.
      - if target is a path ending in `.md`: take the filename stem as slug.
      - otherwise: use the target string directly as slug.
    - Validate: slug must not be empty.
    - `master` is a valid target (returns to global context).
    - Write slug to `.cue/HEAD` via `write_head`.
    - Create `.cue/<slug>/` directory if absent.
    - Print: `switched to task: <slug>` or `switched to global context`.
  - Wire up in `main.rs` and `commands/mod.rs`.
  **Deviation from original plan**: `--branch` takes a required argument
  instead of being a bool flag that auto-detects the current git branch.
  Done in commits `94dbce2` (initial), `0ca1947` (multiline fix), `9267c28`
  (required-argument refactor).

- [x] **Step 6 — `cue status` subcommand**:
  - Add `Status` variant to `Commands` in `cli.rs` (no arguments).
  - New `crates/cue/src/commands/status.rs`:
    - Read HEAD via `read_head`.
    - If absent or "master": print `active context: master (global)`.
    - Otherwise: attempt to read `master/task/<slug>.md`, parse `title:` and
      `status:` from frontmatter; print:
      ```
      active task: <slug>
        title: <title>
        status: <status>
        context: .cue/<slug>/
      ```
      If the task card is not found, print the slug and context path only.
  - Wire up in `main.rs` and `commands/mod.rs`.
  Done in commit `94dbce2`.

- [x] **Step 7 — cargo check + tests**: all tests pass with
  `--test-threads=1`. (Parallel test runs can hit `os error 11` resource
  exhaustion from git subprocess spawning -- a system limitation, not a code
  issue.)
  Done.

- [x] **Step 8 — commit**: committed as `90fbbc5` (head module) and `94dbce2`
  (scope migration + new subcommands), plus `0ca1947` (QA bugfixes) and
  `9267c28` (--branch argument refactor).

- [x] **Step 8a — Manual QA**: smoke-tested `cue switch`, `cue status`,
  `--task` flag, `--branch` resolution. All working. Two bugs found and fixed
  during QA: multiline branch list parsing (`0ca1947`) and missing `--task`
  on `log list` (later superseded by removing `--branch` entirely in
  `9267c28`). QA notes in
  `.cue/feat-task-mode/todo/1783925542-94dbce2/manual-qa-task-mode.md`.

- [x] **Step 9 — `--json` on `cue status` and `cue switch`**:
  - Add `--json` flag to both `Status` and `Switch` commands in `cli.rs`.
  - `cue status --json` outputs structured JSON:
    ```json
    {"context": "master", "global": true}
    // or
    {"context": "<slug>", "global": false, "title": "...", "status": "..."}
    ```
  - `cue switch --json` outputs structured result after switching:
    ```json
    {"context": "<slug>", "global": false}
    ```
  - Enables programmatic consumption of active context without parsing
    human-readable stdout.
  - `title`/`status` are emitted as `null` (key present) when the task card
    is absent or lacks those fields; omitted entirely in the global case.
    `find_task_for_branch` suppresses its human "no task matched" message
    in JSON mode so stdout remains a single JSON document.
  - Done in commit `2c3c5d8`.

- [x] **Step 10 — SKILL.md documentation**:
  - Document `cue switch`, `cue status`, `--task` flag.
  - Document `.cue/HEAD` semantics and the global/master fallback.
  - Update the cross-command scope-resolution table (`--task` flag vs. HEAD).
  - Done in commit `1deefb5` (external `cue-plugins` repo). Added an "Active
    Context and Scope Resolution" section to SKILL.md with a numbered
    precedence list (style guide: tables avoided in favour of a list), `.cue/HEAD`
    semantics, `cue switch` forms (slug / master / filepath / --branch), `cue
    status` output, and `--json` shapes for both. Updated the Directory
    Structure intro to reference `<scope>` and HEAD.
