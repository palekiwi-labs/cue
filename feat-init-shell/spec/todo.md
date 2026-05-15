# Todo: `mem init`

- [ ] Refactor `src/git.rs` for robustness
    - [ ] Make `run_git` generic over `AsRef<OsStr>`
    - [ ] Update `branch_is_checked_out` to use strict equality on porcelain lines
    - [ ] Enforce `refs/heads/` in branch existence checks
    - [ ] Fix `add_worktree_orphan` syntax
    - [ ] Improve `fetch_branch` with remote-tracking refspec
- [ ] Update `src/config.rs`
    - [ ] Handle missing `HOME` safely when merging global config
- [ ] Update `src/commands/init.rs`
    - [ ] Use exact path matching for worktree detection
    - [ ] Implement strict checkout check for the target branch
- [ ] Strengthen tests in `tests/init.rs`
    - [ ] Add upstream tracking assertion to `test_init_remote_branch_exists`
- [ ] Run all tests and verify fixes