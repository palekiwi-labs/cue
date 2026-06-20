---
status: complete
---
# Executive Plan: TDD Implementation Slice

## Foreword

This slice covers Phase 1 of the master plan on branch `fix/title-yaml-quoting`.
The goal is to write a failing integration test that reproduces the colon-in-title
bug, then apply the one-function fix to `build_frontmatter_bytes`, and verify that
all existing and new tests pass. The master plan (`plan/index.md`) has the full
architectural rationale.

Prerequisites: branch `fix/title-yaml-quoting` checked out, codebase already
explored and understood.

## Steps

- [x] Create branch `fix/title-yaml-quoting`
- [x] Update task status to `in-progress` on master
- [x] Save master plan and this executive plan
- [x] Write failing test `test_add_frontmatter_colon_in_string_value` in
      `crates/cue/tests/add.rs` — verifies that a title containing `": "`
      is written as a quoted YAML string and round-trips as a string, not a mapping
- [x] Run `cargo test` to confirm RED (new test fails, existing tests pass)
- [x] Apply fix to `build_frontmatter_bytes` in `crates/cue/src/add/mod.rs`
- [x] Run `cargo test` to confirm GREEN (all tests pass)
- [x] Run `cargo clippy` and `cargo fmt --check`
- [x] Commit at GREEN
- [x] Spawn diff-reviewer sub-agent for code review
- [x] Fill AC evidence in task file, mark task `complete`
