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

