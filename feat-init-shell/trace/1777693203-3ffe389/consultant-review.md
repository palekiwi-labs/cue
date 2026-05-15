# Consultant Gemini Review - mem init

## 1. Critical Git Interactions & Bugs

### Invalid `--orphan` syntax
In `git.rs`, `add_worktree_orphan` used both `--orphan` and `-b`. For Git 2.42+, the correct syntax is:
`git worktree add --orphan <new-branch> <path>`

### Missing Upstream Tracking on Fetch
Fetching with `git fetch origin branch:branch` doesn't set upstream.
*Fix:* Fetch into remote-tracking ref and let `git worktree add` set up upstream.

## 2. Robustness & False Positives

### Worktree directory matching
Matching worktrees with `.ends_with()` is prone to false positives. Use exact path matching instead.

### Partial matches in branch checks
`branch_is_checked_out` used `.ends_with()` fallback which could match `feature/mem` instead of `mem`. Use strict equality on `branch refs/heads/<name>`.
Enforce `refs/heads/` prefix in `branch_exists_local` and `branch_exists_on_remote` to avoid matching tags.

## 3. Idiomatic Rust & Code Quality

### Native Path Handling (OsStr)
`run_git` should be generic over `AsRef<OsStr>` to handle paths containing non-UTF-8 characters safely without `unwrap()`.

### HOME environment fallback
`Config::load` should gracefully skip global config if `HOME` is unset instead of defaulting to a relative path.

## 4. Testing
Add assertion to `test_init_remote_branch_exists` to verify the branch is tracking `origin`.