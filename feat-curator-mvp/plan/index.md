---
status: complete
---
# Master Plan — curator MVP (Phase 2)

## Problem & Constraints

`curator` must display a kanban board of cue artifacts from the CWD project.
`cuelib` currently has no ability to read artifact files from disk — frontmatter
parsing and directory walking live in the `cue` binary crate.

Scope: CWD only, tasks only for MVP columns, no `acuity`.

## Chosen Architecture

### Stream 1 — `cuelib` artifact reader

Move the two utility functions from `cue` into `cuelib` and build a typed
reader on top:

```
cuelib::artifact
  extract_frontmatter_yaml(path) -> Option<String>   (migrated from cue/list)
  collect_files(dir)             -> Result<Vec<PathBuf>>  (migrated)
  ArtifactMeta { title, status_raw, priority_raw, artifact_type, path }
  read_artifacts(root, branch, artifact_type) -> Result<Vec<ArtifactMeta>>
```

`serde_yaml = "0.9"` added to `cuelib/Cargo.toml`.

The `cue` binary crate re-exports or calls back into `cuelib` — no
duplication.

### Stream 2 — `curator` TUI

```
curator
  main.rs          clap CLI, detects TTY, launches TUI
  app.rs           App struct (board state, should_quit)
  tui.rs           terminal setup/teardown helpers
  ui.rs            root render fn — three-column layout
  event.rs         crossterm event loop
```

Ratatui + crossterm added to `curator/Cargo.toml`.

Board layout:

```
+------------------+------------------+------------------+
|      Open        |   In Progress    |     Complete     |
+------------------+------------------+------------------+
|  Task title      |  Task title      |  Task title      |
|  [priority]      |  [priority]      |  [priority]      |
|  ...             |  ...             |  ...             |
+------------------+------------------+------------------+
```

Navigation: `q` to quit, `j`/`k` (or arrow keys) to scroll within a column,
`h`/`l` (or arrow keys) to switch active column.

## Implementation Phases

### Phase A — `cuelib` extension (Slice 1)

1. Add `serde_yaml` to `cuelib/Cargo.toml`
2. Migrate `extract_frontmatter_yaml` + `collect_files` into `cuelib/src/artifact.rs`
3. Add `ArtifactMeta` struct and `read_artifacts` function
4. Update `cue` crate to call `cuelib` versions (remove duplication)
5. All `cuelib` + `cue` tests green

### Phase B — `curator` TUI (Slice 2)

1. Add `ratatui`, `crossterm`, `clap`, `anyhow` to `curator/Cargo.toml`
2. Scaffold `app.rs`, `tui.rs`, `event.rs`, `ui.rs`
3. Load tasks via `cuelib::artifact::read_artifacts`
4. Render three-column board
5. Keyboard navigation + `q` to quit
6. Smoke-test by running `curator` in this repo

## Key Design Decisions

- **Migrate, don't duplicate**: `extract_frontmatter_yaml` and `collect_files`
  move to `cuelib` so both crates share one implementation.
- **`ArtifactMeta` is stringly-typed for status/priority**: avoids coupling
  `cuelib`'s reader to the exact enum variants; callers coerce via `FromStr`.
- **CWD-only for MVP**: `ProjectStore` consulted in a later phase when
  multi-project view is needed.
- **Tasks only in initial columns**: plans and todos are read but displayed
  under a separate section or deferred; the three kanban columns show tasks.
