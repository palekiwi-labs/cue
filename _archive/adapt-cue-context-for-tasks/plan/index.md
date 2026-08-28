---
status: complete
refs:
- .cue/master/task/adapt-cue-context-for-tasks.md
- .cue/master/spec/cue/task-mode.md
---
# Master Plan: Adapt cue context for tasks

## Problem

`gather_context` in `crates/cue/src/context/mod.rs:170` derives scope by
calling `get_current_branch()` and `sanitize_branch_name()`. All other
`cue context` subcommands already use `cuelib::head::resolve_scope()`. This
makes `cue context render` the only command in the family that still ignores
`.cue/HEAD`.

## Scope

- Fix `gather_context` to call `resolve_scope(&cue_dir)` instead of
  `get_current_branch()` + `sanitize_branch_name()`.
- Add/update integration tests proving render uses HEAD-derived scope.
- No changes to the rendered output format or content (task orientation
  injection was explicitly dropped — see master log).

## Constraints

- No changes to `ResolvedContext`, `Artifact`, or `context.json` schemas.
- `gather_context` signature may change only as far as necessary to accept
  the cue directory path (already derivable from `git_root` + `config.dir_name`).
- Cross-context `@branch:profile` syntax in `context.json` still resolves
  against a literal slug, not the current branch — no change in semantics,
  since slugs were already used everywhere else.

## Approach

Single-phase change:

1. In `gather_context`, derive `git_root` from `cwd` (already done), then
   derive `cue_dir` and call `resolve_scope(&cue_dir)` to get the active
   scope slug.
2. Remove the now-unused `get_current_branch` / `sanitize_branch_name` call.
3. Verify imports: `get_current_branch` may become unused in `context/mod.rs`
   and should be removed from the import list.
4. Write integration tests covering the two HEAD states (slug set, HEAD absent).
5. Run full test suite and clippy.
