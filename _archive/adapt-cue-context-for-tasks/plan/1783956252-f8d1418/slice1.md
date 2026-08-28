---
status: complete
refs:
- .cue/master/task/adapt-cue-context-for-tasks.md
- .cue/adapt-cue-context-for-tasks/plan/index.md
---
# Executive Plan: Fix gather_context scope resolution

## Foreword

This plan covers the single-phase fix for the `adapt-cue-context-for-tasks`
task. The master plan is at `.cue/adapt-cue-context-for-tasks/plan/index.md`.

The change is confined to `crates/cue/src/context/mod.rs`. The goal is to
replace the `get_current_branch()` call in `gather_context` with
`resolve_scope(&cue_dir)` so that `cue context render` honours `.cue/HEAD`.

Prerequisite: be on branch `feat/adapt-context-for-tasks` with the
`adapt-cue-context-for-tasks` task active in `.cue/HEAD`.

## Steps

- [x] 1. Add a failing integration test: render with HEAD pointing to a task
         slug should load that slug's `context.json`, not the git branch's.
- [x] 2. Add a failing integration test: render with HEAD absent (or `master`)
         should fall back to the `master` context.
- [x] 3. In `gather_context` (`context/mod.rs:168-235`), replace
         `get_current_branch(cwd)?` + `sanitize_branch_name(&branch)` with
         `cuelib::head::resolve_scope(&git_root.join(&config.dir_name))?`.
- [x] 4. Remove the now-unused `get_current_branch` / `sanitize_branch_name`
         imports from `context/mod.rs` if no longer referenced.
- [x] 5. Run `cargo test -p cue --test-threads=1` — all tests must pass.
- [x] 6. Run `cargo clippy -p cue` — no new warnings.
- [x] 7. Commit with a conventional commit message.
- [x] 8. Fill Evidence cells in the task card and mark task `complete`.
