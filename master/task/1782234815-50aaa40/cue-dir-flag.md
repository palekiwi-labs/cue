---
title: 'cue: add --dir / -C global flag'
status: complete
priority: high
---
# cue: add --dir / -C global flag

Add a global `--dir`/`-C` flag to the `cue` CLI so that all
subcommands operate as if invoked from the given directory, rather
than always using the process working directory.

Primary consumer: agents that are executing inside one project
directory but need to manage `.cue/` artifacts in another registered
project without changing their own CWD.

## Source

- `todo/1782211100-a8d0fb9/cue-support-dir-flag.md`
- `spec/index.md`

## Acceptance Criteria

| #  | Criterion | Verify by | Evidence |
|----|-----------|-----------|----------|
| 1  | `cue --dir <path> list` lists artifacts for the project at `<path>`, not CWD | `cargo test` + manual smoke test | `test_dir_flag_targets_given_path` (8b2d012) |
| 2  | `-C <path>` short alias works identically | `cargo test` | `test_short_alias_c` (a0f4f87) |
| 3  | Non-existent or non-directory path produces a clear error, not a panic | `cargo test` | `test_dir_flag_nonexistent_path_errors`, `test_dir_flag_file_path_errors` (a0f4f87) |
| 4  | All existing tests pass unmodified | `cargo test --workspace` | 8 dir_flag + all workspace tests pass (8b2d012) |
| 5  | `cargo clippy -- -D warnings` clean | CI / local run | Verified clean (8b2d012) |
