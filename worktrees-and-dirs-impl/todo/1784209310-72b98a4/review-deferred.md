---
status: open
priority: normal
refs:
- .cue/worktrees-and-dirs-impl/trace/1784209310-72b98a4/adversarial-review-findings.md
- .cue/worktrees-and-dirs-impl/plan/1784209310-72b98a4/review-fixes.md
---
# Review Findings Deferred from worktrees-and-dirs-impl

Verified-but-deferred items from the adversarial review
(trace: `trace/1784209310-72b98a4/adversarial-review-findings.md`). These are
real but outside the merge-blocker set being implemented in
`plan/1784209310-72b98a4/review-fixes.md`. Promote individually to a task on
master when prioritized.

## High

### H7-broad: remaining read-path test coverage gaps
Negative `resolve_store` unit cases not covered: symlinked STORE target, STORE
pointing at a file (not a dir). No integration test exercises the redirect on
the read side for `cue add`, `cue log add`, `cue log list`. (The review-fixes
plan covers `cue list` and `cue context render` integration tests as part of
H3; this item is the remainder.) This coverage gap is structurally what
allowed H3 to pass CI - worth closing fully.

## Normal

### H2: context traversal guard over-narrowed (design decision needed)
`crates/cue/src/context/mod.rs:172,191` - the refactor changed the traversal
guard anchor from `canonical_git_root` to `canonical_store`. This was necessary
(the old anchor blocked all proxy reads) but over-narrowed: profile refs to
files outside `.cue/` (repo source, READMEs, scripts) are now silently blocked.
Decision required: do context profiles support out-of-`.cue/` refs? If yes,
dual-anchor the guard (`store_dir` OR `git_root`); if no, document the
tightening in the spec. Not a blocker - no spec support and no test for the
affected use case today.

### M5: silent exit 0 on dangling --task slug
`cue link --task <typo>` warns to stderr but exits 0 (spec-conformant at
`spec/index.md:105`). Orchestrators parsing exit codes miss the typo, creating
an isolated scope nobody inspects. Design discussion: add `--allow-dangling` /
`--strict` flag, or rely on the orchestrator-creates-cards-first SOP
(`spec/index.md:255`). Changes the documented exit-code contract.

### L1: read_head swallows all read errors as "no HEAD"
`crates/cuelib/src/head.rs:9` uses `.ok()?`, collapsing "absent" and
"unreadable/corrupt" HEAD into `None` -> silent drop into `master` scope.
Pre-existing; feature raises the stakes (cross-context bleed). Distinguish
`NotFound` from other `io::Error` kinds and fail loud on corrupt HEAD.

### L2: spec contradicts adopted cue switch decision
`.cue/worktrees-and-dirs-impl/spec/index.md` "Out of Scope" list still includes
the agent `cue switch` prohibition, but the decision (recorded in `log.md`)
dropped it. Requires a human-authored spec edit to reconcile.

### M7: --dir precedence misleading in spec docs
`spec/index.md:65-73` frames `--dir` as overriding store location; actually it
overrides CWD for `git::get_git_root`, and `head_dir` is always git-root-
relative. Doc-only fix.

## Low

### M2: duplicate resolve_store call in cue list
`crates/cue/src/commands/list.rs:25` + `list/mod.rs:141` resolve the store
twice per invocation. Resolve once and thread `ResolvedStore` through. Cheap
cleanup; couples naturally to Phase 3 of the review-fixes plan if bundled.

### M3: canonical_store.to_str().unwrap_or("") silent empty STORE write
`crates/cue/src/commands/link.rs:41`. Non-UTF-8 store path yields an empty
STORE file -> confusing downstream error. One-liner fail-loud fix; sits
adjacent to M1/H5 changes in the review-fixes plan if bundled.

### M4: symlink attack surface in validate_store_target
`crates/cuelib/src/store.rs:56-69` uses `exists()`/`is_dir()` which follow
symlinks; a `master/` symlinked elsewhere passes validation. Canonicalize the
target before validating. Couples to H1's chaining check (same function) if
bundled. Low-value in the current trust model (orchestrator writes STORE).

### H4: cue link ignores custom dir_name config
`crates/cue/src/commands/link.rs:27` hardcodes `.cue/` without loading
`Config`. Breaks projects that customized `dir_name`. Tension: `cue link`
deliberately has no git dependency, but `Config::load` reads from git root.
Needs verification that `Config::load` tolerates non-git dirs before fix.

### L4: cue link emits no .gitignore hint
Proxy `.cue/` shows as untracked in worktree `git status`. User confirmed
`.cue/` is already project-gitignored so no management is needed, but a
one-line stderr reminder would reduce foot-guns.

### L5: unrelated curator/ui.rs churn inflates PR diff
~130 lines of block relocation in `crates/curator/src/ui.rs` (the
`items_after_test_module` clippy fix) plus formatting churn from the toolchain
bump inflate the feature PR diff. PR-hygiene note; consider isolating
clippy-driven fixes into a separate commit/PR. No code defect - pure
relocation.
