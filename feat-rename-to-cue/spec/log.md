# Project Log

## [d1a9cf9] rename mem → cue: source code complete

All source code references to the old `mem` name have been replaced with `cue`. 114 tests pass. Committed as d1a9cf9.

- **Found:** context/mod.rs had hardcoded .mem string literals not using config.dir_name — threaded dir_name param through context_json_path, parse_artifact_path, resolve_profile
- **Found:** context test files (context_init, context_path, context_show, context_render) had hardcoded .mem paths that needed updating
- **Found:** All 4 commands files (add, init, list, log/add, log/list) had mem_path local variables to rename
- **Found:** ListOptions.mem_type and AddOptions.mem_type fields renamed to cue_type; CLI enum fields renamed correspondingly
- **Decided:** Thread dir_name through context functions rather than just doing a string replace — this also fixes a pre-existing bug where context ignored the configured dir_name
- **Decided:** Keep .mem/ worktree data migration out of scope (runtime concern, not source concern)
- **Decided:** Rename internal Rust identifiers (mem_type, MemFile, mem_path) to cue-prefixed equivalents for consistency

