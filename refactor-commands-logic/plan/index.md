---
status: open
---
# Plan: Refactor `src/commands/` to Thin Handlers

## Goal

Bring `src/commands/` into conformance with the rust-cli guideline: handlers
are thin dispatchers; domain logic lives in top-level source modules.

The reference model is `src/commands/context.rs` -> `src/context/`.

---

## Target Directory Structure

```
src/
  main.rs               -- thin entrypoint (resolve_clipboard moves to src/add/)
  cli.rs                -- Clap types (one import path change)
  config.rs             -- unchanged
  git.rs                -- unchanged
  context/mod.rs        -- unchanged (already correct)

  add/mod.rs            -- NEW domain module
  init/mod.rs           -- NEW domain module
  list/mod.rs           -- NEW domain module
  log/mod.rs            -- NEW domain module

  commands/
    mod.rs              -- log entry changes from dir to file
    add.rs              -- thinned handler
    init.rs             -- thinned handler
    list.rs             -- thinned handler
    context.rs          -- unchanged
    config.rs           -- unchanged
    log.rs              -- NEW single file (replaces commands/log/ subdirectory)
```

The `src/commands/log/` subdirectory (`mod.rs`, `add.rs`, `list.rs`) is deleted
and replaced by the single file `src/commands/log.rs`.

---

## What Moves Where

### `src/add/mod.rs`

Sources: `src/commands/add.rs` and `src/main.rs`

| Item | Source location |
|---|---|
| `pub struct AddOptions` | `commands/add.rs:8-16` |
| `validate_filename(filename) -> Result<()>` | `commands/add.rs:130-146` |
| `build_frontmatter_bytes(fields) -> Result<Vec<u8>>` | `commands/add.rs:115-128` |
| `resolve_dest_dir(...)` | inline in `commands/add.rs:65-73` |
| `write_artifact(path, content, force)` | inline in `commands/add.rs:88-105` |
| `resolve_clipboard(filename) -> Result<Vec<u8>>` | `main.rs:98-144` |
| `pub fn add(root, opts: AddOptions) -> Result<PathBuf>` | orchestration in `commands/add.rs:18-113` |

`src/commands/add.rs` becomes: resolve git root + config, construct `AddOptions`,
call `crate::add::add(...)`, print the returned path.

---

### `src/init/mod.rs`

Source: `src/commands/init.rs`

| Item | Source location |
|---|---|
| `ensure_worktree(root, cue_path, config) -> Result<()>` | `commands/init.rs:46-84` |
| `pub fn init(root, config) -> Result<()>` | `commands/init.rs:7-44` body |

`src/commands/init.rs` becomes: get git root + config, call
`crate::init::init(root, config)`, print confirmation.

---

### `src/list/mod.rs`

Source: `src/commands/list.rs` (largest extraction)

| Item | Source location |
|---|---|
| `pub struct ListOptions` | `commands/list.rs:114-122` |
| `pub struct Filter`, `FilterOp`, `FromStr for Filter` | `commands/list.rs:14-66` |
| `get_nested`, `evaluate_filter`, `apply_filters` | `commands/list.rs:69-99` |
| `CueFile` struct | `commands/list.rs:101-112` |
| `is_valid_cue_file(...)` | `commands/list.rs:242-273` |
| `to_cue_file(...)` | `commands/list.rs:275-340` |
| `extract_frontmatter_yaml(...)` | `commands/list.rs:342-367` |
| `collect_files(...)` | `commands/list.rs:369-385` |
| `resolve_scan_paths(...)` | `commands/list.rs:217-240` |
| all `#[cfg(test)]` unit tests | `commands/list.rs:388-528` |
| `pub fn list(root, opts) -> Result<Vec<(PathBuf, Option<Value>)>>` | `commands/list.rs:124-215` core |

`Filter` must remain `pub` — `cli.rs` imports it.

Import in `src/cli.rs:1` changes from:
```rust
use crate::commands::list::Filter;
```
to:
```rust
use crate::list::Filter;
```

`src/commands/list.rs` becomes: get git root + config, call `crate::list::list(...)`,
handle output formatting (the `println!` / JSON printing stays — it is presentation,
not domain logic).

---

### `src/log/mod.rs`

Source: `src/commands/log/add.rs`

| Item | Source location |
|---|---|
| `LogEntry` struct | `commands/log/add.rs:10-20` |
| `pub struct LogAddOptions` | new (wraps current flat args) |
| `build_log_markdown(entry, hash) -> String` | inline in `commands/log/add.rs:97-137` |
| `pub fn add_entry(root, LogAddOptions) -> Result<PathBuf>` | `commands/log/add.rs:22-147` |

---

### `src/commands/log.rs` (new single file)

Replaces the entire `src/commands/log/` subdirectory. Combines the thinned add
handler and the already-thin list handler:

```rust
pub fn handle(cwd: &Path, command: LogCommands) -> Result<()> {
    match command {
        LogCommands::Add { .. } => {
            // construct LogAddOptions, call crate::log::add_entry(...)
        }
        LogCommands::List { branch } => {
            // the 43-line body from commands/log/list.rs (already thin, moves here verbatim)
        }
    }
}
```

---

## Files Deleted

- `src/commands/log/mod.rs`
- `src/commands/log/add.rs`
- `src/commands/log/list.rs`

---

## Files Unchanged

- `src/commands/context.rs` -- already the reference model
- `src/commands/config.rs` -- already correct
- `src/config.rs`
- `src/git.rs`
- `src/context/mod.rs`
- `tests/` (all integration tests exercise the binary; zero changes needed)
- `Cargo.toml` (no `[lib]` target needed)

---

## Execution Order

1. Create `src/add/mod.rs` with extracted domain logic + move `resolve_clipboard`
2. Thin `src/commands/add.rs`
3. Create `src/init/mod.rs` with extracted domain logic
4. Thin `src/commands/init.rs`
5. Create `src/list/mod.rs` with extracted domain logic + unit tests
6. Thin `src/commands/list.rs`
7. Fix `src/cli.rs:1` import
8. Create `src/log/mod.rs` with extracted domain logic
9. Create `src/commands/log.rs` (thin handler, single file)
10. Delete `src/commands/log/` subdirectory
11. Register new top-level modules in `src/main.rs`
12. Run `cargo test` -- all tests must pass with no changes to `tests/`
