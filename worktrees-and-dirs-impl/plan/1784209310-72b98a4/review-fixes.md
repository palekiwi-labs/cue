---
status: complete
refs:
- .cue/worktrees-and-dirs-impl/trace/1784209310-72b98a4/adversarial-review-findings.md
- .cue/worktrees-and-dirs-impl/spec/index.md
- .cue/worktrees-and-dirs-impl/plan/index.md
- .cue/worktrees-and-dirs-impl/log.md
---
# Review Fixes for Worktree Context Isolation

## Foreword

This executive plan addresses the merge-blocking findings from the adversarial
code review of `feat/worktrees-and-dirs-impl`. The review was performed in two
phases (parallel review by diff-reviewer-opus + consultant-gemini-flash, then
verification by consultant-opus against the actual source) and is recorded in
`trace/1784209310-72b98a4/adversarial-review-findings.md` with the milestone
logged in `log.md`.

Scope is exactly the agreed blocker set plus cheap landings: **C1, H1, H3, M6,
H5, M1** (and L6 bundled as it completes C1/H1). Out-of-scope items are captured
in `todo/review-deferred.md`. **H6 was dropped** after analysis showed the
worktree-isolation scope-per-agent invariant prevents the collision in the
designed workflow.

**C1 design decision (locked):** enforce the absolute-path STORE contract.
Reject non-absolute and empty/whitespace STORE contents loudly. Do not add
relative-path support. Rationale: the spec mandates absolute paths and the cast
deployment invariant (mount at identical inside/outside path) makes absolute
paths portable by construction; relative-against-`head_dir` would add a second
resolution code path and a new "relative to what?" bug class for zero benefit
at prototyping stage.

Ordering is TDD red-green per phase. Commit atomically per phase following the
git-commit skill; log each commit via `cue-log`.

## Steps

### Phase 1 - STORE contract enforcement (cuelib)

- [x] 1.1 Add unit tests in `crates/cuelib/src/store.rs` for the negative
      `resolve_store` cases: empty STORE file, whitespace-only STORE file,
      relative-path STORE contents. All must error with actionable messages.
      (Red.)
- [x] 1.2 Implement C1 guards in `resolve_store` (`store.rs:38-41`): reject
      empty/whitespace raw contents, reject non-absolute `target_path`, both
      before `validate_store_target`. (Green.)
- [x] 1.3 Add unit test: a valid store target that itself contains a nested
      `STORE` file must error with a message containing "chaining". (Red.)
- [x] 1.4 Implement H1 chaining check in `validate_store_target`
      (`store.rs:56-70`): `if target.join("STORE").exists() { bail!(...) }`
      placed after the existing `master/` check. (Green.)
- [x] 1.5 Make `validate_store_target` `pub` in `store.rs` (H5a) and re-export
      from `crates/cuelib/src/lib.rs` if not already covered by a wildcard.
- [x] 1.6 Update the `store.rs:21-27` doc comment to state the absolute-path
      and no-chaining contract (L6 - bundled because it completes C1/H1).

### Phase 2 - cue link validation reuse + master warning

- [x] 2.1 Refactor `crates/cue/src/commands/link.rs:8-19` to call
      `store::validate_store_target(&store_path)?` instead of the inline
      exists + `master/` check (H5b). Verify existing link integration tests
      still pass (they assert on `"master/"` and `"does not exist"`, both
      present in the shared validator's messages, so no test rewrite expected).
- [x] 2.2 Extend the `link_with_task_master_is_permitted` integration test to
      also assert `.stderr(predicate::str::is_empty())`. (Red - currently emits
      a spurious "no task card found for 'master'" warning.)
- [x] 2.3 Implement M1: skip the card-existence warning when `slug == "master"`
      at `link.rs:54` (`if slug != "master" && !card.exists() { ... }`).
      (Green.)

### Phase 3 - Path rendering under STORE redirect (H3 + M6)

- [x] 3.1 Add an integration test in `crates/cue/tests/` that sets up a proxy
      worktree via `cue link`, runs `cue list --json` inside it, and asserts
      the emitted `path` fields are store-relative (begin with `master/` or a
      scope slug; contain no absolute host prefix). (Red.)
- [x] 3.2 Add an integration test that runs `cue context render` inside the
      same proxy setup and asserts the `<artifact path="...">` attribute is
      store-relative. (Red.)
- [x] 3.3 Fix `crates/cue/src/commands/list.rs:33` (human output): strip
      against `resolved.store_dir` first, fall back to `root`. (Green for list
      human output.)
- [x] 3.4 Fix `to_cue_file` in `crates/cue/src/list/mod.rs:259-261` (JSON
      output): strip against `cue_path` (already = `store_dir`) before `root`.
- [x] 3.5 Fix `handle_render` in `crates/cue/src/commands/context.rs:64-67`:
      resolve the store inside the handler and strip `artifact.path` against
      `resolved.store_dir` before `git_root`. (Green.)
- [x] 3.6 Fix `handle_init` in `crates/cue/src/commands/context.rs:18-24`
      (M6): strip `config_path` against `resolved.store_dir` before `git_root`.

### Phase 4 - Verification and commit

- [x] 4.1 `cargo test --workspace` passes (excluding the pre-existing acuity
      rustc ICE which is unrelated).
- [x] 4.2 `cargo clippy --workspace --tests -- -D warnings` is clean.
- [x] 4.3 One atomic commit per phase (git-commit skill); `cue-log` after each
      commit summarizing what landed.

## Out of Scope

Tracked in `todo/review-deferred.md`: H2 (traversal guard design decision),
H4 (cue link + custom dir_name), H7-broad (remaining read-path coverage),
M2, M3, M4, M5, L1, L2, L4, L5, L6 is bundled here. H6 is dropped entirely.
