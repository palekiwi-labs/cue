# Cue task-based workflow

---

## Context

`cue` currently derives the scope directory from the active git branch:
`get_current_branch()` returns a string that is sanitized and used as a path
segment under `.cue/`. This was a pragmatic early decision, but it couples the
memory system to git mechanics in ways that create friction:

- **Coordination workspaces** (research hubs, planning repos) have no meaningful
  branches. All artifacts collapse into a single context with no per-topic
  isolation.
- **Multi-branch tasks** (a feature split across two PRs) scatter related
  context across multiple branch directories.
- **Context switching** requires a `git checkout` even when the intent is
  purely mental: "I want to work on X now."

The root problem is that the scope key defaults to `git branch`, and a branch
is the wrong unit. The right unit is a **task**: a named, semantic piece of
work with its own isolated context.

---

## Proposal

Replace branch-derived scope with task-derived scope entirely. The active task
is stored in `.cue/HEAD`; all artifact writes are scoped to the active task's
context directory. There is no longer a "branch mode" — task-based scope is the
only mode.

The implementation is split into two independent parts that can be shipped
separately:

- **Part 1** — Flat task-card layout: move task cards from anchored
  subdirectories to flat files named by slug. Small, safe, no new CLI surface.
- **Part 2** — HEAD-driven context: introduce `.cue/HEAD`, `cue switch`, and
  task-scoped context directories. Requires the `resolve_scope()` abstraction.

---

## Part 1: Flat task-card layout

### Task identity

A task's identity is its **slug**: a human-readable, lowercase-hyphenated
string (e.g., `auth-login`, `cue-task-mode`). The slug is:

- The filename stem of the task card file
- The name of the task's context directory (Part 2)

The filename stem is the single source of truth for the slug. No frontmatter
field duplicates it — identity is derived from the path, consistent with how
all other artifacts are resolved in this system.

#### Slug rules

- Set at task creation time, **immutable thereafter**
- Unique within the repository, enforced by the flat file layout
- Lowercase, hyphen-separated, no spaces or special characters
- `master` is a reserved slug and must be rejected at creation time

### Task card storage

#### Current layout (branch-derived scope)

Task cards are stored under a point-in-time anchor subdirectory:

```
.cue/master/task/<timestamp>-<hash>/<slug>.md
```

Multiple cards may share an anchor bucket when created in the same git session.
Slug uniqueness is not enforced.

#### New layout (task-based scope)

Task cards are stored **flat**, with no anchor subdirectory:

```
.cue/master/task/<slug>.md
```

For example:

```
.cue/master/task/auth-login.md
.cue/master/task/cue-task-mode.md
.cue/master/task/db-migration.md
```

Slug uniqueness is enforced by the filesystem. Card paths are stable and
predictable as reference targets in `refs:` and in tooling.

### Task frontmatter

Task cards gain one new frontmatter field.

#### `branch:` (optional list)

A list of git branch names associated with this task. Used in Part 2 by
`cue switch --branch` for automatic context switching via git hooks:

```yaml
---
status: open
priority: normal
title: "Implement auth login"
branch:
  - feature/auth-login
  - feature/auth-login-cleanup
---
```

### Affected components (Part 1)

- `cuelib / artifact.rs` — Task card reader accepts flat layout; board scanner reads `master/task/*.md` directly
- `curator` — Task enumeration updated to flat path pattern
- `cue-plugins / cue-task.ts` — Writes flat cards (no anchor subdirectory)
- nvim plugin — Task picker updated for flat card paths
- SKILL.md — Task storage documentation updated

---

## Part 2: HEAD-driven context directories

### The `resolve_scope()` abstraction

The current codebase derives the scope directory at multiple independent call
sites, each calling `get_current_branch()` directly (`add`, `log`, `context`,
anchor-bucket logic in `add/mod.rs`). Task-based scope introduces a single
chokepoint:

```rust
fn resolve_scope(root: &Path) -> Result<String> {
    Ok(read_head(root).unwrap_or_else(|| "master".into()))
}
```

Every write path calls `resolve_scope()` instead of calling
`get_current_branch()` directly. There is no branch fallback — if `.cue/HEAD`
is absent or empty the global context (`master`) is used.

### Task context directory

The working context for a task lives at `.cue/<slug>/`, mirroring the
structure of the current `.cue/<branch>/` directories exactly:

```
.cue/
  HEAD                          <- active task pointer
  master/                       <- global context (unchanged)
    log.md
    spec/
    task/
      auth-login.md             <- task card (flat, from Part 1)
      db-migration.md
  auth-login/                   <- task context
    log.md
    plan/
      index.md
      slice-1.md
    spec/
      index.md
    trace/
      <timestamp>-<hash>/
        build-error.log
    note/
      open-question.md
    todo/
      refactor-items.md
```

All existing artifact types are supported within a task context. Internal
storage conventions — anchored vs. root artifacts, frontmatter structure,
`<timestamp>-<hash>` bucket naming — are identical to the current behaviour.
Only the source of the top-level directory name changes.

The context directory is auto-created on first `cue switch <slug>`.

### Global context

`.cue/master/` remains the global context — the home for cross-task artifacts:
global `log.md`, cross-project `spec/`, and the task board (`task/`).

When `.cue/HEAD` is absent or contains `master`, the global context is active.
All cue operations write to `.cue/master/` in that state. `cue status` handles
a missing HEAD file gracefully, reporting `master` rather than erroring.

### Active task pointer: `.cue/HEAD`

The active task is stored in `.cue/HEAD`, a plain-text file containing exactly
the task slug:

```
auth-login
```

Rules:

- Absent or containing `master` → global context is active.
- Any other value → that task's context directory (`.cue/<slug>/`) is active.
- The `--task <slug>` flag overrides HEAD for a single invocation without
  modifying the file.
- Created by `cue switch`; read by `resolve_scope()`.

### CLI surface

#### `cue switch <slug>`

Sets the active task. Writes `<slug>` to `.cue/HEAD`. Creates `.cue/<slug>/`
if it does not exist. `cue switch master` returns to the global context.

Also accepts a task card filepath, reading the slug from the filename stem:

```sh
cue switch auth-login
cue switch master
cue switch .cue/master/task/auth-login.md
```

#### `cue switch --branch`

Auto-selects the active task based on the current git branch. Scans task cards
in `.cue/master/task/` for any card whose `branch:` list contains the
checked-out branch name, and switches to the matching task. If no match is
found, the active context is unchanged.

Intended for use in a git `post-checkout` hook:

```sh
#!/usr/bin/env sh
# .git/hooks/post-checkout
cue switch --branch
```

An explicit `cue switch <slug>` always takes precedence over what the hook
would set. The hook should not overwrite a manually pinned context.

#### `cue status`

Prints the active context. Handles a missing `.cue/HEAD` gracefully:

```sh
$ cue status
active task: auth-login
  title: Implement auth login
  status: in-progress
  context: .cue/auth-login/

$ cue status    # global context
active context: master (global)
```

#### `--task <slug>` flag

Available on all artifact-writing commands. Overrides HEAD for a single
invocation without modifying `.cue/HEAD`. Replaces `--branch` in the new
workflow:

```sh
cue log --task auth-login "Finished the OAuth flow"
cue add --task db-migration --type trace --filename errors.log
```

#### `cue context` (updated)

`cue context` includes the active task's slug, title, and status so agents are
immediately oriented without needing to query the active task themselves.

### Affected components (Part 2)

- `cuelib` — New `resolve_scope(root)` function; `read_head` / `write_head` helpers for `.cue/HEAD`; remove `get_current_branch()` call sites from all write paths
- `cue` CLI — New `switch` subcommand; `--task` flag on all write commands; updated `status` and `context`
- SKILL.md — `cue switch`, `cue status`, `--task` flag, task workflow documentation

---

## Deferred

**Git worktrees.** A single `.cue/HEAD` is shared across all worktrees in a
multi-worktree setup. Two parallel agents can overwrite each other's active
context. The full solution — per-worktree HEAD files mirroring git's own
mechanism — is deferred. Mitigation: use `--task <slug>` explicitly in
automated multi-worktree workflows.

---

## Open questions

- **`cue list --all`.** Task context directories (`.cue/<slug>/`) and the
  global context (`.cue/master/`) coexist at the same level. How should
  `cue list --all` distinguish them?
- **Task creation UX.** Should `cue task create "..."` be a first-class CLI
  command enforcing the flat layout and forbidden slugs, or does task creation
  remain the responsibility of `cue-plugins`?
