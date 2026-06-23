# Project Log

## [a0f4f87] feat/cue-dir-flag: --dir / -C global flag implemented

Commit a0f4f87 on feat/cue-dir-flag.

Changes:
- crates/cue/src/cli.rs: Added Optional PathBuf field `dir` with
  short = 'C', long = 'dir', global = true to Cli struct.
- crates/cue/src/main.rs: Resolution block validates path exists and
  is a directory, canonicalises it, replaces env::current_dir(). No
  command handlers needed changes — they all already take &cwd.
- crates/cue/tests/dir_flag.rs: 4 integration tests (long flag,
  -C alias, nonexistent path, file-not-directory error).

All workspace tests pass (35 acuity + all cue/cuelib/others).
Clippy and fmt clean.

- **Decided:** global = true on the clap Arg so the flag is accepted before or after the subcommand name, matching git -C ergonomics
- **Decided:** Validate + canonicalize at the main() boundary; no changes needed in any command handler
- **Decided:** Error messages use the --dir prefix in the string so error output is unambiguous regardless of whether the user passed -C or --dir

## [8b2d012] feat/cue-dir-flag: review fixes complete, task closed

Completed the --dir flag task on feat/cue-dir-flag. All acceptance
criteria verified; task marked complete.

Commits (in order):
- a0f4f87 feat: add --dir / -C global flag to cue CLI
  - cli.rs: Optional PathBuf field with global = true
  - main.rs: validate (exists + is_dir) then canonicalize
  - dir_flag.rs: 4 integration tests
- 8b2d012 refactor: simplify --dir validation + expand test coverage
  - main.rs: single metadata() call instead of exists()/is_dir()
  - dir_flag.rs: 4 new tests (relative path, non-git dir,
    flag-after-subcommand, add mutation path)

Final state: 8 dir_flag tests, full workspace green, clippy/fmt clean.

Code review (consultant-opus): APPROVED with minor suggestions.
All actionable feedback addressed in 8b2d012. One deferred item
captured as todo/1782236157-8b2d012/cross-binary-dir-flag-naming.md
(curator --root vs cue --dir naming inconsistency; address in Phase 6).

Original todo master/todo/1782211100-a8d0fb9/cue-support-dir-flag.md
closed as superseded by the task.

- **Found:** clap global = true is the correct attribute for a flag accepted before or after the subcommand name
- **Found:** metadata() follows symlinks, so it correctly treats a symlink-to-directory as a directory (same semantics as is_dir)
- **Found:** curator already has a --root flag (crates/curator/src/main.rs:22-23) with no validation that does the same conceptual thing as the new cue --dir flag — naming inconsistency to align later
- **Decided:** Use single metadata() call (Opus suggestion) instead of exists()/is_dir() — fewer syscalls, identical semantics
- **Decided:** Add 4 tests from review feedback: relative path, non-git dir, flag-after-subcommand (guards global=true), add mutation path
- **Decided:** Defer curator --root vs cue --dir naming alignment to Phase 6; captured as a todo

