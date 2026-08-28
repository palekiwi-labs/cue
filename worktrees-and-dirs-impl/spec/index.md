# Spec: Worktree Context Isolation

## Problem

cue's `.cue/HEAD` is a single file shared by all agents and the human developer.
There is one active context at a time and it belongs to whoever wrote it last.

This blocks parallel orchestration. When an orchestrator delegates work to
multiple concurrent sub-agents (implementation, review, QA), each agent needs
its own isolated context so their logs, plans, and notes do not bleed into each
other — but all agents must write to the same `.cue/` store so the orchestrator
can see and compose their outputs.

The workaround is threading a `--task <slug>` flag through every cue invocation
inside each sub-agent. This is not a structural solution: it is a per-call
correctness burden that compounds across delegation chains. Agents cannot be
reliably expected to pass the correct flag on every invocation, and env vars
are not viable because agents run in sandboxed containers that do not share
environment with the host.

The required property: an agent dropped into a directory should automatically
have the right context with no flags, no env vars, no ceremony. Context must be
a filesystem property, not a process property.

Git worktrees are the natural unit of parallel agent work. Each sub-agent gets
its own worktree on its own branch; changes in one do not affect the others.
cue currently gives every worktree under the same project the same default
context (the same `.cue/` store, the same `HEAD`). This spec closes that gap.

## Solution Overview

Introduce a `STORE` file inside a worktree's `.cue/` directory. When present,
it redirects all artifact I/O to the path it contains while leaving `HEAD`
local. Each worktree has its own `HEAD` (its own active context) and shares one
artifact store.

A new `cue link` command initializes a proxy `.cue/` — a plain directory
containing only `STORE` and `HEAD` — in a project worktree. The orchestrator
runs `cue link` once per worktree before handing it off to a sub-agent. The
sub-agent requires no further configuration.

## The `STORE` File

`STORE` is a plain-text file placed inside a `.cue/` directory. It contains a
single absolute path to a real cue store:

```
/absolute/path/to/main/project/.cue
```

When cue resolves the working store, it checks for `STORE` after finding the
local `.cue/` directory. If present:

- Read the absolute path from `STORE`.
- Verify the target exists and contains a `master/` subdirectory (structural
  check; errors loudly if not satisfied).
- Use the target path for all artifact I/O.
- Continue to use the local `.cue/HEAD` for context (scope) resolution.

If `STORE` is absent, use the local `.cue/` for both artifact I/O and context.

If `STORE` exists but its target contains another `STORE` file, error loudly.
Chaining is not supported.

### Resolution Precedence

From highest to lowest:

1. `--dir` flag (explicit per-invocation override)
2. `STORE` file in the local `.cue/` (filesystem-persistent redirect)
3. Local `.cue/` resolved from git root (existing default)

`HEAD` is always read from the local `.cue/`, regardless of precedence level.

## `cue link`

New top-level subcommand that initializes a proxy `.cue/` in the current
directory.

### Usage

```sh
cue link <store-path> [--task <slug>]
```

### Arguments

- `<store-path>` (required): absolute path to the real `.cue/` store to link
  to. Must exist and contain a `master/` subdirectory.
- `--task <slug>` (optional): task slug to write to `HEAD`. If omitted, `HEAD`
  is not written and the proxy defaults to the global context (same behavior as
  an empty `HEAD`).

### Behavior

1. Verify `<store-path>` exists and contains `master/` (error if not).
2. Verify a `.cue/` directory does not already exist in the current directory
   (error if it does, to prevent accidental re-linking).
3. Create `.cue/` as a plain directory (not a git worktree; no branch creation).
4. Write `.cue/STORE` containing the canonicalized absolute `<store-path>`.
5. If `--task` was given, write `.cue/HEAD` containing `<slug>` after
   structural validation (no path traversal, not the reserved slug `master`
   unless intentional, etc.).
6. Emit a warning to stderr if `--task` was given but no matching task card
   exists at `<store-path>/master/task/<slug>.md`. Exit 0 regardless.

### What `cue link` does not do

- Does not run `git worktree add` (that is the orchestrator's job before calling
  `cue link`).
- Does not commit or modify any git state.
- Does not register the worktree as a cue project.

## Store Resolution Refactor

### Current state

Every command independently computes:

```rust
let root = git::get_git_root(cwd)?;
let cue_dir = root.join(&config.dir_name);
```

And uses `cue_dir` for both `HEAD` reads and artifact I/O.

### Required change

Introduce a store-resolution helper in `cuelib` that returns a two-valued
struct:

```rust
pub struct ResolvedStore {
    /// Directory to read/write HEAD from (always local)
    pub head_dir: PathBuf,
    /// Directory to read/write artifacts from (may differ via STORE redirect)
    pub store_dir: PathBuf,
}
```

Resolution logic:

```
head_dir  = git_root.join(config.dir_name)
store_dir = if head_dir.join("STORE").exists() {
    read and canonicalize path from STORE
    validate target (exists, contains master/)
    validate no nested STORE in target
    target path
} else {
    head_dir
}
```

Call sites to refactor (at minimum):
- `commands/add.rs` and `add/mod.rs`
- `commands/list.rs` and `list/mod.rs`
- `commands/log.rs` and `log/mod.rs`
- `commands/switch.rs`
- `commands/status.rs`
- `commands/context.rs`

`curator` reads registered project paths only. Proxy worktrees are not
registered as cue projects, so curator is unaffected and requires no changes.

`cue.nvim` similarly operates on registered project paths. No changes required.

## `cue switch` in a Proxy Worktree

Under worktree isolation, each proxy worktree has its own local `.cue/HEAD`.
An agent calling `cue switch` inside its worktree writes only to that
worktree's HEAD and cannot clobber other contexts. The prior skill-level
prohibition on agents calling `cue switch` was premised on `HEAD` being a
single shared file; that premise is obsolete under worktree isolation, so the
prohibition has been dropped. Agents may call `cue switch` freely.

Technically, `cue switch` in a proxy worktree must:
- Write `HEAD` to the local `head_dir`.
- Create the scope directory under `store_dir` (the STORE target).

This is the natural consequence of the two-valued resolution: scope-dir
creation goes through `store_dir`, HEAD write goes through `head_dir`. No
special-casing is needed once the refactor is in place.

## Worktree Placement Convention

Worktrees must be created as nested directories under the project root, not as
siblings:

```
./worktrees/<branch-name>/
```

Rationale: cast mounts the project root as the container's working directory.
Sibling directories fall outside the mount boundary. Nested worktrees are
within scope automatically.

`worktrees/` must be added to the main project's `.gitignore`.

`.cue/` must be added to the `.gitignore` of each feature branch checked out
in a worktree. This prevents the proxy `.cue/` (containing `STORE` and `HEAD`)
from appearing as untracked files in `git status` within the worktree.

## Concurrency Model

cue commands never commit. Artifact creation is file-create-only. All git
add/commit operations on the shared store are performed by the human developer
as a single serialized actor.

Per-task-scope directories (`proj-123-impl/`, `proj-123-review/`) keep each
agent's `log.md` and plans in separate paths. Concurrent appends to different
scope `log.md` files do not race.

Writes to `master/` (task cards, global spec) are not race-protected by the
scope mechanism. The operational convention is that task cards are created by
the orchestrator before sub-agents are launched, and sub-agents do not create
or modify task cards. This is a dev SOP / agent skill constraint, not enforced
by the code.

## Resulting Layout

```
project/ (main worktree, cast CWD mount)
  .gitignore              <- includes: worktrees/
  .cue/                   <- real artifact store (git worktree on cue branch)
    HEAD                  <- orchestrator's active context ("proj-123")
    master/
      task/
        proj-123.md
        proj-123-impl.md
        proj-123-review.md
    proj-123/
      log.md
    proj-123-impl/
      log.md              <- impl agent writes here
    proj-123-review/
      log.md              <- review agent writes here
  worktrees/
    proj-impl/            <- git worktree on branch proj-123-impl
      .gitignore          <- includes: .cue/
      .cue/               <- proxy (plain directory)
        STORE             -> /abs/path/to/project/.cue
        HEAD              -> proj-123-impl
      src/
    proj-review/          <- git worktree on branch proj-123-review
      .gitignore          <- includes: .cue/
      .cue/               <- proxy (plain directory)
        STORE             -> /abs/path/to/project/.cue
        HEAD              -> proj-123-review
      src/
```

## Orchestrator Workflow

1. Create task cards for each sub-task on master (before launching agents).
2. For each sub-agent:
   a. `git worktree add worktrees/<branch-name> -b <branch-name>`
   b. `cue link /abs/path/to/.cue --task <slug>`
      (run from inside `worktrees/<branch-name>/`)
   c. Launch sub-agent with CWD set to `worktrees/<branch-name>/`.
3. Sub-agent runs. All its cue operations read `HEAD` from its local
   `worktrees/<branch-name>/.cue/HEAD` and write artifacts to the shared store.
4. Orchestrator inspects results via `cue list --all` from the main worktree.

## Out of Scope for This Spec

- STORE target validation beyond existence and `master/` check (deferred).
- cast mount semantics documentation (separate concern).
- Absolute path portability across container mounts (operational requirement
  on cast: mount at identical inside/outside path).
- cue.nvim and curator changes (not required; they operate on registered
  projects only).
- `cue worktree remove` / pruning of orphaned scope dirs (future story).
- `parent:` frontmatter implementation in the cue CLI (separate task).
