---
status: complete
refs: .cue/use-default-context-from-config/plan/index.md
---
# Slice 1: Implement config fallback for context commands

## Foreword

This plan implements the fallback feature described in `plan/index.md`. The
active cue task is `use-default-context-from-config`. All work is in
`crates/cue/src/context/mod.rs` and `crates/cue/src/commands/context.rs`.

Prerequisites: feature branch `feat/use-default-context-from-config` checked out.

## Steps

- [x] Create feature branch `feat/use-default-context-from-config`
- [x] Write failing tests for the fallback behavior in `context/mod.rs`
- [x] Add `load_context_or_config` helper
- [x] Add `resolve_profile_with_config` for root scope artifact resolution
- [x] Update `gather_context` to use both new functions
- [x] Update `handle_show` and `handle_profiles` in `commands/context.rs`
- [x] Fix error message when profile not found in fallback config
- [x] Run `cargo test` — all tests pass
- [x] Run `cargo clippy` and `cargo fmt` — clean
- [x] Commit (0b8a772)
