# Project Log

## [e798914] Review task opened

Created and started a dedicated review task for the completed feat/worktree-store-cue-task implementation. The review is separated from implementation to preserve independent findings and will cover specification alignment, regression risk, CLI behavior, tests, documentation, and legacy STORE/cue link removal.

- **Decided:** Review findings will be recorded without silently modifying implementation code
- **Decided:** Task status starts at in-progress as requested

## [c9a7039] Consultant review found init repair gap

Saved the feature branch diff after moving feat/worktree-store-cue-task into the repository root. Gemini Flash reviewed the saved diff and reported one medium-severity init idempotency defect. Direct inspection confirmed that init returns when the cue directory exists before creating the required master directory. Opus review was attempted twice but could not start because its provider refresh token is expired.

- **Found:** Saved diff: tmp/1787806296-c9a7039/branch.diff
- **Found:** crates/cue/src/init/mod.rs returns at lines 11-16 when .cue exists, bypassing master directory creation at line 31
- **Found:** crates/cuelib/src/store.rs lines 60-65 rejects stores missing master and instructs users to run cue init, so the documented repair path cannot repair this state
- **Found:** Consultant Opus failed twice with invalid_grant because its refresh token expired
- **Open:** Re-run the Opus review after its provider credentials are refreshed
- **Open:** Decide whether the verified init repair defect should block acceptance

## [c9a7039] Opus review completed

Opus successfully reviewed the saved branch diff on retry and reported ten findings. Source inspection independently confirmed the core path traversal, ambient CUE_TASK test isolation, linked-worktree output path, memory-worktree scope fallback, init repair loop, context observability, partial context --task coverage, and hardcoded status path concerns. Runtime test reproduction could not be rerun in the current shell because cargo is unavailable outside the project devshell. The formatting concern and stale anchor-spec note remain lower-priority follow-up observations.

- **Found:** High: unvalidated CUE_TASK values become path components and can escape the store
- **Found:** High: integration TestEnv does not remove ambient CUE_TASK
- **Found:** Medium-high: add/log strip output paths against the local root instead of the store root
- **Found:** Medium: running inside the .cue worktree derives .cue/.cue as head_dir and silently falls back to master
- **Found:** Medium: cue init cannot repair an existing store missing master/
- **Found:** Medium: context show/render resolve scope but do not report provenance or store path
- **Found:** Low-medium: context init/profiles/path honor CUE_TASK but expose no --task override
- **Found:** Low: status hardcodes .cue in its context display despite configurable store directory
- **Found:** Low: possible branch-local rustfmt drift requires devshell verification
- **Found:** Low: task-mode anchor documentation remains stale outside the code diff
- **Open:** Run runtime reproductions and formatting checks through the Nix devshell if needed
- **Open:** Decide which verified findings block feature acceptance

## [c9a7039] Flash verified Opus findings

Gemini Flash independently checked all ten Opus findings using the saved diff, source, and Nix devshell reproductions. Nine were confirmed and the context provenance/store-path finding was partially confirmed as a specification mismatch: adding metadata to context show/render stdout would break their machine-oriented output contracts.

- **Found:** Confirmed blockers: CUE_TASK path traversal, ambient CUE_TASK test contamination, absolute add/log paths in linked worktrees, .cue worktree scope misresolution, and init repair loop
- **Found:** Confirmed follow-ups: incomplete context --task coverage, hardcoded .cue status path, rustfmt drift, and stale task-mode documentation
- **Found:** Partially confirmed: context show/render omit provenance and store path as written in the design spec, but changing stdout would break raw JSON/artifact-stream consumers
- **Decided:** Recommend fixing the five correctness/security/test-isolation blockers before merge
- **Decided:** Recommend resolving context observability by clarifying the spec rather than contaminating machine-readable stdout
- **Decided:** Recommend formatting and low-risk CLI consistency fixes before merge where they remain within intended scope

## [bd1dde5] Implemented verified review fixes

Committed the agreed review corrections as bd1dde5. The implementation now validates CUE_TASK slugs, isolates integration tests from ambient CUE_TASK, keeps add/log output relative to the shared store root, removes master/ as an artificial store-validity marker while retaining a simple store-directory existence check, adds --task to remaining scoped context subcommands, and uses the configured directory name in status output. Machine-readable context output remains unchanged. The scoped design and task-mode anchor documentation were corrected in the shared cue store.

- **Found:** An initialized store only needs its configured store directory to exist; master scope directories are created lazily
- **Found:** The full cue and cuelib test suites pass in the Nix devshell
- **Found:** Clippy passes for cue and cuelib with warnings denied
- **Decided:** Do not address invocation from inside the memory worktree because it is contrived and outside the agreed fix set
- **Decided:** Preserve context show/render stdout contracts and narrow the observability specification instead

## [b5fb1fa] Clarified worktree root naming

Renamed the ambiguous root helpers after the walkthrough exposed that their different semantics were not visible at call sites. The current checkout resolver is now `current_worktree_root`, while shared store ownership resolves through `main_worktree_root`. Also corrected the stale store test-helper comment. Committed as b5fb1fa.

- **Found:** The previous `git_root` and `get_git_root` names obscured the main-worktree versus current-worktree distinction
- **Found:** The helper creates a master scope for test setup but master is no longer required for store validity
- **Decided:** Use `main_worktree_root` for the shared store owner
- **Decided:** Use `current_worktree_root` for checkout-local HEAD state

## [ea51d07] Validated HEAD-derived task scopes

Closed the scope validation asymmetry identified during the walkthrough. `resolve_scope` now applies the same single-path-segment validation to values read from worktree-local HEAD as it already did to flag and CUE_TASK values. Added a regression test through a red-green cycle and committed as ea51d07.

- **Found:** A manually edited or corrupted HEAD could previously return a traversal scope even though flag and environment scopes were validated
- **Decided:** Every non-default scope provenance must satisfy the same slug safety invariant

