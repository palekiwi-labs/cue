---
title: Remove context profile include feature
status: inbox
priority: normal
refs:
- .cue/use-default-context-from-config/todo/1784698417-23ab691/config-default-include-fallback.md
- .cue/master/spec/index.md
---
# Remove context profile include feature

The `include` field in context profiles allows one profile to pull in
artifacts from another scope. It was never used in practice and adds
significant complexity: recursive resolution, cycle detection, diamond
dedup, and the unresolved asymmetry where the config-default fallback
does not propagate to included scopes.

Remove the feature entirely.

## Scope

- Remove `include` from `ContextProfile` struct
- Delete `resolve_profile` and `resolve_profile_body` (the entire
  recursive resolution machinery exists solely to support includes)
- Simplify `resolve_profile_with_config` to iterate `artifacts` only
- Remove all cycle-detection and dedup logic that was there for includes
- Remove `visited: HashSet` from all call sites
- Delete tests that cover include behaviour:
  `test_resolve_profile_include_formats`, `test_resolve_profile_cycle`,
  `test_resolve_profile_diamond_dependency`,
  `test_context_render_config_default_with_include`
- Update integration tests and any documentation that references `include`

## Acceptance Criteria

1. **`include` field removed from schema and serialization.**
   - Verify by: `cargo test` passes; `grep -r '"include"' crates/` returns
     no hits in production code.
   - Evidence:

2. **All tests pass.**
   - Verify by: `cargo test`
   - Evidence:

3. **No functional regression on artifact resolution without includes.**
   - Verify by: existing artifact and glob tests still pass.
   - Evidence:
