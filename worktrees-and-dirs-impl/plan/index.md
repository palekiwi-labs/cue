---
status: complete
refs: cue/master/spec/index.md
---
# Master Plan: Worktree Context Isolation

## Problem

cue currently uses a single path for both HEAD resolution and artifact I/O:

```
git_root.join(config.dir_name)   // == .cue/
```

Every command computes this path independently. There is no mechanism to
redirect artifact writes to a shared store while keeping HEAD local.

This blocks parallel agent orchestration. Each sub-agent in a git worktree
must write to the same shared artifact store but maintain its own HEAD context.

## Approach

Three deliverables, implemented in sequence:

1. **`ResolvedStore` in `cuelib`**: A struct and resolution function that
   splits the single `.cue/` path into two: `head_dir` (always local) and
   `store_dir` (may be redirected via a `STORE` file). This is the core
   abstraction the rest of the work depends on.

2. **Refactor all command call sites** to use `ResolvedStore`. All commands
   currently pass one `cue_dir` value everywhere. After the refactor, each
   command uses `head_dir` for HEAD reads/writes and `store_dir` for artifact
   I/O. With no `STORE` file present, `head_dir == store_dir`, so existing
   behavior is fully preserved.

3. **`cue link` subcommand**: A new top-level command that creates a proxy
   `.cue/` directory (containing `STORE` and optionally `HEAD`) in the current
   directory (or a `--dir`-specified directory). Run once by the orchestrator
   to wire up a git worktree before handing it to a sub-agent.

## Architecture

### `ResolvedStore`

Defined in `crates/cuelib/src/store.rs` (new file):

```rust
pub struct ResolvedStore {
    /// Directory to read/write HEAD from (always the local .cue/).
    pub head_dir: PathBuf,
    /// Directory to read/write artifacts from.
    /// Equals head_dir unless a STORE file redirects it.
    pub store_dir: PathBuf,
}

pub fn resolve_store(cue_dir: PathBuf) -> Result<ResolvedStore>
```

Resolution logic:

1. Set `head_dir = cue_dir`.
2. Check if `cue_dir/STORE` exists.
3. If yes: read the path, validate it (exists + contains `master/` subdir),
   set `store_dir` to the canonicalized target path.
4. If no: `store_dir = head_dir`.

### Commands affected

All commands use `root.join(config.dir_name)` today. After the refactor:

- `cue add` (and wrappers: task, plan, todo, note):
  - `head_dir` for `resolve_scope` (HEAD read)
  - `store_dir` for artifact directory creation and file writes
- `cue log add`:
  - `head_dir` for `resolve_scope`
  - `store_dir` for log.md writes and directory creation
- `cue log list`:
  - `head_dir` for `resolve_scope`
  - `store_dir` for log.md reads
- `cue list`:
  - `head_dir` for `resolve_scope`
  - `store_dir` for artifact directory traversal and reads
- `cue switch`:
  - `head_dir` for HEAD writes
  - `store_dir` for scope directory creation and task card scanning
- `cue status`:
  - `head_dir` for HEAD read
  - `store_dir` for task card reads
- `cue context`:
  - `head_dir` for HEAD reads and writes

`cue init` is excluded from this refactor. It creates the real store and
runs before any STORE file can exist.

### `cue link`

New file: `crates/cue/src/commands/link.rs`

Flags:
- Positional `<store-path>` (required): absolute path to the real `.cue/` store
- `--task <slug>` (optional): slug to write to HEAD
- `--dir <path>` (optional): target directory; defaults to CWD

Steps:
1. Resolve target directory (CWD or `--dir`).
2. Validate `<store-path>` exists and contains `master/`.
3. Canonicalize `<store-path>`.
4. Validate `target/.cue/` does not already exist (error if it does).
5. Create `target/.cue/` (plain directory, not a git worktree).
6. Write `target/.cue/STORE` with the canonicalized path.
7. If `--task` given:
   - Validate slug (no path traversal; no empty string).
   - Write `target/.cue/HEAD` with the slug.
   - Warn to stderr if `<store-path>/master/task/<slug>.md` does not exist.
8. Exit 0.

## Implementation Phases

### Phase 1 — `ResolvedStore` in cuelib (TDD)

- Add `crates/cuelib/src/store.rs` with `ResolvedStore` struct and
  `resolve_store` function.
- Export from `crates/cuelib/src/lib.rs`.
- Unit tests: no STORE file (passthrough), valid STORE redirect, STORE
  pointing to nonexistent path, STORE pointing to path without `master/`.

### Phase 2 — Refactor command call sites

- Update each affected command to call `resolve_store` and thread `head_dir`
  vs `store_dir` to the right call sites.
- Where commands call `cuelib` helpers, update helper signatures if necessary
  or pass the correct dir to existing functions.
- Run full test suite after each command update (incremental, testable slices).
- Commands in order: status, context, switch, log (add + list), list, add.

### Phase 3 — `cue link` command

- Implement `crates/cue/src/commands/link.rs`.
- Register in the CLI `App`.
- Integration tests: happy path, store-path missing, store-path lacks master/,
  .cue/ already exists, --task with no matching card (warn but exit 0),
  --dir flag.

### Phase 4 — Deferred

- STORE chaining detection (todo captured in .cue/).
- `worktrees/` in main project `.gitignore` (operational SOP, not code).

## Key Constraints

- `head_dir == store_dir` when no STORE file is present; all existing tests
  must continue to pass without modification.
- `cue link` creates a plain directory, not a git worktree or git init.
- `cue link --task master` is permitted (writes `master` to HEAD).
- No version bumps, no backwards compat concerns (prototyping stage).
