---
status: complete
---
# Executive Plan: Rename `mem` → `cue`

## Foreword

This plan implements the source-code-only rename of the `mem` CLI to `cue`. It covers
Cargo.toml, config defaults, internal Rust identifiers, integration tests, the Nix flake,
and documentation. GitHub and local directory renames are handled separately as manual todos.

The plan follows the ordering from the master plan: source changes first, gate on `cargo test`,
then docs and flake.

---

## Steps

- [x] Slice 1: `Cargo.toml` — rename package to `cue`
- [x] Slice 2: `src/config.rs` — rename all string literals and env prefix
- [x] Slice 3: Rust identifiers — `src/cli.rs`, `src/commands/list.rs`, `src/commands/add.rs`
      (expanded: also `init.rs`, `log/add.rs`, `log/list.rs`, `context/mod.rs`,
      `commands/context.rs`; threaded `dir_name` through context functions)
- [x] Gate: `cargo test` — 114 tests pass
- [x] Commit: source code rename (`d1a9cf9`)
- [x] Slice 4: `flake.nix` — rename pname, devshell name, description
- [x] Slice 5: `AGENTS.md` — update all references
- [x] Slice 6: `.mem/master/spec/index.md` — update tool name references
- [x] Commit: docs and flake rename
