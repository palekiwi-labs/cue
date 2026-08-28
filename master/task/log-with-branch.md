---
priority: high
status: complete
title: log with branch
branch: ""
---

# log with branch

Add the `--branch` flag to `cue log add` so a log entry can be written to an
arbitrary branch (notably `master`) instead of only the current git branch.

## Scope correction (from research)

The original framing ("the `cue log` command does not support the `--branch`
flag") is partially incorrect. `cue log` is split into `Add` and `List`
subcommands:

- `cue log list` **already** supports `--branch`
  (`crates/cue/src/cli.rs:172-176`, handler `crates/cue/src/commands/log.rs:51-59`).
- `cue log add` does **not** support `--branch`
  (`crates/cue/src/cli.rs:151-170`) and hardcodes the current git branch in
  storage (`crates/cue/src/log/mod.rs:52-56`).

This task targets **`cue log add` only**. The pattern to follow is
`add --branch` (`crates/cue/src/cli.rs:53-55`, `crates/cue/src/add/mod.rs:47-57`).
Writing to the master log needs no special-casing: `master` is just a directory
name under `.cue/`.

## Source

- Research trace: `.cue/master/trace/1782236275-5be2d77/log-with-branch-research.md`

## Design decisions

- **`--branch` is orthogonal to `--file`.** `branch` is a destination concern;
  `file` is an input-source concern. `branch` is intentionally **not** added to
  `--file`'s `conflicts_with_all` list. `log add --branch X --file entry.json`
  must work, matching top-level `add --branch ... --file ...`.
- **Git hash/dirty context is read from the working-tree HEAD, not the target
  branch.** `log add --branch master` while on `feature/x` stamps the entry with
  the `feature/x` HEAD hash. This is the desired cross-branch-logging semantics
  (record the commit the human was at) and matches `add --branch`. No change to
  `add_entry`'s git-context logic (`crates/cue/src/log/mod.rs:38-41`).

## Acceptance Criteria

| #   | Criterion (outcome)                                                          | Verify by            | Evidence |
| --- | ---------------------------------------------------------------------------- | -------------------- | -------- |
| 1   | `cue log add --branch <name>` writes to `<memdir>/<name>/spec/log.md`        | integration test     | exit 0 — `test_log_add_with_explicit_branch` (log.rs:205), `test_log_add_file_with_branch` |
| 2   | `cue log add` without `--branch` still writes to the current branch          | integration test     | exit 0 — `test_log_add_basic` (log.rs:7, pre-existing regression) |
| 3   | All cue crate tests pass                                                     | `cargo test -p cue`  | exit 0 — full suite green |
| 4   | Clippy is clean                                                             | `cargo clippy -p cue`| exit 0 — 1 pre-existing warning in `tests/dir_flag.rs` (unrelated, not introduced by this branch) |
