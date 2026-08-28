# Project Log

## [616fa6c] [616fa6c] gather_context now uses resolve_scope; all tests pass

Fixed the last remaining call to `get_current_branch()` in the `cue context` command family. `gather_context` in `crates/cue/src/context/mod.rs` now calls `cuelib::head::resolve_scope(&cue_dir)` to derive scope from `.cue/HEAD`, falling back to `master` when HEAD is absent. Two existing tests were updated (they used `.cue/main/` assuming branch-derived scope; now they use `.cue/master/`). Two new integration tests were added to pin the correct behaviour.

- **Found:** gather_context was the only function in the context family still using get_current_branch(); all other subcommands (show, profiles, path, init) already used resolve_scope.
- **Found:** Removing get_current_branch from the gather_context call site also removed it from the import list in context/mod.rs; sanitize_branch_name is still used by parse_artifact_path and resolve_profile.
- **Found:** The two pre-existing clippy warnings in switch.rs (collapsible_if) were present before this change and remain out-of-scope.
- **Decided:** Reorder local variable setup in gather_context so git_root and config are derived before cue_dir, which is needed for resolve_scope.
- **Decided:** Update existing context_render tests to use .cue/master/ (HEAD-absent fallback) instead of .cue/main/ (git-branch-derived scope).

## [32385b8] [32385b8] Branch terminology and dead sanitization removed

Removed all branch terminology from the scope resolution code path and eliminated dead `sanitize_branch_name` calls across 6 source files and 2 test files. The `sanitize_branch_name` function still exists in `cuelib/src/git.rs` as a public API but is no longer called from the cue crate.

- **Found:** sanitize_branch_name (cuelib/src/git.rs:141) only replaces / and \ with -. validate_slug (cuelib/src/head.rs:39) is stricter: rejects multi-segment, .., ., absolute paths.
- **Found:** The context.json @scope:path syntax still works without sanitization for normal single-segment scope names. Only the slash-conversion feature (e.g. feat/test -> feat-test) was removed.
- **Found:** test_log_add_validation tested --task '' and expected 'Scope name cannot be empty' -- now validate_slug catches it first with 'Invalid task slug', which is a better error message anyway.
- **Decided:** Renamed branch_dir/sanitized_branch/branch to scope in context/mod.rs, commands/context.rs.
- **Decided:** Removed sanitize_branch_name calls on HEAD-derived scope in add/mod.rs, log/mod.rs, list/mod.rs, commands/log.rs -- these were guaranteed no-ops since validate_slug at switch time already rejects / and \.
- **Decided:** Added validate_slug to the --task override path in all 4 modules: malformed slugs like 'feat/auth' are now rejected instead of silently transformed to 'feat-auth'.
- **Decided:** Removed sanitize_branch_name from context.json cross-context reference parsing in parse_artifact_path and resolve_profile: scope names in JSON must now use the literal task slug.
- **Decided:** Removed dead `use crate::git;` import from list/mod.rs (sanitize_branch_name was its only consumer).
- **Decided:** Renamed test_add_with_explicit_branch to test_add_with_task_override; added test_add_with_task_override_rejects_invalid_slug to pin the new validate_slug behavior.
- **Open:** sanitize_branch_name in cuelib/src/git.rs is now dead code from the cue crate's perspective. Consider removing it or marking deprecated in a future cleanup.

