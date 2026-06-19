---
status: in-progress
---
# Implementation Plan: Slices 0-8 (curtain feature branch)

## Foreword

This plan covers the full implementation of the `curtain` feature branch.
The spec is fully settled at `.cue/feat-curtain/spec/index.md`.
Starting baseline: all 115 tests green at commit `173152e`.

The implementation proceeds in TDD order as defined by the spec. Each slice
starts with failing tests and is committed at GREEN. Slice 0 has no new tests
(structural only).

## Steps

- [x] Slice 0: Convert to Cargo workspace
  - [x] Create workspace-level `Cargo.toml` with `[workspace]` pointing to `cue/` and `cuelib/`
  - [x] Move `cue/` source into `cue/cue/` subdirectory (rename the package dir)
  - [x] Create `cue/cuelib/` crate skeleton (lib.rs with pub mod stubs)
  - [x] Verify: `cargo test --workspace` all green
  - [x] Commit: `refactor: convert to Cargo workspace`

- [x] Slice 1: `cuelib` — status constants and canonical artifact types
  - [x] Write failing tests in `cuelib` for status constants and artifact type lists
  - [x] Implement `cuelib::artifact` module with `CANONICAL_TYPES`, `DEFAULT_IGNORED_TYPES`, `TodoStatus` enum
  - [x] Commit: `feat(cuelib): add status constants and canonical artifact types`

- [x] Slice 2: `cuelib` — extract config, git, artifact modules from `cue`
  - [x] Move `config.rs` logic to `cuelib::config`
  - [x] Move `git.rs` logic to `cuelib::git`
  - [x] Re-export from `cue` crate via `cuelib` dependency
  - [x] Verify all existing tests still pass
  - [x] Commit: `refactor(cuelib): extract config and git modules`

- [x] Slice 3: `cuelib` — project registry (`ProjectStore`, `derive_project_key`)
  - [x] Write failing tests: derive_project_key (github/local), load/save store, add/remove paths
  - [x] Implement `cuelib::project` module
  - [x] Commit: `feat(cuelib): add project registry`

- [x] Slice 4: `cue` — `cue project add/remove/list` subcommands
  - [x] Write failing integration tests for CLI subcommands
  - [x] Wire up CLI + handlers in `cue`
  - [x] Commit: `feat(cue): add project subcommands`

- [ ] Slice 4a: `cuelib` — add `task` to canonical types; add `TaskStatus` enum
  - [ ] Add `"task"` to `CANONICAL_TYPES` (9 types); update len assertion in tests
  - [ ] Add `TaskStatus` enum (`Open`, `InProgress`, `Complete`, `Closed`) with
        `FromStr`, `as_str()`, `is_kanban_visible()`
  - [ ] Add tests mirroring existing `TodoStatus` tests
  - [ ] Commit: `feat(cuelib): add task type and TaskStatus enum`

- [ ] Slice 5: `cue` — `cue init` registers project in store
  - [ ] Write failing tests: `init_registers_project_in_store`,
        `init_twice_does_not_duplicate_entry`
  - [ ] Update `commands/init.rs::handle` to call `ProjectStore::add_path` + `save`
  - [ ] Commit: `feat(cue): register project on init`

- [ ] Slices 6–8: `curtain` — DEFERRED
  Moved to a separate curtain design plan. Architecture to be designed based on
  the kanban reference project analysis in `.cue/feat-curtain/doc/`. Key changes
  from original plan: board shows `task` artifacts (not `todo`); discovery scans
  `.cue/master/task/` per registered project; card branch line shows the
  `branch:` frontmatter field (the working feature branch), not the storage
  branch.
