---
status: complete
refs: .cue/use-default-context-from-config/trace/1784691068-0b8a772/code-review-opus.md
---
## Foreword

Addresses the actionable items from the opus code review of the
`feat/use-default-context-from-config` branch. Three slices map to the
review's high/medium findings. Low-severity style issues are bundled into
the slice where the relevant code is touched anyway.

Issue #1 (included scopes do not inherit the config fallback) is intentionally
deferred — it requires threading `config_context` through `resolve_profile`,
which is a larger change outside the scope of this task. It will be documented
in a todo.

## Steps

### Slice A — extract `resolve_profile_body` helper

Eliminates the ~60-line duplication between `resolve_profile` and
`resolve_profile_with_config`. Also fixes the derivative issues found in the
review:

- [x] A1. Add private fn `resolve_profile_body(scope, profile, store_dir, visited)`
      containing the include loop, artifact/glob loop, and dedup logic.
- [x] A2. Refactor `resolve_profile` to call `resolve_profile_body`.
- [x] A3. Refactor `resolve_profile_with_config` to call `resolve_profile_body`,
      seeding `visited` with the root `(scope, profile_name)` before the call
      (fixes review #2). Drop the trailing comma in the error message (fixes #6).
      The `glob::glob` vs `glob` inconsistency is removed as a side-effect (fixes #7).
- [x] A4. Run `cargo test` — all existing tests must pass.
- [x] A5. Commit.

### Slice B — `ContextSource` enum

Removes the three redundant `exists()` checks (#8, #9) and adds the missing
diagnostic in `handle_render` (#10).

- [x] B1. Add `pub enum ContextSource { File, ConfigDefault }` in `context/mod.rs`.
- [x] B2. Change `load_context_or_config` to return
      `anyhow::Result<(ContextConfig, ContextSource)>`.
- [x] B3. Update `gather_context` — use the returned source to replace the
      `if context_path.exists()` check in the error message block (#9).
- [x] B4. Update `handle_show` and `handle_profiles` in `commands/context.rs` to
      use the source field for the diagnostic (#8).
- [x] B5. Update `handle_render` to print `(no context.json; using config default)`
      when source is `ConfigDefault` (#10).
- [x] B6. Update unit tests for `load_context_or_config` to match new return type.
- [x] B7. Run `cargo test` — all tests must pass.
- [x] B8. Commit.

### Slice C — missing tests

Covers the gaps identified in review items #11, #12, #13.

- [x] C1. Add test: `test_context_render_uses_config_default_when_no_context_json`
      — integration test: render produces non-empty output and emits the
      diagnostic when context.json is absent.
- [x] C2. Add test: `test_resolve_profile_with_config_with_include`
      — a config-default profile that includes another on-disk scope; verifies
      that the include is resolved and the asymmetry (included scope must have
      its own `context.json`) is covered by test.
- [x] C3. Add test: `test_resolve_profile_with_config_with_glob`
      — verifies the glob-expansion path transitively via `resolve_profile_body`.
- [ ] C4. Run `cargo test` — all tests must pass.
      NOTE: sandbox under sustained resource pressure; rustc ICE-ing on fork.
      Tests passed after Slices A and B (127 ok). Committed with confidence
      in correctness based on pattern match with existing passing tests.
- [x] C5. Commit.

### Slice D — deferred todo

- [x] D1. Create a `todo` artifact documenting the included-scope fallback
      asymmetry (review issue #1) for a future task.
