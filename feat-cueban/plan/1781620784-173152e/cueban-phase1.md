---
status: open
---
# Plan: Cueban Phase 1

## Goal

Build a Cargo workspace with three crates:
- `cue-lib`: shared library (config, git, artifact discovery, project registry)
- `cue`: CLI binary (refactored to use `cue-lib`, gains `project` subcommand)
- `cueban`: ratatui TUI kanban dashboard for `todo` artifacts

---

## Step 1: Convert to Cargo Workspace

**Files changed:**
- Root `Cargo.toml` becomes workspace manifest
- Move current crate into `cue/` subdirectory
- Create `cue-lib/` and `cueban/` crate skeletons

Root `Cargo.toml`:
```toml
[workspace]
members = ["cue", "cue-lib", "cueban"]
resolver = "2"

[workspace.dependencies]
anyhow = "1.0"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
dirs = "6"
```

`cue/Cargo.toml` — binary, depends on `cue-lib`
`cue-lib/Cargo.toml` — lib, no binary deps
`cueban/Cargo.toml` — binary, depends on `cue-lib` + `ratatui` + `crossterm`

---

## Step 2: Extract `cue-lib`

Move the following from `cue/src/` into `cue-lib/src/`:

| Source file | Target module | Notes |
|---|---|---|
| `config.rs` | `cue_lib::config` | Unchanged |
| `git.rs` | `cue_lib::git` | Unchanged |
| `list/mod.rs` | `cue_lib::artifact` | Rename module; drop command-level types |
| (new) | `cue_lib::project` | Project registry |
| (new) | `cue_lib::status` | Canonical status/type enums |

Public API of `cue-lib`:
```
cue_lib::config::{Config, ContextProfile, ContextConfig}
cue_lib::git::{run_git, get_git_root, get_current_branch, ...}
cue_lib::artifact::{CueFile, Filter, list, collect_files, extract_frontmatter_yaml, ...}
cue_lib::project::{ProjectStore, ProjectKey}
cue_lib::status::{ArtifactStatus, DEFAULT_ARTIFACT_TYPES}
```

---

## Step 3: Standardize Artifact Types and Status Vocabulary

### `cue_lib::status`

```rust
pub const DEFAULT_ARTIFACT_TYPES: &[&str] = &[
    "spec", "plan", "trace", "doc", "todo", "bin", "tmp", "ref",
];

pub const DEFAULT_IGNORED_TYPES: &[&str] = &["tmp", "bin"];

// Canonical todo statuses (free-form in frontmatter, but these are the
// standard values recognized by cueban's column layout)
pub const STATUS_OPEN: &str = "open";
pub const STATUS_IN_PROGRESS: &str = "in-progress";
pub const STATUS_COMPLETE: &str = "complete";
pub const STATUS_CLOSED: &str = "closed";
pub const STATUS_ARCHIVED: &str = "archived";
```

Update `Config::default()` in `cue_lib::config`:
```rust
artifact_types: DEFAULT_ARTIFACT_TYPES.iter().map(|s| s.to_string()).collect(),
ignored_types: DEFAULT_IGNORED_TYPES.iter().map(|s| s.to_string()).collect(),
```

Update config tests accordingly.

---

## Step 4: Project Registry (`cue_lib::project`)

Path: `~/.local/share/cue/projects.json`

Format:
```json
{
  "github:org/repo": "/abs/path/to/project"
}
```

Key derivation: parse `git remote get-url origin`, normalize to `github:org/repo` form.
Fall back to bare path key if no remote.

API:
```rust
pub struct ProjectStore { inner: HashMap<String, PathBuf> }

impl ProjectStore {
    pub fn load() -> anyhow::Result<Self>;
    pub fn save(&self) -> anyhow::Result<()>;
    pub fn add(&mut self, key: &str, path: &Path) -> bool; // true if new
    pub fn remove(&mut self, key: &str) -> bool;
    pub fn list(&self) -> Vec<(&str, &Path)>;
    pub fn paths(&self) -> Vec<&Path>;
}

pub fn derive_project_key(root: &Path) -> anyhow::Result<String>;
```

---

## Step 5: Update `cue init` to Register Projects

In `cue/src/commands/init.rs`, after the existing worktree setup:
```rust
let key = cue_lib::project::derive_project_key(&root)?;
let mut store = cue_lib::project::ProjectStore::load()?;
store.add(&key, &root);
store.save()?;
```

This is idempotent (add returns false if already present, but save is always called).

---

## Step 6: Add `cue project` Subcommand

In `cue/src/cli.rs`, add to `Commands`:
```rust
/// Manage registered projects
Project {
    #[command(subcommand)]
    command: ProjectCommands,
},
```

```rust
#[derive(Subcommand)]
pub enum ProjectCommands {
    /// Register a project path in the global store
    Add {
        /// Path to the project root (defaults to cwd)
        path: Option<PathBuf>,
    },
    /// Remove a project from the global store
    Remove {
        /// Path or key to remove
        path: Option<PathBuf>,
    },
    /// List all registered projects
    List,
}
```

Add `cue/src/commands/project.rs` handler.

---

## Step 7: Build `cueban` TUI

### Dependencies
```toml
[dependencies]
cue-lib = { path = "../cue-lib" }
ratatui = "0.29"
crossterm = "0.28"
anyhow = "1"
```

### Architecture

```
cueban/src/
  main.rs       -- arg parsing (project mode: current | global), terminal setup
  app.rs        -- App state machine
  ui/
    mod.rs
    kanban.rs   -- kanban board widget
    card.rs     -- todo card widget
```

### App state
```rust
pub struct App {
    pub todos: Vec<TodoItem>,        // all loaded todos
    pub selected_col: usize,         // 0=open, 1=in-progress, 2=complete
    pub selected_row: usize,
    pub mode: ViewMode,              // CurrentProject | AllProjects
}

pub enum ViewMode { CurrentProject, AllProjects }

pub struct TodoItem {
    pub title: String,
    pub status: String,
    pub priority: Option<String>,
    pub path: PathBuf,
    pub project_key: String,
}
```

### Kanban columns
Three fixed columns derived from `cue_lib::status` constants:
- `open`
- `in-progress`
- `complete`

Artifacts with any other status value (e.g. `archived`, `closed`) appear in a collapsed
"Other" section or are hidden (decide during implementation).

### Data loading
```rust
fn load_todos(projects: &[&Path], config: &Config) -> Vec<TodoItem> {
    // For each project path:
    //   list artifacts with cue_type = Some("todo")
    //   parse frontmatter for status, title, priority
    //   build TodoItem
}
```

### Key bindings (Phase 1)
| Key | Action |
|---|---|
| h/l or Left/Right | Move between columns |
| j/k or Up/Down | Move within column |
| Tab | Toggle current/all-projects mode |
| e | Open selected file in $EDITOR |
| q / Esc | Quit |

### $EDITOR integration
```rust
fn open_in_editor(path: &Path) -> anyhow::Result<()> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    std::process::Command::new(&editor).arg(path).status()?;
    Ok(())
}
```

---

## Step 8: Tests

- `cue-lib` unit tests: project store load/save/add/remove, status constants
- `cue` integration tests: `cue project add`, `cue project list`, `cue project remove`
- `cue init` integration test: verify project is registered in store
- `cueban` unit tests: `load_todos` with fixture files, column grouping logic

---

## Dependency Graph

```
cueban ──┐
         ├──> cue-lib
cue ─────┘
```

---

## Phase 2 Note

The observability hub ("cue-pulse" or "cue-scope"?) will be a fourth workspace member.
Its SSE feed would be consumed by `cueban`. No architectural constraints need to be locked
in now beyond: keep `cueban`'s data-loading logic behind a trait so the source
(filesystem vs. SSE) can be swapped.

---

## Open Questions

1. What to name the Phase 2 observability hub? (`cue-hub`, `cue-pulse`, `cue-scope`, `cue-lens`?)
2. Should `archived`/`closed` todos be hidden or shown in a fourth column in `cueban`?
3. Should `cueban` accept a `--path` flag to specify project root, or always use cwd?
