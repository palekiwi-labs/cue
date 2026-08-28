---
title: Native store resolution and CUE_TASK env
status: complete
priority: normal
refs:
  - .cue/design-worktree-store-and-task-env/spec/index.md
  - .cue/master/spec/cue/task-mode.md
kind: design
---

# Problem Statement

`cue` is used increasingly by agents operating inside git worktrees, but the
current store-linking and task-selection primitives assume a single checkout
and a human-driven interactive flow:

1. **Store resolution requires manual linking.** Each worktree must run
   `cue link` and maintains a `.cue/STORE` pointer file to find the shared
   store. This is clunky, error-prone, and unnecessary given that the store
   is (by convention) the `.cue/` directory at the git root. Since `cue`
   stores its data on an orphan branch inside the very repo it serves,
   every worktree of that repo should share the same store implicitly.

2. **`.cue/HEAD` conflates human and agent context selection.** `.cue/HEAD`
   is owned by the human operator, but agents need a reliable way to pin
   their task scope, especially child agent processes spawned by the
   upcoming `cue-agent` binary. An environment variable (`$CUE_TASK`)
   would let a parent process guarantee that children write to the correct
   task context without relying on `--task` flags being passed everywhere.

# Proposed Directions (to be refined in discussion)

- Remove `cue link` and the `.cue/STORE` file; standardize on resolving the
  store as `.cue/` in the git worktree/common root automatically.
- Introduce `$CUE_TASK` env var with precedence: `--task` argument >
  `$CUE_TASK` > `.cue/HEAD`.

# Objectives

- Analyze current store resolution, `cue link`, and `.cue/STORE` mechanics.
- Analyze current scope/task resolution (`.cue/HEAD`, `--task`).
- Evaluate implications of removals and the new precedence chain.
- Converge with the user on a specification for both changes.
