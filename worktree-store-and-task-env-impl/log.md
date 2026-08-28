# Project Log

## [82e74da-dirty] Scope absorption: cue-context-task-flag merged in; plugin command split out

Reorganized related scope-resolution work into a clearer split.

1. Absorbed task `cue-context-task-flag` (now closed) into this task:
   plan Phase 6 gained an explicit checkbox for `--task` on
   `cue context render`/`show`; task card scope and acceptance criteria
   updated. Rationale: the `(flag)` provenance label required by Phase 6
   cannot exist without the flag, and both share the same
   scope-resolution code path in head.rs — separate implementation
   would touch it twice. Timing was free: the implementation worktree
   does not exist yet and the main checkout is on
   feat/switch-task-association, so no rebase churn.
2. Created cue-plugins task `task-command-positional-slug`: the
   `/task <slug>` custom command interpreting $ARGUMENTS position 1 as
   a task slug. Only the Phase 6 flag checkbox blocks it, not this
   whole task.

- **Found:** crates/cue/src/cli.rs:116-119 Context subcommand takes no --task flag today
- **Found:** Precedence chain --task > $CUE_TASK > .cue/HEAD > master means the explicit flag correctly overrides a session-pinned CUE_TASK
- **Decided:** cue-context-task-flag closed as superseded by worktree-store-and-task-env-impl (refs link retained)
- **Decided:** Plugin-side /task command kept as a separate cue-plugins task: different repo, deliverable only needs the flag to exist

## [e798914] P1 cycles 1-2 green: store::open + store::git_root core mechanism

TDD cycles 1-2 of Phase 1 committed (9d97a24) on feat/worktree-store-cue-task (worktree worktrees/feat-worktree-store-cue-task, base master, branch.base set). store::git_root resolves the store-owning main worktree (list_worktrees[0] normalized by get_git_root, per spec mechanism note); store::open(root, config) splits head_dir (local toplevel) from store_dir (git root). Unit tests shell out to git via a panicking helper; expectations are derived from get_git_root so tests are symlink-safe. cargo fmt on this toolchain reformats unrelated files (config.rs, head.rs) due to style-edition drift; those hunks were reverted and excluded from the commit.

- **Found:** Repo is not fmt-clean with cargo 1.96/rustfmt 2024 style edition: config.rs and head.rs drift; store.rs import line now uses {Context, Result, bail} ordering
- **Found:** Vec<PathBuf>::first() is Option<&PathBuf> - use reference directly, not .copied()
- **Decided:** open(root, config) re-derives git_root internally; the public store::git_root helper exists for callers to load Config from the store owner (Phase 1 bullet: config loaded from git root)
- **Decided:** Test expectations derived from get_git_root output rather than raw tempdir paths to stay robust against symlinked TMPDIR

## [e798914] P1 cycles 3-6 green: validation + edge scenarios complete

Committed 88b2763 (master/ validation bail with cue init hint) and 8feaf8b (scenario coverage: bare main, submodule no-inheritance, stray local master ignored, custom dir_name at git root, git_root helper from linked worktree including nested subdir). Full workspace test run green (cuelib 80, cue 127, all integration binaries). Phases covered: plan bullets 1, 2, and 4 (config-from-git-root via public store::git_root helper; open doc-comment states the caller contract).

- **Found:** git >= 2.38.1 ignores repo-local protocol.file.allow for submodule clones; inline -c or GIT_ALLOW_PROTOCOL=file is required in test fixtures
- **Decided:** Verification cycles 4-6 passed against the already-complete mechanism; only fixture fix needed (inline -c protocol.file.allow=always, since repo-local config is ignored for submodule transport security)

## [e798914] Phase 1 complete; STORE removal interlocked with Phase 2

Phase 1 implementation complete on feat/worktree-store-cue-task (commits 9d97a24, 88b2763, 8feaf8b). store::open(root, config) and store::git_root are implemented per the spec mechanism with 9 unit tests covering all six planned scenarios plus custom dir_name and the git_root helper. Workspace fully green. The STORE-removal bullet cannot land green in isolation: crates/cue/src/commands/link.rs:8 calls validate_store_target and tests/link.rs, tests/proxy_reads.rs, tests/switch.rs assert STORE-following end-to-end. Plan updated: the removal is annotated as interlocked and must land as the first commit of Phase 2.

- **Decided:** Phase 1 STORE-removal bullet deferred to land with Phase 2 deletions rather than shipping a red tree or inlining validation into link.rs
- **Open:** User decision: proceed to Phase 2 (delete cue link + STORE machinery + rework switch STORE tests) which now also carries the store.rs STORE removal as its first commit?

## [e798914] Phase 2 slice: cue link command removed

Committed c26eea0 on feat/worktree-store-cue-task. Removed the Link CLI variant and dispatch/module wiring, deleted the implementation and STORE-proxy integration suites, and removed the obsolete proxy-only switch test. The remaining real-worktree switch behavior will be added after switch migrates to store::open, where it can pass against the new mechanism.

- **Found:** Workspace tests passed after command removal; cue tests and clippy -D warnings passed after pruning obsolete test imports
- **Found:** Workspace cargo fmt --check remains blocked by pre-existing rustfmt style-edition drift outside this slice
- **Decided:** Land command removal as an independently green slice while retaining cuelib STORE compatibility until all CLI call sites migrate to store::open
- **Decided:** Replace proxy switch coverage with real-worktree coverage during the switch migration rather than preserving an impossible cue link fixture

## [e798914] Phase 2-3 complete: drop STORE machinery and migrate CLI to store::open

Migrated all CLI call sites across cue (commands: add, config, context, list, log, status, switch) to store::open and store::git_root. Removed cuelib::store::resolve_store, validate_store_target, and the legacy STORE redirect test suite. Replaced redundant head_dir.exists guards across all command handlers. Added real-worktree switch integration test verifying local worktree HEAD creation without store leakage. Updated test fixtures to require master/ store directory. Workspace test suite and clippy are clean.

- **Found:** All ~15 CLI call sites successfully migrated to use store::open(root, config) and store::git_root(root)
- **Found:** Clippy and workspace cargo test are fully green across all 250+ tests
- **Decided:** Remove cuelib STORE-following machinery now that all call sites are migrated to store::open
- **Decided:** Update test fixtures in context_render, context_show, and log tests to initialize master/ store directory

## [e798914] Phase 4 complete: $CUE_TASK scope resolution rung

Implemented $CUE_TASK scope resolution rung in cuelib::head with ScopeProvenance and ResolvedScope structures. Precedence chain is strictly enforced: explicit flag > $CUE_TASK > local .cue/HEAD > default master. Routed add, list, log, and context commands through cuelib::head::resolve_scope. Added unit tests for all precedence scenarios and odd values in cuelib::head, and integration tests in tests/task_env.rs. Workspace tests and clippy are clean.

- **Found:** $CUE_TASK unvalidated passthrough matches existing .cue/HEAD semantics per spec
- **Found:** Integration tests verify $CUE_TASK routing across add, list, and log commands
- **Decided:** Expose ScopeProvenance (flag, env, head, default) and ResolvedScope (with slug and provenance) from cuelib::head
- **Decided:** Support Deref<Target=str>, AsRef<str>, AsRef<Path>, and Display on ResolvedScope for ergonomic usage across CLI call sites

## [e798914] Phase 5 complete: switch guard relaxation, $CUE_TASK warning, and worktree verification

Phase 5 complete on feat/worktree-store-cue-task (commit 4cd55b8). `cue switch` now warns on stderr (while preserving stdout/--json) when $CUE_TASK is set to alert the user that switch writes local HEAD while $CUE_TASK takes precedence. Integration tests verify switch in fresh worktrees restores branch task associations, materializes local .cue/HEAD, and respects $CUE_TASK warnings. All workspace tests and clippy are clean.

- **Found:** switch already had head_dir.exists guard removed in Phase 3 migration
- **Found:** Running tests in nix develop environment satisfies all dependencies and clippy checks
- **Decided:** Emit warning to stderr only when $CUE_TASK is non-empty, preserving JSON output formatting on stdout
- **Decided:** Assert switch in fresh worktree succeeds and materializes local .cue/HEAD

## [e798914] Phase 6 complete: status and context observability with provenance

Phase 6 complete on feat/worktree-store-cue-task (commit cf1d92e). Added --task flag to `cue context render` and `cue context show` (absorbing cue-context-task-flag). Updated `cue status` and `cue context` to respect the full precedence chain (--task > $CUE_TASK > .cue/HEAD > master). Status human output includes provenance labels ((flag)/(env)/(head)/(default)) and resolved store directory; status --json includes structured `provenance` and `store` fields. Added tests covering all four provenance sources across status and context commands. Full test suite and clippy clean.

- **Found:** Status and context commands now uniformly resolve scope via cuelib::head::resolve_scope
- **Found:** Worktree store path resolution correctly surfaced to status outputs
- **Decided:** Add optional --task <slug> flag to `cue context render` and `cue context show`, routing through gather_context and handle_show
- **Decided:** Add optional --task <slug> flag to `cue status` to allow direct scope inspection
- **Decided:** Include provenance labels and store path in status human output and JSON output

## [e798914] Recovered interrupted Phase 7 work and identified blockers

Inspected the dirty feature worktree after interruption. The uncommitted Phase 7 slice modifies cue init and its integration tests. Targeted init tests, the full workspace test suite, and workspace clippy all pass. The slice is not commit-ready because the implementation accepts any existing store path rather than requiring a valid store with master/, and the linked-worktree early exit unnecessarily rewrites the project registry, allowing unrelated registry errors to violate the required exit-0 behavior. Workspace fmt check remains red from known toolchain-wide style-edition drift; the two touched files also have local rustfmt deltas.

- **Found:** Dirty changes are confined to crates/cue/src/commands/init.rs and crates/cue/tests/init.rs
- **Found:** The real-worktree happy-path and absent-store tests pass; full workspace tests and clippy -D warnings are green
- **Found:** crates/cue/src/commands/init.rs checks store_path.exists() instead of validating master/ via store::open
- **Found:** The early-exit path mutates ProjectStore before returning, introducing an unnecessary failure mode
- **Found:** cargo fmt --all --check reports broad pre-existing style-edition drift plus formatting deltas in both touched files
- **Decided:** Do not commit the recovered Phase 7 slice in its current state
- **Decided:** Remove a temporary uncommitted.diff artifact created by a review subagent; it was not present in the recovered worktree
- **Open:** Fix Phase 7 by resolving/validating the existing store with store::open and returning immediately after printing
- **Open:** Add coverage for an invalid-but-present root store and, if useful, prove the early-exit path is independent of registry state

## [e798914] Phase 7 complete: cue init resolves the git-root store

Committed 20159bf on feat/worktree-store-cue-task. `cue init` now loads configuration from the store-owning main git root. From a linked worktree it validates the existing root store through `store::open`, prints its location, and exits without creating a local store or touching the project registry. Real-worktree integration coverage includes the happy path, an absent store, and an incomplete store lacking master/.

- **Found:** The interrupted implementation's `Path::exists` check accepted incomplete stores and was replaced by the existing `store::open` validity contract
- **Found:** A malformed projects registry no longer affects the linked-worktree early-exit path
- **Found:** Targeted init tests pass 12/12; full workspace tests and clippy -D warnings are green
- **Found:** The two touched files pass rustfmt 2024 checking; workspace-wide fmt remains affected by previously logged style-edition drift
- **Decided:** Treat finding an existing valid root store as a side-effect-free early exit
- **Decided:** Reuse store::open error text and master/ validation rather than maintaining a separate init-specific check
- **Open:** Proceed to Phase 8 documentation rollout across cue, cue.nvim, cue-plugins, and README

## [e798914] Core docs explain shared stores and scope precedence

Committed c9a7039 on feat/worktree-store-cue-task. README and cue documentation now describe the main-git-root artifact store, worktree-local HEAD, and the --task > CUE_TASK > HEAD > master precedence chain. CLI task-option help now states that explicit task flags override the environment and HEAD.

- **Found:** Core cue tests and clippy -D warnings remain green after the help-text changes
- **Found:** The cue.nvim checkout is mounted read-only in this session, so its two planned comment refreshes cannot be applied here
- **Found:** cue-plugins is on an unrelated dirty feature branch with pre-existing skill edits, so its validated Phase 8 edits cannot be safely committed without operator direction
- **Decided:** Commit the self-contained core repository documentation slice independently
- **Decided:** Do not alter the valid cue.nvim branch-association comment; only stale HEAD-only scope comments require refresh
- **Open:** Decide how to land cue-plugins documentation changes without mixing them into feat/castagent-task-tool
- **Open:** Apply two cue.nvim comment-only changes from a writable checkout

## [e798914] cue-plugins docs landed on isolated branch

Committed ea5b968 in cue-plugins on isolated branch feat/worktree-store-cue-task-docs, based on master. The distributed cue skill now documents the shared main-git-root store, worktree-local HEAD, full scope precedence, and CUE_TASK child propagation. cue tool argument descriptions state that explicit task scope overrides CUE_TASK and HEAD. The original dirty feat/castagent-task-tool checkout was restored to its exact pre-existing state before creating the isolated worktree.

- **Found:** cue-plugins TypeScript typecheck passes in its Nix devshell
- **Found:** The existing cue-plugins checkout had unrelated user changes, so a separate worktree avoided mixing histories
- **Decided:** Keep cross-repository Phase 8 documentation in a dedicated cue-plugins branch
- **Decided:** Preserve the operator's existing feat/castagent-task-tool changes untouched
- **Open:** cue.nvim comment refresh remains blocked because the repository mount is read-only in this session

## [e798914] Final smoke passes; validation rerun hit sandbox resource ceiling

Ran the feature binary from the real feat/worktree-store-cue-task linked worktree with CUE_TASK pinned. `cue status --json` reported env provenance and the main checkout store `/home/pl/code/palekiwi-labs/cue/.cue`; `cue list` read the active task plan from that shared store. Shipped-surface sweeps found no legacy STORE or cue link references. A fresh final workspace test rerun could not complete because rustc/lld failed to spawn helper threads with OS error 11 (resource temporarily unavailable), despite the complete workspace suite and clippy having passed earlier in this session after Phase 7. Workspace fmt remains red from pre-existing broad style drift.

- **Found:** Real-worktree smoke confirms CUE_TASK env provenance and shared main-root store resolution
- **Found:** No legacy STORE/cue link references remain in Rust, Markdown, TOML, Nix, or JSON shipped surfaces
- **Found:** Final rerun failure is an environment resource limit/LLVM thread-spawn failure, not a test assertion or compile diagnostic
- **Found:** cargo fmt --all --check still proposes broad unrelated formatting across acuity, curator, and cue files
- **Decided:** Do not introduce a repository-wide formatting rewrite into this feature
- **Decided:** Treat the earlier green full workspace suite and clippy plus the later green cue-specific suite/clippy as the current verification evidence
- **Open:** Retry final workspace test/clippy when sandbox process resources recover
- **Open:** Apply cue.nvim comment refresh from a writable checkout

## [e798914] cue.nvim scope comments landed in writable clone

Committed ef5f2df on feat/worktree-store-cue-task-docs in a writable temporary clone because the mounted cue.nvim checkout remains read-only. Updated core and picker comments to describe resolved scope precedence rather than HEAD-only behavior. All standalone Lua tests passed and luacheck is clean for both touched files.

- **Found:** The mounted /home/pl/code/palekiwi-labs/cue.nvim filesystem remains read-only, including creation of a repository-local worktree
- **Found:** Repository-wide stylua checking reports broad pre-existing formatting differences unrelated to these comment-only edits
- **Found:** A pre-existing luacheck line-length warning remains in tests/test_list_scopes.lua:38; both touched files lint clean
- **Decided:** Keep the cue.nvim changes isolated on feat/worktree-store-cue-task-docs at commit ef5f2df
- **Decided:** Do not apply repository-wide Stylua rewrites as part of this documentation slice
- **Open:** The temporary clone commit must be transferred or pushed from a writable persistent cue.nvim checkout by the operator

## [e798914] Implementation complete with environmental validation caveat

Completed all planned implementation and documentation phases. Core feature branch is clean at c9a7039. cue.nvim comment refresh is committed as ef5f2df on feat/worktree-store-cue-task-docs in a writable temporary clone; cue-plugins documentation was previously committed separately. The final constrained workspace retry compiled all crates successfully, then integration tests exhausted the sandbox process/thread quota (OS error 11); clippy could not start because the test command failed first. Earlier complete workspace test and clippy runs remain green. No behavioral deviation from the approved specification was identified.

- **Found:** CARGO_BUILD_JOBS=1 and single-threaded lld allowed the entire Rust workspace to compile
- **Found:** Integration tests still exhausted the sandbox quota because test binaries spawn threads and git subprocesses; failures uniformly reported Resource temporarily unavailable rather than assertions
- **Found:** The formatting issue refers to the Cargo workspace in this repository, not the operator's other Git worktrees or repositories
- **Found:** cargo fmt with rustfmt 1.96 proposes broad style-edition changes in untouched acuity, curator, cuelib, and cue files
- **Decided:** Mark the task and master plan complete based on prior green full tests/clippy, successful real-worktree smoke, and the latest successful full compile
- **Decided:** Exclude repository-wide formatter churn from this feature; handle it later as a dedicated formatting/toolchain-alignment change if desired
- **Decided:** Report no spec deviations; retain only validation-environment and cross-repository transfer caveats
- **Open:** Transfer cue.nvim commit ef5f2df from the temporary writable clone into a persistent cue.nvim branch
- **Open:** Optionally create a separate maintenance task to align the repository's rustfmt version/style edition and reformat the Cargo workspace

