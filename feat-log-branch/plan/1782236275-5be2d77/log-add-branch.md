---
status: complete
---
# Plan: add `--branch` to `cue log add`

## Foreword

This executive plan implements task
`.cue/master/task/1782236275-5be2d77/log-with-branch.md`: add the `--branch`
flag to `cue log add` so an entry can be written to an arbitrary branch (notably
`master`) rather than only the current git branch.

**Prerequisite context:** research trace
`.cue/master/trace/1782236275-5be2d77/log-with-branch-research.md`. It
established that `cue log list` already supports `--branch`
(`crates/cue/src/cli.rs:172-176`) and that only `cue log add` lacks it. The
pattern to copy is `add --branch`: CLI field `crates/cue/src/cli.rs:53-55`,
resolution `crates/cue/src/add/mod.rs:47-57`.

**Scope:** `cue log add` only. Three source edits + one integration test.
Approach is TDD with vertical slices (one test -> minimal impl -> next).

**Working branch:** `feat/log-branch` (base `master`).

## Steps

- [x] 1. RED — add integration test `test_log_add_with_explicit_branch` to
      `crates/cue/tests/log.rs`, mirroring `test_add_with_explicit_branch`
      (`crates/cue/tests/add.rs:666-692`). It runs
      `log add --title "..." --branch feature/other` and asserts (a) the entry
      lands at `.test-mem/feature-other/spec/log.md`, and (b) the file's
      **content** contains the title and the `# Project Log` header (a path that
      exists but is empty/wrong would otherwise pass). Confirm it fails (unknown
      flag).
- [x] 2. GREEN — add a `branch` field to `LogCommands::Add` in
      `crates/cue/src/cli.rs:167-170` using `#[arg(short = 'b', long)]` (matches
      the write command `add`; independent of `--file`, no conflict needed).
- [x] 3. GREEN — destructure the new `branch` field in the `log add` handler
      (`crates/cue/src/commands/log.rs:27`) and pass it into `LogAddOptions`.
- [x] 4. GREEN — add `branch_name: Option<String>` to `LogAddOptions`
      (`crates/cue/src/log/mod.rs:24`); in `add_entry`
      (`crates/cue/src/log/mod.rs:52-58`) resolve the branch via `if let
      Some(b) = branch_name` pattern (per Opus consultation), then sanitize
      as before.
- [x] 5. Confirm the new test passes and the existing `test_log_add_basic`
      (`crates/cue/tests/log.rs:7-36`, the no-flag regression case) still passes.
- [x] 6. Verify the full suite: `cargo test -p cue` — all 5 log tests pass.
- [x] 7. Verify lint: `cargo clippy -p cue --all-targets` — clean (1 pre-existing
      unrelated warning in dir_flag.rs, not introduced by this change).
- [x] 8. Commit at GREEN (`feat: add --branch flag to cue log add`, sha `0f8d9be`).
      Then `cue-log` the milestone.
- [ ] 9. Fill the task's Acceptance Criteria Evidence cells and set
      `status: complete`.
