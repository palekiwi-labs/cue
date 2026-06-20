---
status: open
---
---
status: complete
---
# Executive Plan - Remove Archived Status

This plan removes the `Archived` status from `TodoStatus` in `cuelib`, making it symmetric with `TaskStatus` as per Opus's advice. `archived` is recognized as an orthogonal visibility flag rather than a lifecycle status.

## Steps

- [x] Remove `TodoStatus::Archived` and update its methods in `crates/cuelib/src/artifact.rs`
- [x] Update unit tests in `crates/cuelib/src/artifact.rs`
- [x] Verify `cuelib` tests pass
- [x] Update `.cue/feat-curtain/spec/index.md` to reflect the decision
- [x] Close the todo artifact `.cue/master/todo/1781887990-2de5bc1/archived-as-separate-field.md`
- [x] Commit code changes (excluding `.cue/`)
- [x] Log completion with `cue-log`
