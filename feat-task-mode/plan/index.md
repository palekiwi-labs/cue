---
status: complete
refs: .cue/master/spec/cue/task-mode.md
---
# Task workflow — Phase 2: HEAD-driven context directories

Implements the second phase of the task-centric context system described in
`spec/cue/task-mode.md`. Requires Phase 1 (flat task-card layout, tracked in
the `ai` coordination workspace) to be merged first.

---

## Phase 2: HEAD-driven context directories

**Goal:** Introduce the `resolve_scope()` abstraction, `.cue/HEAD`, `cue
switch`, and task-scoped context directories. Requires Part 1 to be merged
first.

### Scope

#### `cuelib`

- [x] Implement `resolve_scope(root: &Path) -> Result<String>` in `cuelib`.
  Reads `.cue/HEAD`; returns `"master"` when the file is absent or empty.
  There is no branch fallback — git branch is no longer consulted for scope
  resolution.
- [x] Implement `read_head(root: &Path) -> Option<String>` and `write_head(root:
  &Path, slug: &str) -> Result<()>` helpers.
- [x] Replace every direct call to `get_current_branch()` in write paths (`add`,
  `log`, `context`, anchor-bucket logic) with a call to `resolve_scope()`.

#### `cue` CLI

- [x] New `cue switch <slug>` subcommand: validates the slug (not empty, not
  `master` unless switching to global context), writes to `.cue/HEAD`,
  auto-creates `.cue/<slug>/` if absent.
- [x] `cue switch master`: writes `master` to `.cue/HEAD` (returns to global
  context).
- [x] `cue switch <filepath>`: accepts a task card path, derives slug from
  filename stem, delegates to slug form.
- [x] `cue switch --branch <name>`: scans `master/task/*.md` for any card whose
  `branch:` list contains `<name>`; bails with non-zero exit and HEAD unchanged
  on no match (post-checkout-hook use case). Requires a required argument.
- [x] New `cue status` subcommand: reads `.cue/HEAD` (absent → `master`),
  prints active context. Also prints the task card's title and status when a
  task is active.
- [x] Add `--task <slug>` flag to all artifact-writing subcommands (`add`, `log`,
  etc.). When present, overrides the result of `resolve_scope()` for that
  invocation without modifying `.cue/HEAD`.
- [ ] Update `cue context`: inject active task slug, title, and status into the
  rendered context so agents are immediately oriented. (Deferred — tracked in
  `master/todo/1783930716-2c3c5d8/inject-task-into-context-render.md`.)

#### SKILL.md

- [x] Document `cue switch`, `cue status`, `--task` flag.
- [x] Document `.cue/HEAD` semantics and the global/master fallback.
- [x] Update the cross-command scope-resolution table (`--task` flag vs. HEAD).

---

## Deferred (not in scope for either part)

- Git worktrees: per-worktree HEAD files
- `cue task create` as a first-class CLI command
- `cue list --all` disambiguation
- `cue context` task injection (criterion #8): tracked in
  `master/todo/1783930716-2c3c5d8/inject-task-into-context-render.md`
- JSON output field `"branch"` in `cue list --json`: rename to `"scope"` or
  `"task"` for consistency (low priority cosmetic)
- `.cue/HEAD` gitignore: tracked in
  `feat-task-mode/todo/1783925542-94dbce2/cue-head-should-be-gitignored.md`
