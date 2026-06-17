# Plan: Curtain Phase 1 (revised)

TDD throughout: every slice begins with failing tests, then implementation to go green.

---

## Workspace target layout

```
cue/                     (workspace root)
  Cargo.toml             (workspace manifest)
  cue/                   (binary crate)
    Cargo.toml
    src/
    tests/
  cuelib/                (library crate)
    Cargo.toml
    src/
      lib.rs
      config.rs
      git.rs
      artifact.rs        (was list/mod.rs)
      project.rs         (new)
      status.rs          (new)
  curtain/               (binary crate)
    Cargo.toml
    src/
      main.rs
      app.rs
      loader.rs
      ui/
```

---

## Slice 0 — Cargo workspace restructure

No new tests. Purely mechanical: move `src/`, `tests/`, `Cargo.toml` into `cue/`; create workspace root `Cargo.toml` with `members = ["cue", "cuelib", "curtain"]`; create skeleton crates. Gate: `cargo test -p cue` stays green.

---

## Slice 1 — `cuelib`: status vocabulary + artifact types

**Red first.** Tests in `cuelib/src/status.rs`:
- `default_artifact_types_contains_expected_set` — the 8 canonical types
- `default_ignored_types` — `["tmp", "bin"]`
- `kanban_columns` — `["open", "in-progress", "complete"]`
- all five status constants (`open`, `in-progress`, `complete`, `closed`, `archived`)

Test in `cuelib/src/config.rs`:
- `default_artifact_types_matches_canonical_set`

**Green.** Implement `status.rs` constants; update `Config::default()` to use them; update the existing `test_default_artifact_types` / `test_default_ignored_types` tests; update the `tests/init.rs` gitignore assertion (`bin` is now also ignored).

---

## Slice 2 — `cuelib`: extract shared modules

Move `config.rs`, `git.rs`, and `list/mod.rs` (→ `artifact.rs`) from the `cue` binary into `cuelib`. Wire `cue` to depend on `cuelib`; replace all `use crate::` with `use cuelib::`. Gate: `cargo test -p cue && cargo test -p cuelib` all green.

---

## Slice 3 — `cuelib`: project registry

**Design decisions:**
- A key maps to `Vec<PathBuf>` — supports the same repo checked out in multiple locations or as git worktrees.
- `CUE_DATA_DIR` env var controls the store path, for test isolation (mirrors existing `CUE_CONFIG_DIR` pattern).
- `add` is idempotent: same path twice is a no-op, returns `false`.
- Removing the last path for a key removes the key entirely.

**Red.** Unit tests in `cuelib/src/project.rs`:
- `load_missing_file_returns_empty_store`
- `add_new_key_creates_entry`
- `add_second_path_appends`
- `add_duplicate_path_is_noop` — returns `false`, length unchanged
- `remove_path_removes_from_vec`
- `remove_last_path_removes_key`
- `save_and_load_round_trip` — two paths survive serialization
- `derive_key_from_github_https_remote` — `https://github.com/org/repo.git` → `github:org/repo`
- `derive_key_from_github_ssh_remote` — `git@github.com:org/repo.git` → `github:org/repo`
- `derive_key_no_remote_falls_back_to_local` — key = `local:<dir-name>`

**Store format** (`~/.local/share/cue/projects.json`):
```json
{
  "github:org/repo": ["/path/to/main-checkout", "/path/to/worktree"]
}
```

---

## Slice 4 — `cue project` subcommand

**Red.** Integration tests in `cue/tests/project.rs`. Add helper `cue_cmd_with_data_dir(data_dir)` following the existing `cue_cmd()` pattern:
- `project_list_empty`
- `project_add_registers_path`
- `project_add_then_list_shows_entry`
- `project_add_idempotent` — adding same path twice shows it once
- `project_remove_by_path`
- `project_remove_missing_path_is_not_an_error`

**Green.** New `Commands::Project` variant in `cli.rs`:
```
Project::Add    { path: Option<PathBuf> }   // defaults to cwd
Project::Remove { path: Option<PathBuf> }   // defaults to cwd
Project::List
```
New `cue/src/commands/project.rs`.

---

## Slice 5 — `cue init` registers the project

**Red.** Extend `cue/tests/init.rs` (both set `CUE_DATA_DIR`):
- `init_registers_project_in_store`
- `init_twice_does_not_duplicate_entry`

**Green.** Append to end of `commands/init.rs::handle`, after the existing worktree setup:
```rust
let key = cuelib::project::derive_project_key(&root)?;
let mut store = cuelib::project::ProjectStore::load()?;
store.add(&key, &root);
store.save()?;
```

---

## Slice 6 — `curtain`: data loading

The data model is fully testable without a terminal.

**Red.** Unit tests in `curtain/src/loader.rs`:
- `load_todos_from_single_project`
- `todo_item_carries_project_key_and_branch`
- `group_by_status_distributes_correctly`
- `items_with_non_kanban_status_excluded_from_grouped_map`
- `filter_by_project_key_returns_subset`
- `todo_without_title_frontmatter_falls_back_to_filename`

`TodoItem`:
```rust
pub struct TodoItem {
    pub title:       String,
    pub status:      String,
    pub priority:    Option<u8>,
    pub project_key: String,
    pub branch:      String,
    pub path:        PathBuf,
}
```

**Green.** Implement `load_todos` (calls `cuelib::artifact::list` with `cue_type = Some("todo")`) and `group_by_kanban_status`.

---

## Slice 7 — `curtain`: App state logic

The column/filter logic is pure — also testable without a terminal.

**Red.** Unit tests on `App`:
- `cycle_project_filter_rotates_through_keys_and_back_to_all`
- `visible_todos_for_col_respects_active_filter`

**Green.** Implement `App::cycle_project_filter`, `App::visible_todos_for_col`.

---

## Slice 8 — `curtain`: TUI rendering

No unit tests for the ratatui layer. Goal: it compiles and runs end-to-end.

**Card layout:**
```
┌──────────────────────────────┐
│ Fix login bug                │
│ github:org/repo  @main       │
└──────────────────────────────┘
```

**Key bindings:** `h/l` columns, `j/k` rows, `Tab` cycle project filter, `e` open in `$EDITOR`, `q`/`Esc` quit.

**CLI:**
```
curtain [--all | --path <path>] [--type <type>]
```
