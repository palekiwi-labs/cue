---
status: complete
priority: high
refs: tmp/1787806296-c9a7039/branch.diff
---
# Review fixes

Implement the verified review corrections before accepting the feature branch.

- [x] Validate non-empty `$CUE_TASK` as a task slug before using it as a store path component.
- [x] Remove ambient `$CUE_TASK` from the integration test harness by default.
- [x] Keep `cue add` and `cue log add` output store-root-relative in linked worktrees.
- [x] Remove the artificial `.cue/master/` store-validity invariant and unconditional initialization directory creation; scope directories should be created lazily by artifact writes.
- [x] Add explicit `--task` overrides consistently to scoped `cue context` subcommands that currently inherit `$CUE_TASK`.
- [x] Remove the hardcoded `.cue/` directory name from human-readable status output.
- [x] Preserve machine-readable `cue context show` and `render` stdout; clarify the design requirement instead of injecting provenance text.
- [x] Update the task-mode anchor to remove the obsolete statement that per-worktree HEAD support is deferred.
- [x] Run formatting and relevant tests in the Nix devshell.

The contrived case of invoking cue from inside the memory worktree is not part of this fix set unless implementation discovery shows it affects normal operation.
