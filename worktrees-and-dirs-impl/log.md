# Project Log

## [bf3b026] Audited all command call sites for store resolution

- **Found:** All commands resolve store via the same pattern: git::get_git_root(cwd) -> root.join(config.dir_name) -> the single cue_dir used for both HEAD and artifact I/O
- **Found:** Commands that need ResolvedStore: add (and wrappers task/plan/todo/note), log add, log list, list, switch, status, context
- **Found:** cue init creates the real store and runs before STORE exists - no ResolvedStore needed for MVP
- **Found:** Core HEAD helpers (read_head, write_head, resolve_scope) live in crates/cuelib/src/head.rs
- **Found:** Artifact batch reads live in crates/cuelib/src/artifact.rs:244
- **Found:** cue task/plan/todo/note are wrappers that delegate to crate::add::add - one refactor covers all
- **Found:** User confirmed: .cue/ is already in project .gitignore so worktrees inherit it - no gitignore management needed in cue link
- **Found:** User confirmed: cue link must accept --dir flag
- **Found:** User confirmed: STORE chaining validation deferred as todo
- **Found:** User confirmed: cue link --task master is permitted
- **Decided:** STORE chaining detection deferred as todo item
- **Decided:** cue link will accept --dir to specify target directory for proxy .cue/ creation
- **Decided:** cue link --task master is allowed
- **Decided:** cue init excluded from ResolvedStore refactor for MVP

## [1c21233] Phase 1 complete: ResolvedStore in cuelib

Committed feat(cuelib): add ResolvedStore and resolve_store function (1c21233).

- **Found:** resolve_store needs #[derive(Debug)] on ResolvedStore for unwrap_err in tests - added immediately
- **Found:** All 67 cuelib tests pass, clippy clean
- **Decided:** ResolvedStore derives Debug to support test ergonomics
- **Decided:** validate_store_target is a private helper (not pub) - callers only need resolve_store

## [d3e0957] Phase 2 complete: all command call sites refactored

Committed refactor: thread ResolvedStore through all command call sites (d3e0957). All workspace tests pass, clippy clean.

- **Found:** context/mod.rs required refactoring context_json_path, parse_artifact_path, and resolve_profile signatures - three internal helpers all needed cue_dir instead of (root, dir_name)
- **Found:** gather_context path-traversal guard used canonical_git_root which would block all artifact reads in proxy worktrees - fixed to use canonical store_dir
- **Found:** resolve_scan_paths needed separate head_dir and store_dir params since it reads HEAD (local) but scans files (store)
- **Found:** commands/list.rs independently computes resolve_store so to_cue_file can use store_dir
- **Found:** status.rs had a redundant use cuelib::store import (used full path instead) - cleaned up by clippy
- **Decided:** Each domain function computes resolve_store internally (not passed from handler) - cheap read-only op, avoids signature churn
- **Decided:** list.rs handler also computes resolve_store independently for to_cue_file - two calls is fine
- **Decided:** context_json_path, parse_artifact_path, resolve_profile: new signatures take cue_dir directly, cleaner than (root, dir_name) tuple

## [c7fa32b] [c7fa32b] Phase 3 complete: cue link subcommand

Committed feat: add cue link subcommand for proxy worktree setup (c7fa32b). All 10 integration tests pass, full workspace tests pass (acuity ICE is pre-existing), clippy clean.

- **Found:** cue link does not require a git repo - it operates purely on the filesystem, no git::get_git_root call needed
- **Found:** global -C/--dir flag (global = true in clap) naturally handles the --dir requirement from the spec: cue -C <worktree> link <store> works without a separate per-command flag
- **Found:** acuity crate has a pre-existing rustc ICE (internal compiler error) unrelated to cue link changes - confirmed by stash test
- **Found:** switch.rs had two pre-existing collapsible_if clippy errors, fixed as part of this commit
- **Decided:** cue link handle() takes (cwd, store_path, task) - no git dependency
- **Decided:** use global -C flag for --dir instead of a per-command flag; they are semantically equivalent
- **Decided:** fix pre-existing clippy errors in switch.rs in same commit since they block cargo clippy --workspace -D warnings
- **Decided:** 10 integration tests cover: happy path, --task writes HEAD, master task permitted, missing card warns, matching card no warning, store not exists, store lacks master/, .cue already exists, traversal slug rejected, -C flag targeting different dir
- **Open:** Phase 4 (deferred): STORE chaining detection - todo captured in spec

## [c7fa32b] Dropped cue switch prohibition from agent skill

Resolved the todo "Let agents use cue switch in skill" by dropping the prohibition entirely (Option B, user decision). In the worktree-isolation world the skill's premise (shared .cue/HEAD) is obsolete: each proxy worktree has its own local HEAD, so an agent calling cue switch cannot clobber other contexts.

Edit made in the external cue-plugins repo (skills/cue/SKILL.md, branch feat/task-mode-cli) — NOT in this repo. Left uncommitted there for the user.

This decision deviates from the worktrees-and-dirs-impl spec, which explicitly listed "cue switch in proxy worktrees from an agent (prohibited by skill)" as out of scope. The spec text now contradicts the adopted decision and should be reconciled.

- **Found:** The skill's rule 4 premise ('Agents share the .cue/HEAD file ... silent context collisions') was the basis for the prohibition and is now factually obsolete under worktree isolation
- **Found:** cue-plugins is a separate git repo on branch feat/task-mode-cli — skill edit lives there, not in the cue repo
- **Found:** The worktrees-and-dirs-impl spec ('cue switch in a Proxy Worktree' section + 'Out of Scope' list) now contradicts the adopted decision
- **Decided:** Option B: full permission — drop the cue switch prohibition for agents entirely, no situational branching (user explicitly rejected the distinguish-two-cases approach as 'extra noise' that confuses agents)
- **Open:** Spec reconciliation: .cue/worktrees-and-dirs-impl/spec/index.md still says agents must never call cue switch and lists it as out of scope — needs updating to match the decision
- **Open:** Skill change in cue-plugins repo is uncommitted (left for user)

## [ec98431] Completed switch-in-proxy test and dead-code cleanup

Two commits close out the remaining code work for acceptance criteria 7 and 8.

1. style(cffd2ab): Added #[allow(dead_code)] to setup_git_repo in tests/helpers.rs. The function is used by some test binaries (switch.rs) but not others (link.rs); each test binary compiles the helpers module independently, so the warning appeared in units that don't call it. Matches the pattern of all other helpers in the file.

2. test(ec98431): Added switch_in_proxy_writes_head_locally_scope_dir_in_store integration test to switch.rs, satisfying acceptance criterion 7. Sets up a worktree as its own git repo (so git rev-parse --show-toplevel returns the worktree path, simulating a real git worktree), links it to a shared store via 'cue link', runs 'cue switch proj-123-impl', and asserts HEAD lands in the local .cue/HEAD while the scope dir is created under the STORE target with no local leakage.

- **Found:** Pre-existing clippy errors confirmed unrelated: dir_flag.rs empty_line_after_doc_comments (cue) and items_after_test_module (curator) — both reproduce on a stashed tree, neither file is in this branch's diff
- **Found:** setup_git_repo is genuinely used by switch.rs tests but not link.rs tests; the per-test-binary compilation of the helpers module is what triggers the dead_code warning only in link
- **Found:** cue switch resolves the git root via rev-parse --show-toplevel, so a worktree must be its own git repo (or real git worktree) for head_dir to point at the proxy .cue/ rather than the main project root
- **Decided:** Made the proxy worktree its own git repo via setup_git_repo rather than a real 'git worktree add' — the behavior under test is the STORE redirect, not git worktree mechanics, and a standalone git init correctly reproduces the toplevel property
- **Decided:** Two atomic commits: dead-code cleanup separate from the new test
- **Open:** Acceptance criterion 9 (manual end-to-end orchestrator workflow) still requires human attestation — todo to be created for the manual QA checklist

## [72b98a4] Resolved pre-existing clippy lints blocking workspace clean build

Fixed two pre-existing clippy violations that were unrelated to the worktree-isolation feature but blocked `cargo clippy --workspace --tests -- -D warnings`.

1. crates/cue/tests/dir_flag.rs: empty_line_after_doc_comments — the module-level description used /// doc-comment syntax with a blank line before the first item. Converted to plain // comments (it describes the file, not a specific item).

2. crates/curator/src/ui.rs: items_after_test_module — 9 production helper functions (format_datetime_on, harness_abbrev, trunc_pad, format_tokens, format_event_datetime, session_unique_agents, session_unique_models, acuity_status_parts, status_help_line) were defined after the #[cfg(test)] mod tests block. Moved the test module to the end of the file (idiomatic Rust layout). Pure block reorder, no content change.

Verified: cargo clippy --workspace --tests -- -D warnings is now fully clean. curator tests (127) and dir_flag tests (8) still pass.

- **Found:** Both lints are newly enforced by clippy 1.96.0 in this environment; the code was clean when originally written under older toolchains
- **Found:** The items_after_test_module lint fires only in test builds (the #[cfg(test)] module is omitted in release builds), so the misordered production functions compiled fine in non-test builds
- **Found:** ui.rs test module spans lines 891-1449 (563 lines); the 9 trailing production functions span lines 1451-1580 (130 lines)
- **Decided:** For ui.rs, moved the test module to the end rather than moving the 9 production functions up before it — both produce production-code-first layout, but relocating one contiguous test block is cleaner than scattering 9 functions and keeps git's move detection happier
- **Decided:** Single commit for both lint fixes since they share one responsibility: making the workspace clippy-clean under the current toolchain

## [72b98a4] Adversarial review completed for worktrees-and-dirs-impl

Two-phase adversarial code review of feat/worktrees-and-dirs-impl against master (merge base bf3b026). Findings saved at .cue/worktrees-and-dirs-impl/trace/1784209310-72b98a4/adversarial-review-findings.md.

Phase 1: parallel review by diff-reviewer-opus and consultant-gemini-flash against the saved branch diff. Produced 21 synthesized findings (C1, H1-H7, M1-M7, L1-L6).

Phase 2: consultant-opus verified every finding against the actual source, marking each Confirmed/Refuted/Partial, and proposed concrete fixes with file:line citations. Verdict: true merge-blockers narrowed to C1 (STORE relative-path resolves against process CWD) and H3/M6 (host-path leakage in cue list / context render / context init under STORE redirect). H1+H5 (chaining detection + validate_store_target reuse in cue link) recommended as cheap landings alongside. M1 (spurious --task master warning) bundled as trivial.

H6 (point-in-time artifact collision under parallel agents) was investigated and DROPPED. The review and consultant focused only on the <timestamp>-<hash> leaf component, but the full artifact path is <store_dir>/<scope>/<type>/<ts>-<hash>/<filename> and the <scope> component is per-agent by worktree-isolation design (each worktree has its own HEAD). A collision requires a scope-sharing misconfiguration (which defeats the feature's purpose) plus identical filename choice plus identical base commit plus TOCTOU timing. The spec's own concurrency model (spec/index.md:204-218) assumes one-scope-per-agent, so the designed workflow prevents H6 entirely. Not filed as a task.

- **Found:** C1 CONFIRMED: resolve_store (store.rs:38-43) has no is_absolute() guard; relative STORE resolves against process CWD. cue link only ever writes canonicalized absolute paths, so only reachable via hand-edited STORE.
- **Found:** H1 CONFIRMED: validate_store_target (store.rs:56-70) has no chained-STORE check; spec mandates loud error (index.md:62-63). Deliberately deferred per plan/log but spec disagrees -> reconcile.
- **Found:** H3/M6 CONFIRMED: every strip_prefix(&root) in list.rs:33, list/mod.rs:259-261, context.rs:64-67 and context.rs:18-24 falls through to absolute host path under STORE redirect. Triggers on every proxy invocation. No integration test covers read paths in a proxy -> CI passed while broken.
- **Found:** H4 CONFIRMED: cue link (link.rs:27) hardcodes .cue/ without loading Config -> breaks customized dir_name. Deferred (most projects use default).
- **Found:** H5 CONFIRMED: link.rs:8-19 duplicates validate_store_target logic inline; the validator is private. Bundling with H1 fix.
- **Found:** H7 CONFIRMED: 10 link tests cover proxy creation; switch test covers write-split; NO test exercises read-side redirect. Coverage gap is why H3 sailed through CI.
- **Found:** H2 PARTIAL: traversal guard anchor changed from canonical_git_root to canonical_store (context/mod.rs:172,191). Necessary for the feature (old guard blocked all proxy reads) but over-narrowed: profile refs to files outside .cue/ now blocked. Latent behavior change with no test; not a demonstrated broken workflow. Downgraded from blocker.
- **Found:** H6 REFUTED-as-blocker: collision requires shared scope which contradicts worktree-isolation design. Dropped.
- **Found:** L3 REFUTED: resolve_store taking PathBuf by value is net-neutral at call sites (callers construct fresh PathBuf and move in). No action.
- **Decided:** Merge-blocker set: C1 + H3/M6. H1+H5 landed alongside as cheap bundle. M1 bundled as trivial.
- **Decided:** C1 resolution: enforce absolute-path STORE contract. Reject non-absolute and empty/whitespace STORE loudly. Do NOT add relative-path support. Rationale: spec mandates absolute; cast mount invariant (identical inside/outside path) makes absolute portable by construction; relative-against-head_dir adds a second code path and new bug class for zero benefit at prototyping stage.
- **Decided:** H6 dropped entirely. The scope component of the artifact path differs per agent by design; the designed workflow prevents the collision. No task filed.
- **Decided:** H2 not a blocker: the guard change was necessary and intentional; affects only out-of-.cue profile refs (no spec support, no test). Deferred to todo pending a design decision on whether out-of-.cue refs are a supported feature.
- **Decided:** Decomposition: fixes stay under existing task worktrees-and-dirs-impl (blockers are part of its done-state); one executive plan review-fixes.md covers the coupled bundle; deferred items captured in todo/review-deferred.md; no new tasks created.
- **Open:** Should context profiles be allowed to reference files outside .cue/ (e.g. repo source/READMEs)? If yes, dual-anchor the traversal guard (store_dir OR git_root). If no, document the tightening. (H2 design decision)
- **Open:** cue link handling of customized dir_name (H4): Config::load must tolerate non-git dirs since cue link deliberately has no git dependency. Needs verification before fix.
- **Open:** --allow-dangling / --strict flag for cue link --task (M5): design discussion, changes documented exit-0 contract.

## [69232aa] Phase 1 complete: STORE contract enforcement in cuelib (69232aa)

- **Found:** C1 (empty/whitespace STORE): PathBuf::from("") was resolving against process CWD producing confusing 'does not exist: ' error; now caught immediately with 'STORE file is empty' before PathBuf construction
- **Found:** C1 (relative path): non-absolute STORE was resolving against process CWD non-deterministically; now rejected before validate_store_target with 'must contain an absolute path' message
- **Found:** H1 (chaining): validate_store_target had no chained-STORE check; target.join("STORE").exists() check added after master/ check
- **Found:** H5a: validate_store_target is now pub, enabling cue link to reuse it directly (Phase 2 prerequisite)
- **Found:** All 9 store unit tests green; full workspace test suite clean; clippy clean
- **Decided:** Single commit for C1+H1+H5a+L6 since they are all on the same function group in store.rs and form one logical unit (STORE contract enforcement)
- **Decided:** Chaining check placed last in validate_store_target (after exists() and master/) so earlier, more fundamental checks fire first
- **Decided:** Doc comment updated to state the absolute-path contract explicitly with a note that callers must ensure the path is absolute before calling validate_store_target

## [e70eece] Phase 2 complete: link.rs reuses validate_store_target, master card warning suppressed (01c7eca)

- **Found:** H5b: replaced 17-line inline exists+master/ check in link.rs with single store::validate_store_target call - also gains the chaining check for free
- **Found:** M1: added slug != 'master' guard before card-existence warning; link_with_task_master_is_permitted test now asserts empty stderr and passes
- **Found:** validate_store_target is already pub from Phase 1 - no additional export change needed
- **Decided:** Single atomic commit 01c7eca covers H5b + M1 + test update - they share one logical unit (link validation correctness)

## [e70eece] Phase 3 complete: path rendering under STORE redirect fixed (e70eece)

- **Found:** H3/M6 FIXED: list.rs human output, to_cue_file JSON path, handle_render, handle_init all now strip against store_dir first, falling back to git_root
- **Found:** handle_render needed an explicit store::resolve_store call since gather_context does not expose store_dir in its return value
- **Found:** handle_init similarly: init_context returns an absolute config_path inside store; now strips against store_dir
- **Found:** Three existing tests required path expectation updates: context_init (Created master/context.json not .cue/master/...), context_render (path= attributes now store-relative), list (test_list_from_subdirectory expects master/spec/index.md)
- **Found:** 3 new integration tests in proxy_reads.rs confirm list --json, list human, and context render all emit store-relative paths from a proxy worktree
- **Found:** H7 coverage gap closed: proxy read paths now covered by CI
- **Decided:** strip_prefix(store_dir).or_else(strip_prefix(root)) pattern used consistently in all four sites - provides graceful fallback for non-proxy case
- **Decided:** proxy_reads.rs is a new test file (not added to link.rs) to keep proxy read coverage clearly separated from proxy creation tests
- **Decided:** Single atomic commit e70eece covers all path-rendering fixes + test updates + new proxy tests

