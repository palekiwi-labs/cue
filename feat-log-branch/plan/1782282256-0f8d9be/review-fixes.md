---
status: open
---
# Executive Plan - Review Follow-up Fixes

Implement fixes for `feat/log-branch` as requested by reviewers.

## Foreword
This plan addresses four issues identified during review:
1. Duplicated sanitization logic (replace with `git::sanitize_branch_name`).
2. Improper destructuring of `LogAddOptions`.
3. Unguarded empty branch names.
4. Missing test cases for round-trip and combined flags.

## Steps
- [x] Implement changes in `crates/cue/src/add/mod.rs` (sanitize, guard empty branch)
- [x] Implement changes in `crates/cue/src/log/mod.rs` (destructure, sanitize, guard empty branch)
- [x] Implement changes in `crates/cue/src/commands/log.rs` (sanitize)
- [x] Implement changes and add new tests in `crates/cue/tests/log.rs`
- [x] Verify with `cargo test -p cue`
- [x] Verify with `cargo clippy -p cue --all-targets`
