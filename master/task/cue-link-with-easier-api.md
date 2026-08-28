---
priority: normal
status: closed
title: Cue Link With Easier API
refs:
  - .cue/master/task/bug-cue-status-with-no-cue-dir.md
---

`cue link` currently requires an absolute path to the cue store.

Can we make it easier to execute?

Example:

I create a worktree in `./worktrees/my-feature` and I want to link that worktree
to the cue store in the root of the project. The most convenient way to do this
would be: `cue link --dir ./worktrees/my-feature` which means: use the cue store
from current directory and apply it to `./worktrees/my-feature`.

Research the code to see if we can achieve this.

## Resolved Design

Flag is `--at` (not `--dir`, which collides with the global `-C/--dir`).
Source/destination are orthogonal: `store_path` (which store, default =
discovered from cwd) and `--at <PATH>` (where the proxy goes, default = cwd).
Also absorbs the `cue status` no-`.cue` bug by making `resolve_store` strict.

## Acceptance Criteria

1. **`cue link --at <target>` creates a proxy at the target.**
   - Verify by: integration test in `crates/cue/tests/link.rs`
   - Evidence: (pending)

2. **Linking from a linked worktree writes the real store, not a chain.**
   - Verify by: integration test asserting the new `STORE` points at
     `resolved.store_dir` and `validate_store_target` accepts it
   - Evidence: (pending)

3. **Orthogonal mode works (`store_path` + `--at` together).**
   - Verify by: integration test `cue link <abs_store> --at <target>`
   - Evidence: (pending)

4. **`cue status` with no `.cue` dir errors loudly.**
   - Verify by: integration test asserting non-zero exit and an error
     (no silent "active context: master (global)")
   - Evidence: (pending)

5. **Remedy hint is dynamic (`cue link` in linked worktrees, `cue init`
   otherwise).**
   - Verify by: integration tests covering both `.git`-file and `.git`-dir cases
   - Evidence: (pending)

6. **All workspace tests pass with no regressions.**
   - Verify by: `cargo test --workspace`
   - Evidence: (pending)
