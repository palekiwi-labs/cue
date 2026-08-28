# Project Log

## [a90b393] Opened fix branch

Created the build task and started branch fix/worktree-artifact-output-paths from merged master. The accepted behavior is that artifact creation output must resolve directly from the caller's current working directory, including linked worktrees.

- **Decided:** Verify the reported regression through the public CLI before changing implementation
- **Decided:** Use a focused red-green cycle for output path behavior

## [844454d] Fixed linked-worktree artifact output

Committed the regression fix as 844454d. `cue add` now prints paths relative to the invocation directory when the artifact is beneath it and otherwise prints the absolute artifact path, ensuring the output is directly openable from linked worktrees.

- **Found:** The regression test failed with `.test-mem/master/spec/test.md` because that path was interpreted inside the linked worktree
- **Found:** The focused regression and all 44 add integration tests pass
- **Found:** Cue clippy passes with warnings denied
- **Found:** Workspace-wide rustfmt check remains blocked by unrelated pre-existing formatting drift; the changed test file was formatted directly
- **Decided:** Preserve existing relative output in the main worktree
- **Decided:** Use an absolute path when the shared artifact store is outside the invocation directory

