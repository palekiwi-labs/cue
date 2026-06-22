---
status: complete
---
# Slice 2 — curator TUI

## Foreword

This plan covers Phase B (Slice 2) of the curator MVP master plan. Slice 1
(cuelib artifact reader) is complete as of commit 8402d2b. This slice builds
the Ratatui TUI binary in `crates/curator/`.

Prerequisites:
- `cuelib::artifact::read_artifacts` and `ArtifactMeta` are available and tested.
- `TaskStatus` has `FromStr` impl and `is_kanban_visible()` method.
- `curator/src/main.rs` is an empty stub — clean slate.

## Steps

- [x] Add `ratatui`, `crossterm`, `clap`, `anyhow` to `curator/Cargo.toml`
- [x] Scaffold `app.rs` — `App` struct: board columns, selected column, scroll offsets, `should_quit`
- [x] Scaffold `tui.rs` — terminal setup (raw mode, alternate screen) and teardown
- [x] Scaffold `event.rs` — crossterm event polling loop returning typed `Action` enum
- [x] Scaffold `ui.rs` — root render function: three-column layout with block borders
- [x] Wire `main.rs`: load tasks via `cuelib`, build `App`, start event loop
- [x] Keyboard navigation: `q` quit; `j`/down scroll down; `k`/up scroll up; `h`/left switch left; `l`/right switch right
- [x] Arrow key support (Left/Right/Up/Down) alongside `h`/`j`/`k`/`l`
- [x] Render task cards: title + optional [priority] badge
- [x] clippy clean, cargo build passes, cargo test -p cue green
- [x] commit: feat: add curator TUI kanban board
