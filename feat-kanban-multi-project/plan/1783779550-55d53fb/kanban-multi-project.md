---
status: complete
refs:
- .cue/master/spec/curator/curator-improvements.md
- .cue/master/task/1782644149-dab157e/projects-view.md
- .cue/master/task/1783779550-55d53fb/kanban-multi-project.md
---
## Foreword

This plan implements the multi-project kanban redesign for `curator` — the
highest-priority UX initiative. It makes the kanban collect task cards from
all registered projects (CWD-independent), renders richer multi-line cards,
and adds an Enter-toggled bottom detail pane. Cross-project *filtering* is
explicitly deferred to a later PR (the existing `projects-view.md` task).

Spec: `spec/curator/curator-improvements.md` (multi-project kanban, formerly
out-of-scope line 80, now in scope).

Design decisions (locked): D1 remove `--root` (always global via ProjectStore);
D2 card=basename / detail=full path; D3 first path only per project key;
D4 reflective-only detail pane; D5 char-wrap title, 2-line cap, `…` ellipsis.

Every slice ends green: `cargo test -p curator` + `cargo clippy -p curator -- -D warnings`.

---

## Slice 1 — Data model + multi-project collector

- [x] Add curator-local `struct KanbanTask { meta: ArtifactMeta, project_key: String, project_root: PathBuf }` (app.rs).
- [x] Add `collect_tasks(store: &ProjectStore, branch: &str) -> Vec<KanbanTask>`:
      iterate `store.entries()`, take the FIRST path per key, call
      `cuelib::artifact::read_artifacts(path, branch, "task")`, tag each
      `ArtifactMeta` into a `KanbanTask`; skip missing/inaccessible roots.
- [x] Migrate `App` columns to `Vec<KanbanTask>`; update `new`, `reload_kanban`,
      `classify_tasks`, `column_tasks`, `column_sel`, and the selection helpers.
- [x] Tests (CUE_DATA_DIR + temp dirs): multi-project collection; missing-root
      skip; first-path-only; priority sort still holds on `KanbanTask`.

## Slice 2 — Wiring: drop `--root`, load from store

- [x] main.rs: remove `--root` from `Cli`; `ProjectStore::load()` (non-fatal,
      empty store on error); call `collect_tasks` at startup and on Refresh.
- [x] Drop `root` from `run()` / `process_msg()` / `reload_tasks`; keep `branch`.
- [x] Status-bar hint when the store is empty ("no projects registered").
- [x] Verify: `cargo test -p curator`; manual launch from a non-project dir.

## Slice 3 — Multi-line card rendering

- [x] Pure `wrap_title(title, width) -> Vec<Line>` (char-wrap, 2-line cap, `…`).
- [x] Unit tests: empty, exact-width, 1-line, 2-line, overflow-with-ellipsis.
- [x] `render_column` (ui.rs:89-145): build 3-line `ListItem` — lines 1-2 wrapped
      title, line 3 `[priority]  basename` (reuse `project_basename` ui.rs:610,
      priority color ui.rs:80-87). Inner width = rect − borders − highlight symbol.

## Slice 4 — Detail split pane

- [x] Add `KanbanLayout { ColumnsFull, Split }` to `App` (default `ColumnsFull`),
      mirroring `ActivityLayout` (app.rs:23-31).
- [x] `render_kanban`: `ColumnsFull` = today's full columns; `Split` = top columns
      (constrained height) + bottom detail pane.
- [x] New `render_kanban_detail` (static/reflective): full title, full project
      path, status — styled like `render_session_info` (ui.rs:255-327).
- [x] input.rs: Enter in Kanban toggles layout; update the kanban help bar
      (`Enter detail`).
- [x] Tests: layout-state toggle; detail-line derivation as a pure helper.
