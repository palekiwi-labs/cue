# Project Log

## [7b0d069] Phase 1: CLI Scaffolding complete

- **Found:** Added Context variant and ContextCommands enum to cli.rs
- **Decided:** Implemented stubs for all context subcommands (init, show, profiles, render) to allow cargo build to pass

## [38bcd20] Phase 2: Shared Types complete

- **Found:** Defined ContextProfile and ContextConfig in commands/context/mod.rs
- **Decided:** Added load_context_config and context_json_path helpers; verified via unit tests

## [292a816] Phase 3: mem context init complete

- **Found:** Implemented mem context init with spec/ file auto-population
- **Decided:** Standardized on slash-to-dash branch sanitization; added integration tests for init and --force

## [9e8084e] Phase 4: mem context show and profiles complete

- **Found:** Implemented show and profiles commands for mem context
- **Decided:** Verified JSON pretty-printing and sorted profile listing; added integration tests

## [457b240] Phase 5: Path Resolution & Security complete

- **Found:** Implemented parse_artifact_path with support for ./ and @branch notations
- **Decided:** Enforced security rules (no .., no absolute paths, sanitized branch names); verified with unit tests

## [469d968] Phase 6: Include Resolution & Cycle Detection complete

- **Found:** Implemented resolve_profile with DFS recursion and HashSet-based cycle detection
- **Decided:** Added deduplication (first occurrence wins); verified with unit tests for cycles

## [80de863] Phases 7-9: mem context render and config additions complete

- **Found:** Implemented full render logic including recursive includes, XML-wrapped artifacts, and git diff with exclude patterns
- **Decided:** Added diff_exclude_paths to mem.json config; verified with full test suite

## [bbe60ec] Security: Path Resolution Hardened

- **Found:** Identified absolute path bypass via prefixes in parse_artifact_path; identified crude '..' check flagging valid filenames
- **Decided:** Moved absolute check after prefix stripping; used Path::components for robust traversal check; verified with bypass tests

## [b20cfa6] Logic: Diamond Dependencies Supported

- **Found:** Identified that sticky 'visited' set blocked valid DAGs
- **Decided:** Implemented backtracking in resolve_profile by removing keys from visited after recursion; verified with diamond dependency test

## [dcaef11] Phase 11: Security & Logic Hardening complete

- **Found:** Implemented backtracking for diamond dependencies; added directory checks in render; centralized branch sanitization; fixed git diff -- separator conflict
- **Decided:** Applied all recommendations from consultant-gemini review; verified with updated unit and integration tests

## [dcaef11-dirty] Relaxed artifact path parsing and improved branch handling

Refactored `parse_artifact_path` to relax security constraints and improve usability.
The function now allows:
1. Relative paths starting with `.` or `..`.
2. Simple paths without prefixes (e.g., `spec/plan.md`), defaulting to the current branch context.
3. Colons in branch names by using `rsplit_once(':')` to split the branch from the path in `@branch:path` references.

Security:
- Path traversal is no longer explicitly blocked since the tool runs in a trusted/sandboxed environment.
- Root/absolute paths are still rejected to prevent escaping the `.mem` storage context incorrectly via `PathBuf::join`.

Tests:
- Updated unit tests to verify the new behaviors and ensure no regressions in cross-branch logic.
- Verified all integration tests pass.

- **Found:** PathBuf::join overwrites the base if the second path has a root.
- **Found:** Canonicalize requires the file to exist on disk.
- **Decided:** Allow relative and simple paths in artifact references.
- **Decided:** Use rsplit_once(':') for cross-branch references to support colons in branch names.
- **Decided:** Continue rejecting absolute paths for base-path overwrite protection.

## [9c223f1] Refactor artifact path parsing for usability

Refactored `parse_artifact_path` to support relative paths (`.`, `..`) and simple paths without prefixes, improving ergonomics for manual context configuration.

The refactor also improves branch handling by using `rsplit_once(':')` for cross-branch references, allowing branch names to contain colons.

Security remains managed by rejecting absolute/root paths to prevent escaping the repository context during `PathBuf::join`.

Verification:
- Unit tests updated to cover new formats and colon-in-branch-name cases.
- Full test suite passed.

- **Found:** Existing constraints were too restrictive for dev usability (e.g. neovim resolution).
- **Decided:** Allow path traversal and simple paths in artifact configuration.
- **Decided:** Support colons in branch names via rsplit_once logic.

## [fd8000d] Refactor context subcommand and decouple core logic

Moved core context types and logic to src/context/mod.rs. Simplified CLI handlers to act as thin wrappers. Inlined unit tests into the core module.

- **Decided:** Move core logic to src/context/mod.rs
- **Decided:** Inlined unit tests in core module
- **Decided:** Decoupled gathering from rendering

## [56b191d] Consolidated context command handlers and refactored core logic

Refactored the `context` command to improve separation of concerns and simplify the codebase.
- Moved `init_context` logic from CLI handler to `src/context/mod.rs`.
- Consolidated multiple files in `src/commands/context/` into a single `src/commands/context.rs`.
- Standardized CLI handlers to be thin wrappers around core module functions.
- Applied project-wide formatting via `cargo fmt`.

- **Found:** Individual subcommand handlers were thin enough to be consolidated without losing clarity.
- **Decided:** Consolidated CLI handlers into a single file for better maintainability.
- **Decided:** Moved core initialization logic to the core context module to allow for programmatic use.

## [df9443e] Add 'mem context path' subcommand

Implemented `mem context path` and `mem context path --all` to help users locate `context.json` files.
- Added `Path` subcommand to `ContextCommands` in `src/cli.rs`.
- Implemented `handle_path` in `src/commands/context.rs` to resolve and print paths.
- Added integration tests in `tests/context_path.rs`.
- Verified all tests pass.

- **Found:** Current branch context path resolution logic already exists in 'src/context/mod.rs' via 'context_json_path'.
- **Decided:** Use 'mem context path' instead of adding to 'mem list' to maintain separation between config and artifacts.

## [606f620] Harden security and fix directory context

- **Decided:** Standardize on git_root for context path resolution.
- **Decided:** Implement boundary checks for artifacts in gather_context using canonicalize and starts_with.
- **Decided:** Switch to split_once(':') for cross-branch references.

## [98a061a] Implement configurable context templates and glob support

- **Decided:** Support 'context' templates in mem.json (global/local).
- **Decided:** Implement glob expansion for artifacts at render time.
- **Decided:** Preserve glob patterns in context.json during init when a template is used.
- **Decided:** Maintain security via path boundary checks in gather_context.

## [6f8faf6] Isolate integration tests from local configuration

Introduced MEM_CONFIG_DIR environment variable to allow overriding the global configuration directory. Updated the integration test suite to use a new TestEnv helper that isolates tests by setting MEM_CONFIG_DIR to a temporary directory. This prevents local developer configurations from leaking into and breaking the tests. Refactored all context-related integration tests to use this helper.

- **Found:** Integration tests were leaking user's global mem.json configuration.
- **Found:** Redirecting HOME is less robust than a dedicated config directory override.
- **Decided:** Use MEM_CONFIG_DIR environment variable for config isolation in tests.
- **Decided:** Refactor integration tests to use a TestEnv helper to ensure clean state.

## [32f2a48-dirty] Fix directory artifact error in 'mem context render'

Modified `resolve_profile` to filter out directories during glob expansion and added an explicit file check in `gather_context` to prevent OS error 21 when encountering directory matches. Added a unit test to verify that recursive globs like `**/*` do not include directory paths in the artifact list.

- **Found:** Recursive globs matching directories cause `std::fs::read_to_string` to fail with 'Is a directory' (os error 21).
- **Decided:** Filter out directories during glob expansion in `resolve_profile`
- **Decided:** Add explicit file check in `gather_context` before reading content

## [dea541e] Secure base branch resolution implemented

- **Found:** Arbitrary shell commands in local config files are a major security risk (RCE).
- **Decided:** Removed 'base_branch_cmd' config option for security reasons.
- **Decided:** Use '@base' sigil for dynamic base branch resolution in diffs.
- **Decided:** Require explicit '--base' CLI flag when '@base' is used to maintain security boundary.
- **Decided:** Updated documentation and added integration tests to verify successful resolution and failure when the flag is missing.

## [dea541e] Secure base branch resolution implemented

Removed the insecure 'base_branch_cmd' from configuration to prevent RCE. Introduced the '@base' sigil in the context diff field, which requires an explicit '--base <branch>' CLI flag to resolve. Updated documentation and added integration tests to verify successful resolution and failure when the flag is missing.

- **Found:** Arbitrary shell commands in local config files are a major security risk (RCE)
- **Decided:** Remove 'base_branch_cmd' config option for security reasons
- **Decided:** Use '@base' sigil for dynamic base branch resolution in diffs
- **Decided:** Require explicit '--base' CLI flag when '@base' is used to maintain security boundary

## [13ce90e] Slice 1: @base support in artifacts complete

Extended 'parse_artifact_path' to support the '@base' sigil. When encountered, it resolves to the branch provided via the '--base' CLI flag (sanitized for directory lookup). Added unit tests to verify resolution and error handling when the flag is missing.

- **Found:** parse_artifact_path needed base_branch context to resolve @base dynamically
- **Decided:** Support @base in artifacts field of context.json
- **Decided:** Require --base flag when @base is used in artifact paths

## [e9438f1] Slice 2: @base support in includes complete

Updated 'resolve_profile' to handle the '@base' sigil in 'include' paths. It dynamically resolves to the branch provided via '--base'. Also propagated the 'base_branch' through the recursion and into 'parse_artifact_path' so that included profiles can also resolve their own '@base' references. Added unit tests for diamond dependencies and base branch includes.

- **Found:** resolve_profile required base_branch context for dynamic include resolution
- **Decided:** Support @base in include field of context.json
- **Decided:** Propagate base_branch through recursive profile resolution

## [a7c301f] Slice 4: Integration tests for @base complete

Added comprehensive integration tests for the '@base' sigil in 'artifacts' and 'include' fields of 'context.json'. Verified that it correctly resolves when '--base' is provided and fails with a clear error message when it is omitted. Verified cross-branch resolution and deduplication.

- **Found:** Integration tests confirmed correct resolution across branches for all context fields
- **Decided:** Validate @base support via integration tests

## [b6bc83b-dirty] MEM_BASE_BRANCH_FILE support implemented

Implemented support for the MEM_BASE_BRANCH_FILE environment variable to resolve the @base sigil in context.json. This provides a secure way to automate base branch resolution without hardcoding or executing arbitrary commands.

Key changes:
- Added `resolve_base_branch_name` with flag -> env resolution logic.
- Implemented robust sanitization for branch names read from files to prevent exfiltration loops.
- Updated `gather_context` and recursive resolution to propagate the base branch.
- Added unit and integration tests covering resolution priority and error handling.
- Updated feature specification documentation.

- **Found:** Shell execution in configuration is a major RCE risk.
- **Found:** AI agents can inadvertently leak secrets if error messages contain data read from attacker-controlled file paths.
- **Decided:** Reject MEM_BASE_BRANCH_CMD in favor of MEM_BASE_BRANCH_FILE for security.
- **Decided:** Use environment variable instead of mem.json setting to maintain trust boundaries.
- **Decided:** Prioritize --base CLI flag over MEM_BASE_BRANCH_FILE.

## [1ed57a2] Pivot to strictly file-based context system complete

Removed dynamic @base sigil resolution and git diff generation from 'mem context'. The system is now strictly file-based, ensuring 100% predictable and reproducible context streams. Explicit files must now be used for diffs or cross-branch references.

- Removed @base support in artifacts, includes, and diffs.
- Removed MEM_BASE_BRANCH_FILE and --base flag.
- Removed 'diff' field from context.json and 'diff_exclude_paths' from mem.json.
- Updated documentation and tests to reflect the static file-only philosophy.

- **Found:** Dynamic resolution introduces temporal coupling and breaks cross-session reproducibility
- **Decided:** Deprecate dynamic sigils in favor of explicit file artifacts
- **Decided:** Move git diff responsibility to the user/agent (save to file)

## [f0d9eae] Simplified include syntax implemented

Refactored 'resolve_profile' to support a more ergonomic 'include' syntax. The '@' sigil is now optional, and users can include local profiles using the ':profile' shorthand. Updated documentation and added unit tests to cover all include formats.

- **Found:** The '@' prefix in 'include' was redundant since includes are always cross-branch or cross-profile by definition.
- **Decided:** Make '@' sigil optional in 'include' field of context.json
- **Decided:** Support ':profile' for local profile inclusion within the same branch

## [eb2c282] Local profile inclusion shorthand removed

Removed the ':local-profile' shorthand for including profiles from the same branch in 'context.json'. The '@' sigil remains optional for cross-branch includes, but the system now focuses on clear cross-branch or cross-profile references. Updated unit tests and specification documentation accordingly.

- **Decided:** Remove local profile inclusion shorthand (':' prefix) from 'resolve_profile'
- **Decided:** Maintain optional '@' sigil for cross-branch includes

## [f26675a] fix: sanitize branch names in context references

I fixed an issue where `mem context` failed to resolve branch names in `include` and cross-branch `artifacts` when they contained characters like `/` or `\` that required sanitization on the filesystem.

I updated `resolve_profile` and `parse_artifact_path` to apply `sanitize_branch_name` to the branch component of references. I also relaxed the security check in `parse_artifact_path` that previously rejected slashes in branch names, as they are now safely sanitized into dashes.

Added unit tests to verify that:
- `@branch/with/slash:path` resolves to `.mem/branch-with-slash/path`
- `include: ["feat/test"]` resolves to `.mem/feat-test/context.json`

All unit and integration tests passed.

- **Found:** Context resolution failed when using branch names with slashes because they weren't sanitized before filesystem lookup.
- **Decided:** Sanitize branch names in all context references (include and artifacts)
- **Decided:** Relax slash restriction in parse_artifact_path to allow natural branch names

## [f26675a] Exploration of Figment configuration loader

The user wants to support nested environment variables in the configuration loader using the Figment crate, specifically using the `__` separator (e.g., `MEM_CONTEXT__DEFAULT__INSTRUCTIONS`).

Findings:
- `src/config.rs` currently uses `Env::prefixed("MEM_")` but does not call `.split("__")`.
- `Cargo.toml` already includes `figment` with `env` and `json` features.
- `Config::load` is used across various commands.

Proposed Plan:
1. Update `src/config.rs` to include `.split("__")` in the `Figment` builder.
2. Add a test case in `src/config.rs` to verify that nested environment variables are correctly parsed into the `Config` struct.
3. Consider cleaning up the `HOME` / `MEM_CONFIG_DIR` logic to be more robust (as suggested by a trace found during exploration).

## [f26675a-dirty] Support nested environment variables in config

Supported nested environment variables in the configuration loader using the `__` separator.

Changes:
- Modified `src/config.rs` to use `.split("__")` on the `Env` provider in `Config::load`.
- Added `dirs` crate to `Cargo.toml` for more robust home directory resolution.
- Updated `Config::load` to use `dirs::home_dir()` instead of `std::env::var("HOME")`.
- Added a unit test `test_nested_env_override` in `src/config.rs` to verify that `MEM_CONTEXT__DEFAULT__INSTRUCTIONS` correctly overrides the `instructions` field in the `default` profile.

Results:
- Nested environment variables now correctly map to the configuration structure.
- Configuration loading is more robust regarding platform-specific home directories.
- All tests pass.

- **Found:** The project lacks nested env var support in its Figment config loader
- **Found:** Previous research suggested HOME handling could be improved
- **Decided:** Use '__' as separator for nested env vars
- **Decided:** Use 'dirs' crate for home directory resolution

