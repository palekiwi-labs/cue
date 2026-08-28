---
title: Implement git-root store resolution and $CUE_TASK
status: complete
priority: high
refs:
  - .cue/design-worktree-store-and-task-env/spec/index.md
  - .cue/design-worktree-store-and-task-env/trace/1787568301-855ff6a/external-surfaces-audit.md
  - .cue/master/spec/cue/task-mode.md
kind: build
---

# Problem Statement

Implement first-class git worktree support in `cue` as specified by the
completed design task `design-worktree-store-and-task-env`:

1. **Store is always `<git-root>/.cue`** — auto-resolved from any
   worktree via the main-worktree entry of `git worktree list`. The
   `cue link` command and the `.cue/STORE` redirect file are removed
   entirely; no escape hatch.
2. **`$CUE_TASK` env var** joins scope resolution with precedence
   `--task` flag > `$CUE_TASK` > local `.cue/HEAD` > `master`, so
   agent-spawned child processes (cue-agent) write to the correct
   context without explicit flags.

Authoritative spec: `.cue/design-worktree-store-and-task-env/spec/index.md`
(self-contained; includes mechanism notes, affected components with
file:line citations, and resolved observability decisions).

External-surface impact audit with rollout checklist:
`.cue/design-worktree-store-and-task-env/trace/1787568301-855ff6a/external-surfaces-audit.md`
(no external code depends on link/STORE; docs-only refreshes needed).

## Scope

- `cuelib`: new `store::open(root, config)` chokepoint with git-root
  resolution; remove STORE following; `$CUE_TASK` rung in scope
  resolution.
- `cue` CLI: delete `link` command and STORE tests; migrate ~15 call
  sites; switch guard relaxation + `$CUE_TASK` stderr warning;
  status/context provenance and store-path output; `--task` flag on
  `cue context render`/`show` (absorbed from task
  `cue-context-task-flag`); init-in-worktree handling.
- Docs rollout: cue skill, cue.nvim comments, cue-plugins help text,
  README.

## Working rules

- Work on branch `feat/worktree-store-cue-task` (worktree
  `worktrees/feat-worktree-store-cue-task`, base `master`).
- TDD: failing tests first per phase; small commits per milestone.
- Prototyping stage: no version bumps, no back-compat shims.
- Log milestones and decisions to this task context as you go.
