---
status: open
refs: ..cue/master/task/fix-cue-list-path.md
---
# Plan: fix cue list path output

## Problem

After PR #39 (worktrees/proxy feature), `cue list` emits store-relative paths
(e.g. `master/task/fix-cue-list-path.md`). These are relative to `.cue/` and
cannot be opened by programs without knowing the store root.

## Decision

Emit **absolute paths** in all `cue list` output (human and JSON). This is the
simplest and most dependable fix. The only downside (host-path exposure) is
acceptable for the current use case.

## Approach

The paths returned by `collect_files` are already absolute (from `WalkDir`).
The fix is to stop stripping the `store_dir` prefix in the output sites and
instead emit the full path directly.

Affected sites:

1. `crates/cue/src/commands/list.rs:32-37` — human output: use `path` directly.
2. `crates/cue/src/list/mod.rs:259-264` — `to_cue_file`: set `rel_path` to the
   absolute path string.
3. `crates/cue/tests/proxy_reads.rs` — update assertions to expect absolute paths.
4. `crates/cue/tests/list.rs` — update any store-relative path assertions.
