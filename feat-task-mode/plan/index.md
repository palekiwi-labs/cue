---
status: open
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

- [ ] Implement `resolve_scope(root: &Path) -> Result<String>` in `cuelib`.
  Reads `.cue/HEAD`; returns `"master"` when the file is absent or empty.
  There is no branch fallback — git branch is no longer consulted for scope
  resolution.
- [ ] Implement `read_head(root: &Path) -> Option<String>` and `write_head(root:
  &Path, slug: &str) -> Result<()>` helpers.
- [ ] Replace every direct call to `get_current_branch()` in write paths (`add`,
  `log`, `context`, anchor-bucket logic) with a call to `resolve_scope()`.

#### `cue` CLI

- [ ] New `cue switch <slug>` subcommand: validates the slug (not empty, not
  `master` unless switching to global context), writes to `.cue/HEAD`,
  auto-creates `.cue/<slug>/` if absent.
- [ ] `cue switch master`: writes `master` to `.cue/HEAD` (returns to global
  context).
- [ ] `cue switch <filepath>`: accepts a task card path, derives slug from
  filename stem, delegates to slug form.
- [ ] `cue switch --branch`: scans `master/task/*.md` for any card whose
  `branch:` list contains the current git branch; switches to the matching task.
  No-op if no match. Does not overwrite a manually pinned context if the current
  HEAD was set by an explicit `cue switch <slug>` (consider a `--force` flag or
  a pinning mechanism).
- [ ] New `cue status` subcommand: reads `.cue/HEAD` (absent → `master`),
  prints active context. Also prints the task card's title and status when a
  task is active.
- [ ] Add `--task <slug>` flag to all artifact-writing subcommands (`add`, `log`,
  etc.). When present, overrides the result of `resolve_scope()` for that
  invocation without modifying `.cue/HEAD`.
- [ ] Update `cue context`: inject active task slug, title, and status into the
  rendered context so agents are immediately oriented.

#### SKILL.md

- [ ] Document `cue switch`, `cue status`, `--task` flag.
- [ ] Document `.cue/HEAD` semantics and the global/master fallback.
- [ ] Update the cross-command scope-resolution table (`--task` flag vs. HEAD).

---

## Deferred (not in scope for either part)

- Git worktrees: per-worktree HEAD files
- `cue task create` as a first-class CLI command
- `cue list --all` disambiguation
