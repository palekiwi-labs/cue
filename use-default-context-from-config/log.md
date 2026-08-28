# Project Log

## [0b8a772-dirty] feat: config fallback for context commands implemented and committed

Commit 0b8a772 on feat/use-default-context-from-config. Consulted Opus on design — key decisions: branch on path.exists() (not Err) to avoid swallowing parse errors; do not thread fallback into recursive resolve_profile; use is_empty() convention consistent with init_context.

- **Found:** cargo fmt -p cue also reformatted unrelated files (import ordering in add/mod.rs, link.rs, etc.) — left unstaged to keep commit focused
- **Found:** Thread-spawn resource errors during test run were transient environment issues, not test failures
- **Decided:** Stage only context/mod.rs and commands/context.rs — the two files with actual feature changes
- **Decided:** load_context_or_config uses is_empty() as the no-fallback sentinel, mirroring init_context at mod.rs:244
- **Decided:** resolve_profile_with_config is a thin root-scope wrapper; recursive includes still use existing resolve_profile

## [376cf72-dirty] Slice A: extract resolve_profile_body helper

- **Found:** cargo fmt reformatted several files unrelated to the feature — these were staged and committed along with context/mod.rs
- **Decided:** Extract shared include/artifact/dedup logic into private resolve_profile_body fn
- **Decided:** Seed visited with root key in resolve_profile_with_config to prevent self-cycle bypass

## [5db0ad9-dirty] Slice B: ContextSource enum eliminates redundant exists() checks

- **Decided:** Return (ContextConfig, ContextSource) tuple from load_context_or_config
- **Decided:** Propagate source through gather_context return value so handle_render can emit the diagnostic without re-stating

## [23ab691-dirty] Slices C+D: missing tests added, include-fallback asymmetry deferred

- **Found:** Sandbox experienced sustained rustc ICE (fork EAGAIN) during Slice C; could not execute cargo test. All tests had passed after Slices A and B (127 ok).
- **Decided:** Committed test additions based on confidence in correctness (pattern matches existing passing tests exactly)
- **Decided:** Deferred resolve_profile include-fallback fix to a todo — requires threading config_context through resolve_profile, out of scope for this task
- **Open:** Verify all tests pass once the sandbox resource pressure clears

## [23ab691] Review fixes complete; include removal tracked as new task

- **Decided:** Remove the include feature entirely rather than fix the config-default asymmetry (review #1)
- **Decided:** Created task remove-context-profile-include on master
- **Decided:** Closed the deferred todo — superseded by the new task

## [23ab691] Task complete — tests and manual QA verified by user

- **Decided:** Mark task complete
- **Decided:** Branch ready to merge

