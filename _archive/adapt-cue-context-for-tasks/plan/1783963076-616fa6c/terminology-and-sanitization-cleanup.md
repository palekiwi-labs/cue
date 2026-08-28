---
status: complete
refs: .cue/master/task/adapt-cue-context-for-tasks.md
---
# Executive Plan: Remove branch terminology and dead sanitization

## Foreword

Follow-up to the `adapt-cue-context-for-tasks` task. The scope resolution is
now HEAD-based, but the codebase still carries branch terminology and dead
`sanitize_branch_name` calls. This plan covers four agreed changes across
multiple files.

## Steps

- [x] 1. context/mod.rs: rename all `branch`/`sanitized_branch`/`branch_dir`
       to `scope`/`scope_dir`; update comments and warning messages.
- [x] 2. context/mod.rs: remove `sanitize_branch_name` from `parse_artifact_path`
       and `resolve_profile` (context.json refs use literal scope names).
- [x] 3. context/mod.rs: update unit tests to reflect no-sanitization behavior.
- [x] 4. commands/context.rs: rename `sanitized_branch` locals to `scope`.
- [x] 5. add/mod.rs, log/mod.rs, list/mod.rs, commands/log.rs: remove redundant
       `sanitize_branch_name` on HEAD-derived scope; add `validate_slug` for
       the `--task` override path.
- [x] 6. Run full test suite + clippy.
- [x] 7. Commit.
