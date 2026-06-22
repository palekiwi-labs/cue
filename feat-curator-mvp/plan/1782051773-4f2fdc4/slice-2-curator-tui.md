---
status: closed
---
# Slice 2 — curator TUI

## Foreword

This slice builds the `curator` binary: a minimal Ratatui TUI that reads
tasks from the CWD project's `.cue/master/task/` directory via `cuelib` and
renders them as a three-column kanban board (Open | In Progress | Complete).

**Branch**: `feat/curator-mvp`
**Implements**: Phase B of `plan/index.md`
**Requires**: Slice 1 complete (`cuelib` has `read_artifacts`)

Board columns map directly to `TaskStatus::is_kanban_visible()` values.
Navigation is keyboard-driven: `h`/`l` to switch column focus, `j`/`k`
to scroll within a column, `q` to quit.

---

## Steps

- [ ] Add dependencies to `crates/curator/Cargo.toml`:
      `ratatui`, `crossterm`, `clap` (derive), `anyhow`
- [ ] Create `crates/curator/src/app.rs`:
      `App { columns: [Vec<ArtifactMeta>; 3], active_col: usize,
             scroll: [usize; 3], should_quit: bool }`
      with `App::load(root: &Path) -> Result<Self>` that calls
      `cuelib::artifact::read_artifacts` and partitions by status
- [ ] Create `crates/curator/src/tui.rs`:
      `setup_terminal()` / `restore_terminal()` helpers
      (crossterm raw mode + alternate screen)
- [ ] Create `crates/curator/src/event.rs`:
      blocking crossterm event reader; maps key events to an `Action` enum
      (`Quit`, `Left`, `Right`, `Up`, `Down`)
- [ ] Create `crates/curator/src/ui.rs`:
      `render(app: &App, frame: &mut Frame)` — three equal-width `Block`
      panels, each containing a list of task titles; active column has a
      highlighted border
- [ ] Wire everything in `crates/curator/src/main.rs`:
      parse CWD (or `--path` arg via clap), call `App::load`, run event loop
- [ ] `cargo build -p curator` succeeds
- [ ] `cargo test -p cue` still green (regression check)
- [ ] Smoke-test: run `curator` in this repo, verify board renders real tasks
- [ ] Commit: `feat(curator): minimal kanban TUI reading cuelib artifacts`
