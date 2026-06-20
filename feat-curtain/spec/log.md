# Project Log

## [173152e-dirty] Spec rewritten for cueban feature branch

Rewrote .cue/feat-cueban/spec/index.md to reflect the full design discussion. Previous spec was a rough initial idea; new spec is authoritative and covers all settled decisions.

- **Decided:** Cargo workspace with three crates: cue, cue-lib, cueban
- **Decided:** cue-lib is the shared library — config, git, artifact discovery, project registry
- **Decided:** Canonical artifact types: spec plan trace doc todo bin tmp ref (8 types)
- **Decided:** Default ignored types: tmp bin
- **Decided:** Canonical todo statuses: open in-progress complete closed archived; only first three shown in kanban
- **Decided:** Project registry at ~/.local/share/cue/projects.json, key maps to Vec<PathBuf> to support multiple checkouts and worktrees
- **Decided:** Project keys: github:org/repo for GitHub remotes, local:<dir-name> fallback
- **Decided:** CUE_DATA_DIR env var controls store path for test isolation
- **Decided:** cue init registers project in store (idempotent)
- **Decided:** cue project add/remove/remove --key/list subcommands
- **Decided:** archived/closed todos silently hidden in cueban (no clutter)
- **Decided:** cue project remove by path (cwd default); --key removes all paths for a key
- **Decided:** BTreeMap for project store (no extra dep, alphabetical order fine)
- **Decided:** cueban cyclic project filter via Tab: All -> key-A -> key-B -> All
- **Decided:** Card shows title on line 1, project-key and branch on line 2
- **Decided:** cueban --type flag kept as forward-compat hook (default: todo)
- **Decided:** TDD throughout: 8 implementation slices, each starts with failing tests

## [173152e-dirty] Workspace member renames (curtain, cuelib, acuity)

Renamed workspace members across all spec and plan artifacts:
- cue-lib -> cuelib
- cueban -> curtain
- Added acuity as the name for the future observability hub.
Updated all Rust namespace references (e.g. cuelib::artifact) and CLI command names accordingly.

- **Decided:** Crate name: cuelib (no hyphen)
- **Decided:** TUI name: curtain
- **Decided:** Observability hub name: acuity

## [9db6632-dirty] Slice 0+1: Workspace conversion and cuelib artifact module

Committed 9db6632. Converted the single-crate layout to a Cargo workspace. The cue CLI moved to cue/ subdirectory; cuelib/ is a new sibling crate. cuelib::artifact implements CANONICAL_TYPES, DEFAULT_IGNORED_TYPES, and TodoStatus with FromStr trait. All 120 tests pass.

- **Decided:** Root Cargo.toml is workspace manifest; members: cue, cuelib (curtain added later in Slice 6)
- **Decided:** TodoStatus implements std::str::FromStr rather than a custom from_str to satisfy clippy::should_implement_trait
- **Decided:** cuelib/src/project.rs is a placeholder in Slice 0; implemented in Slice 3

## [0e2b035-dirty] Slice 2: config and git modules extracted to cuelib

Committed 0e2b035. cue/src/config.rs and cue/src/git.rs are now thin re-export shims. The full implementations live in cuelib::config and cuelib::git. cue depends on cuelib via path dependency. Config tests migrated to cuelib. All 120 tests pass.

- **Decided:** cue re-exports via `pub use cuelib::config::*` and `pub use cuelib::git::*` to avoid breaking existing internal crate references
- **Decided:** get_remote_url added to cuelib::git to support project key derivation in Slice 3

## [17cefb8] Slice 3: project registry in cuelib

Committed 17cefb8. ProjectStore with BTreeMap backing, JSON persistence, CUE_DATA_DIR isolation. derive_project_key parses GitHub SSH/HTTPS remotes, falls back to local:dirname. 14 new tests, 139 total passing.

- **Decided:** store_path() uses CUE_DATA_DIR env var for test isolation (same pattern as CUE_CONFIG_DIR)
- **Decided:** let-chains (&&) used to satisfy clippy::collapsible_if
- **Decided:** BTreeMap chosen over HashMap for deterministic alphabetical JSON output

## [3e2d075] Slice 4: cue project subcommands

Committed 3e2d075. Added `cue project add/remove/list` subcommands. `add` is idempotent, `remove` accepts either `--path` (default cwd) or `--key` (removes all paths for that key). 7 integration tests added; 146 tests total passing. Fork-exhaustion during test run was a transient OS resource issue, not a code defect.

- **Decided:** project add defaults to cwd; explicit --path accepted for non-interactive use
- **Decided:** remove by key prints 'Key not found' rather than erroring, consistent with idempotent add
- **Decided:** list output format: 'key  path' (two spaces) for easy parsing

## [3e2d075] Research complete: kanban reference project analysis

- **Found:** Monolithic App struct with typed sub-state fields (SelectionHub, FilterState, FocusState, etc.) — all using Default::default()
- **Found:** AppMode enum + mode_stack Vec pattern — push_mode/pop_mode enables dialogs layering on top of any base mode
- **Found:** Event loop: tokio::select! multiplexing crossterm events + async save results + file watcher + export results under a single needs_redraw flag
- **Found:** Two-phase rendering: Phase 1 = base view, Phase 2 = overlays using Clear widget for z-ordering; render fns borrow &App
- **Found:** RenderStrategy trait (Box<dyn RenderStrategy> on App) for switching between Flat / Multi-panel / Grouped board layouts
- **Found:** Centralised theme/ module with named fn helpers (focused_border(), priority_style(), etc.) — no inline styles
- **Found:** Command enum + KanbanContext::execute() for all mutations; undo via UndoEntry { forward, inverse } with pre-capture
- **Found:** PersistenceStore trait with content-sniffing backend auto-detection (SQLite magic bytes vs JSON leading brace)
- **Found:** Background save worker via tokio::mpsc channel; JSON backend marks dirty + flushes on signal, SQLite commits immediately
- **Found:** Dual error UX: Banner (transient, hooks into tracing) + Error Log accessible via F12 (InMemoryLogLayer)
- **Found:** clap derive macros; TUI launches when no subcommand + stdin.is_terminal(); config loaded from TOML before TUI start
- **Found:** Strict workspace layering: domain <- persistence traits <- service <- tui <- cli; TUI never touches persistence directly

## [2de5bc1] Slice 4a + Slice 5: task type, TaskStatus, cue init registration

Two commits on feat/curtain (383cd0a, 2de5bc1).

Slice 4a: added 'task' as the ninth canonical artifact type in cuelib::artifact::CANONICAL_TYPES. Added TaskStatus enum (Open, InProgress, Complete, Closed) with FromStr, as_str(), is_kanban_visible(). Tasks do not use 'archived' — that status is todo-only. Four new tests added to cuelib.

Slice 5: cue init now unconditionally calls ProjectStore::add_path + save after the worktree setup step. The call is idempotent because add_path is a no-op for already-present paths. Two new integration tests confirm registration on first init and no-duplication on second init.

Full workspace: 158 tests, all green. Clippy clean.

- **Found:** init::init early-return (already initialized) still propagates Ok(()) through the ? operator, so the registration code in commands/init.rs::handle always runs — no restructuring was needed
- **Decided:** task is the primary kanban artifact type; todo is for deferred/informal notes only
- **Decided:** TaskStatus has no Archived variant — that is reserved for TodoStatus only
- **Decided:** init registration is unconditional (runs even when already-initialized path is returned) — idempotency is handled by add_path
- **Decided:** Slices 6-8 (curtain TUI) are deferred to a separate design plan pending ratatui architecture review

## [7dffc98] fix(cuelib): treat empty projects.json as empty store

Fixed a crash in ProjectStore::load() that occurred when projects.json existed but was empty. Added a unit test to verify that an empty file is treated as an empty store.

- **Found:** ProjectStore::load() panicked with 'EOF while parsing a value' on empty files
- **Decided:** Treat empty projects.json as empty store instead of failing with EOF error

## [1341ad8] fix(tests): add data_dir isolation to TestEnv

Extended TestEnv struct in cue/tests/helpers.rs to include data_dir and set CUE_DATA_DIR in the command() method. This ensures that any test using TestEnv is correctly isolated from the host's project registry. Verified by running context_*.rs tests.

- **Found:** Context tests were leaking to the host's project registry because TestEnv didn't set CUE_DATA_DIR
- **Decided:** Extend TestEnv to handle data_dir isolation instead of using global helpers

## [36549e9] Commit 1: fix empty projects.json crash in ProjectStore::load()

Committed 36549e9. Added empty-file guard in cuelib/src/project.rs:load(). The function already handled a missing file but called serde_json::from_str on an empty string, causing 'EOF while parsing a value'. Guard added after read_to_string: if data.trim().is_empty() return Ok(Self::default()). TDD: wrote failing test first (load_returns_empty_when_file_is_empty), then added guard. 29 cuelib tests green.

- **Decided:** Empty or whitespace-only projects.json is treated identically to a missing file — returns an empty store. Self-healing: next save() will write valid JSON.

## [15acb00] Commit 2: extend TestEnv with data_dir, fix context_*.rs leak

Committed 15acb00. Added data_dir: PathBuf field to TestEnv (subdir data/ of temp_dir). TestEnv::command() now sets CUE_DATA_DIR and also removes CUE_ARTIFACT_TYPES/CUE_IGNORED_TYPES, aligning it with cue_cmd() isolation policy. This closes the leak for 46 call sites in context_init.rs, context_path.rs, context_render.rs, context_show.rs. All 10 context tests pass. cue_cmd() still exists for the remaining migration (Commit 3).

- **Decided:** TestEnv is documented as the single authoritative isolation boundary
- **Decided:** data_dir is temp_dir/data/ subdir — same TempDir owns both, guaranteeing RAII cleanup

## [d404000] Commit 3: refactor(tests): unify on TestEnv, remove cue_cmd()

Completed the final commit of the fix-test-isolation plan. Migrated all integration test files (add.rs, list.rs, log.rs, config_show.rs, init.rs, project.rs) from helpers::cue_cmd() to helpers::TestEnv. Deleted cue_cmd() from helpers.rs. All 159 tests pass, clippy clean. Commits 1 (36549e9) and 2 (15acb00) were already in history; only Commit 3 was needed.

- **Found:** Commits 1 and 2 from the plan (fix empty projects.json, extend TestEnv with data_dir) were already in git history (36549e9, 15acb00) but the working tree had an older version of helpers.rs — restored with git checkout HEAD
- **Found:** list.rs contained 1303 lines with 84+ cue_cmd() call sites — the largest migration file
- **Decided:** All integration tests now use TestEnv as the single authoritative isolation boundary — config_dir, data_dir, and env-var removal (CUE_ARTIFACT_TYPES, CUE_IGNORED_TYPES) are all handled in TestEnv::command()
- **Decided:** setup_filter_repo() helper in list.rs was updated to accept &TestEnv instead of &TempDir
- **Decided:** cue_cmd() deleted; no remaining call sites

## [d404000] Refactor: Move crates to crates/ directory

- **Decided:** Move workspace crates to crates/ subdirectory for better scalability and cleanliness.

## [4efa490] Refactor: Crates moved to crates/ directory completed

- **Found:** Relative path dependencies between crates were preserved as expected.
- **Decided:** Workspace structure transitioned to crates/ pattern.

## [ae59d67] Remove Archived status from TodoStatus

Removed the `Archived` status from `TodoStatus` to make it symmetric with `TaskStatus`. Following Opus's advice, we've decided that `archived` is an orthogonal visibility flag rather than a lifecycle status. Since the curtain TUI doesn't render todos yet, the clutter problem is not yet observable, and `Closed` already serves the purpose of hiding artifacts from the board.

- Removed `TodoStatus::Archived` and updated methods.
- Updated unit tests to verify `archived` is now an invalid status for both tasks and todos.
- Updated spec to remove mention of `archived`.
- Closed the related todo artifact.

- **Found:** TodoStatus and TaskStatus were asymmetric because of the Archived variant
- **Found:** Archived was behaviorally identical to Closed (hidden from kanban)
- **Decided:** Remove Archived as a status value for all artifacts
- **Decided:** Treat archived as an orthogonal visibility flag if needed in the future
- **Decided:** Maintain symmetry between TaskStatus and TodoStatus lifecycle states

