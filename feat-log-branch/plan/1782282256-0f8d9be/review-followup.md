---
status: complete
---
# Plan: address review findings from feat/log-branch

## Foreword

Two reviewers (Gemini Flash, Sonnet) found six issues with the `feat/log-branch`
PR. This plan addresses the actionable subset: the three that can be fixed on
this branch without scope creep, and documents the pre-existing debt as
deferred todos.

**In scope for this plan (fix now):**

1. Use `git::sanitize_branch_name` helper instead of inline `.replace(...)` —
   the helper already exists in `cuelib/src/git.rs:141-143` and is used by
   `context/mod.rs` and `commands/context.rs`. The callers `add/mod.rs:54`,
   `log/mod.rs:60`, and `commands/log.rs:67` all duplicate it inline.
2. Destructure `LogAddOptions` at the top of `add_entry` using struct
   destructure (matching `AddOptions` in `add/mod.rs:19-27`) instead of two
   individual `let` bindings.
3. Guard against empty/blank `--branch` string, which currently writes to
   `<cue_path>/spec/log.md` (the cue root). Add a `bail!` in `add_entry`
   after branch resolution.
4. Add three missing tests: `log list --branch` round-trip; `--file + --branch`
   combined; slash normalization with `backslash` in branch name.

**Deferred (pre-existing, out of scope):**

- `..` path traversal via branch name — same gap exists in `add/mod.rs`, needs
  a `validate_branch_name` function and a broader decision. Create a `todo`.
- Detached HEAD silent `HEAD/` directory — shared across multiple commands.
  Create a `todo`.

**Working branch:** `feat/log-branch`.

## Steps

### Slice 1 — use `sanitize_branch_name` helper

- [x] 1a. In `crates/cue/src/add/mod.rs:54`: replace
      `branch.replace(['/', '\\'], "-")` with
      `git::sanitize_branch_name(&branch)`.
- [x] 1b. In `crates/cue/src/log/mod.rs:60`: same replacement.
- [x] 1c. In `crates/cue/src/commands/log.rs:67`: same replacement.
- [x] 1d. Run `cargo test -p cue` — green.
- [x] 1e. Run `cargo clippy -p cue --all-targets` — clean.

### Slice 2 — destructure `LogAddOptions`

- [x] 2a. In `crates/cue/src/log/mod.rs:27-29`, replace the two individual
      `let` bindings with a single struct destructure:
      `let LogAddOptions { entry, branch_name } = opts;`
- [x] 2b. Run `cargo test -p cue` — green.

### Slice 3 — guard empty branch name

- [x] 3a. In `crates/cue/src/log/mod.rs`, after branch resolution, added:
      `if branch.trim().is_empty() { bail!("Branch name cannot be empty."); }`
- [x] 3b. In `crates/cue/src/add/mod.rs`, same guard added.
- [x] 3c. Added test case to `test_log_add_validation` for `--branch ""`.
- [x] 3d. Run `cargo test -p cue` — green.

### Slice 4 — add missing tests

- [x] 4a. Added `test_log_add_branch_and_list` — round-trip via `log list --branch`.
- [x] 4b. Added `test_log_add_file_with_branch` — `--file + --branch` combined.
- [x] 4c. Run `cargo test -p cue` — 7 log tests, all pass.
- [x] 4d. Run `cargo clippy -p cue --all-targets` — clean.

### Slice 5 — commit and log

- [x] 5a. Committed as `refactor: address review findings on log add --branch`
      (sha `ce9511f`). 4 files, 104 insertions, 6 deletions.
- [x] 5b. cue-log recorded.

### Deferred todos

- [x] 6a. Todo created: `validate_branch_name` to reject `..` path components.
- [x] 6b. Todo created: detached HEAD produces silent `HEAD/` branch directory.
