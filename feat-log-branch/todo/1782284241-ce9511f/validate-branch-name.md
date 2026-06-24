---
status: open
priority: low
---
# validate_branch_name: reject `..` path components

## Context

`cue add --branch` and `cue log add --branch` both sanitize branch names via
`git::sanitize_branch_name` (replacing `/` and `\` with `-`), but neither
rejects a `..` component. Passing `--branch ..` produces a sanitized
`branch_dir` of `..` and writes into `<git_root>/spec/log.md` (outside the
cue directory).

The same gap exists in `cue log list --branch` (`commands/log.rs:60`).

## Proposed fix

Add a `validate_branch_name(branch: &str) -> Result<()>` function to
`cuelib/src/git.rs` (alongside `sanitize_branch_name`) that rejects:
- `.` and `..`
- any component that Path::new decomposes to ParentDir or RootDir

Call it in `add/mod.rs`, `log/mod.rs`, and `commands/log.rs` after branch
resolution, before sanitization. Mirror the existing `validate_filename`
pattern in `add/mod.rs:127-143`.

## Affected callers (as of feat/log-branch)

- `crates/cue/src/add/mod.rs:51-55`
- `crates/cue/src/log/mod.rs:54-61`
- `crates/cue/src/commands/log.rs:60-67`
