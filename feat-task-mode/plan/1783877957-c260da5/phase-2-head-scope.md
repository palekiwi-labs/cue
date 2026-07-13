---
status: open
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

Key files:
- `crates/cuelib/src/` — new `head.rs` module, updated `lib.rs`
- `crates/cue/src/add/mod.rs:51` — write path migration
- `crates/cue/src/log/mod.rs:56` — write path migration
- `crates/cue/src/context/mod.rs:239` — write path migration
- `crates/cue/src/commands/context.rs:28,41,105` — read path migration
- `crates/cue/src/cli.rs` — new subcommands, flag renames
- `crates/cue/src/main.rs` — dispatch for new subcommands
- `crates/cue/src/commands/switch.rs` — new
- `crates/cue/src/commands/status.rs` — new

## Steps

- [ ] **Step 1 — `cuelib/src/head.rs`**: add `read_head(cue_dir: &Path) ->
  Option<String>`, `write_head(cue_dir: &Path, slug: &str) -> Result<()>`, and
  `resolve_scope(cue_dir: &Path) -> Result<String>`. Export from `lib.rs` as
  `pub mod head;`. Add unit tests (HEAD absent → "master"; HEAD contains slug →
  slug; HEAD contains "master" → "master").

- [ ] **Step 2 — migrate write paths**: replace `git::get_current_branch(root)`
  with `cuelib::head::resolve_scope(&cue_path)` (where `cue_path =
  root.join(&config.dir_name)`) in:
  - `crates/cue/src/add/mod.rs:51`
  - `crates/cue/src/log/mod.rs:56`
  - `crates/cue/src/context/mod.rs:239`
  Remove the error context message about "have you made your first commit" since
  scope resolution no longer requires git.

- [ ] **Step 3 — migrate read/render paths**: replace `get_current_branch` calls
  with `resolve_scope` in:
  - `crates/cue/src/commands/context.rs:28` (`handle_show`)
  - `crates/cue/src/commands/context.rs:41` (`handle_profiles`)
  - `crates/cue/src/commands/context.rs:105` (`handle_path`)

- [ ] **Step 4 — rename `--branch` to `--task` on write commands** in
  `crates/cue/src/cli.rs`:
  - `Commands::Add`: rename `branch: Option<String>` to `task: Option<String>`,
    update the help text to "Override active task scope for this invocation".
  - `LogCommands::Add`: same rename.
  Propagate the field rename through `commands/add.rs`, `commands/log.rs`,
  `add/mod.rs` (`AddOptions.branch_name` → `scope_name`), and `log/mod.rs`
  (`LogAddOptions.branch_name` → `scope_name`).
  Leave `List --branch` and `LogCommands::List --branch` unchanged (they are
  read-only filters, not scope overrides).

- [ ] **Step 5 — `cue switch` subcommand**:
  - Add `Switch` variant to `Commands` in `cli.rs`:
    - positional `target: Option<String>` (slug or filepath)
    - `--branch` flag (bool): auto-select from git branch
  - New `crates/cue/src/commands/switch.rs`:
    - `handle(cwd, target, branch_flag)` resolves the slug:
      - if `--branch`: scan `master/task/*.md` for any card whose `branch:`
        YAML list contains the current git branch; switch to that slug or
        print "no matching task" and exit 0.
      - if target is a path ending in `.md`: take the filename stem as slug.
      - otherwise: use the target string directly as slug.
    - Validate: slug must not be empty.
    - `master` is a valid target (returns to global context).
    - Write slug to `.cue/HEAD` via `write_head`.
    - Create `.cue/<slug>/` directory if absent.
    - Print: `switched to task: <slug>` or `switched to global context` for master.
  - Wire up in `main.rs` and `commands/mod.rs`.

- [ ] **Step 6 — `cue status` subcommand**:
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

- [ ] **Step 7 — cargo check + tests**: run `cargo check` and `cargo test`.
  Fix any compilation errors. Confirm all existing tests pass.

- [ ] **Step 8 — commit**: stage non-`.cue/` changes, commit with message
  `feat: replace branch scope with HEAD-driven task scope`.
