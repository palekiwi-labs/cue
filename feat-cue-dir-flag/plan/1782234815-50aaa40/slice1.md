---
status: open
---
---
status: complete
---
# Plan: add --dir / -C global flag to cue CLI

## Foreword

This plan implements the `--dir`/`-C` global flag for the `cue` CLI on
branch `feat/cue-dir-flag`. It addresses task
`master/task/1782234815-50aaa40/cue-dir-flag.md`.

All `cue` subcommands currently resolve their working directory from
`env::current_dir()` at `crates/cue/src/main.rs:20`. Every handler
already takes `&cwd` as its first argument — so overriding that single
value is the entire change.

The flag follows the git `-C <path>` convention: accepted before or
after the subcommand (clap `global = true`).

Prerequisite: feature branch `feat/cue-dir-flag` is checked out.

## Steps

- [x] Create task `master/task/1782234815-50aaa40/cue-dir-flag.md`
- [x] Add `--dir` / `-C` field to `Cli` in `crates/cue/src/cli.rs`
- [x] Resolve `cwd` from the flag (validate + canonicalize) in
      `crates/cue/src/main.rs`
- [x] Write tests: valid `--dir`, `-C` alias, non-existent path error,
      non-directory path error
- [x] `cargo test --workspace` green
- [x] `cargo clippy --workspace -- -D warnings` clean
- [x] `cargo fmt --check` clean
- [x] Commit (a0f4f87)
- [x] Request code review, address findings (8b2d012)
