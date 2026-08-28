# Code Review: Fall back on default context from config

Reviewer: claude-opus (diff-reviewer-opus)
Branch: feat/use-default-context-from-config
Merge base: 1d5686de22fd23cd519368811b8a253a7a8482e2

## Summary

The change introduces `load_context_or_config` and `resolve_profile_with_config` to let the
`cue context` commands (`show`, `profiles`, `render`) fall back to the `config.context` default
when a scope's `context.json` is absent. The implementation is correct for the happy path and is
well-tested at the unit level. However, there is a significant amount of code duplication, some
TOCTOU / redundant `exists()` checks, and a couple of edge-case correctness gaps worth addressing
before merge.

---

## Correctness and Edge Cases

### 1. (high) `resolve_profile_with_config` silently ignores the fallback for included scopes

`crates/cue/src/context/mod.rs:186-248`

The root profile is resolved from the passed-in `root_config`, but any `include` directives are
delegated to `resolve_profile` (line 212), which reads `context.json` from disk directly
(`crates/cue/src/context/mod.rs:106`). If an included scope also lacks a `context.json`,
`resolve_profile` emits `"Warning: Could not load context for scope ..., skipping"` and returns
empty — it never consults `config.context`.

This is arguably acceptable (the doc comment at lines 182-185 explicitly calls it out), but it
produces an asymmetry: the root scope falls back to config defaults, while included scopes do not.
Consider whether the fallback should also apply to includes, or at minimum document this limitation
in the user-facing spec, not just the code comment.

### 2. (medium) `resolve_profile_with_config` does not seed `visited` with the root scope

`crates/cue/src/context/mod.rs:197`

`resolve_profile` guards against cycles by inserting `(scope, profile_name)` into `visited` before
recursing (lines 95-103). In `resolve_profile_with_config`, the root `(scope, profile_name)` is
never inserted into `visited`. A self-referential include in the config default will not be caught
at the root boundary.

Recommend seeding `visited` with `(scope.to_string(), profile_name.to_string())` before the include
loop.

### 3. (medium) Divergent glob-detection logic could drift

`crates/cue/src/context/mod.rs:219` vs `:149`

The glob heuristic `art.contains('*') || art.contains('?') || art.contains('[')` is duplicated
verbatim. Any future fix to one (e.g. handling `{...}` brace expansion) will silently miss the
other.

### 4. (low) `bail!` vs `if/else` — unreachable final branch style

`crates/cue/src/context/mod.rs:44-52`

The `else { bail! }` after two returning branches is fine, but slightly cleaner as early returns.

---

## Code Quality and Rust Idioms

### 5. (high) Substantial duplication between `resolve_profile` and `resolve_profile_with_config`

`crates/cue/src/context/mod.rs:186-248`

Roughly 60 lines (the include-resolution loop, the artifact/glob loop, and the dedup loop) are
copy-pasted from `resolve_profile` (lines 129-179). The only real difference is where the root
profile comes from.

Suggested refactor — extract `resolve_profile_body(scope, profile, store_dir, visited)` and have
both functions call it. This also fixes issues #2 and #3 by construction.

### 6. (low) Trailing comma inside `anyhow::anyhow!` args

`crates/cue/src/context/mod.rs:193`

```rust
anyhow::anyhow!("Profile '{}' not found in config default", profile_name,)
```

Drop the trailing comma.

### 7. (low) `glob::glob` vs `glob` inconsistency

`crates/cue/src/context/mod.rs:221` uses the fully-qualified `glob::glob`, whereas `resolve_profile`
at line 151 uses the imported `glob`. The refactor from #5 removes this inconsistency.

---

## Error Handling

### 8. (medium) TOCTOU and redundant `exists()` checks in the command handlers

`crates/cue/src/commands/context.rs:40-43` and `:57-60`

Both `handle_show` and `handle_profiles` call `config_path.exists()` again after
`load_context_or_config` has already performed its own `exists()` check. This is a double stat
and a potential TOCTOU window.

Suggested fix — return an enum from `load_context_or_config`:

```rust
pub enum ContextSource { File, ConfigDefault }

pub fn load_context_or_config(
    context_path: &Path,
    config_context: &ContextConfig,
) -> anyhow::Result<(ContextConfig, ContextSource)>
```

### 9. (medium) `gather_context` re-checks `context_path.exists()` for error messaging

`crates/cue/src/context/mod.rs:299-306`

Same stat-duplication pattern. The `ContextSource` approach from #8 unifies all three call sites.

### 10. (low) `handle_render` cannot emit the "showing config default" diagnostic

`crates/cue/src/commands/context.rs:71-97`

`handle_show` and `handle_profiles` print `(no context.json; showing config default)` but
`handle_render` does not. A user rendering from a config default gets no indication of where the
content came from.

---

## Test Coverage

### 11. (medium) No test for fallback through `gather_context` / `render`

The headline requirement is that `cue context render` "should not print empty when `context.json`
is missing." There is no integration test exercising `gather_context` with an absent `context.json`
and a populated `config.context`. An end-to-end test asserting non-empty render output from a
config default would directly validate the feature.

### 12. (low) No test for `resolve_profile_with_config` with `include` resolution

`test_resolve_profile_with_config_returns_artifacts` only covers a profile with `artifacts` and
empty `include`. The include-delegation path (lines 199-214) — the subtle, asymmetric part from
issue #1 — is untested.

### 13. (low) No test for the glob branch in `resolve_profile_with_config`

The glob-expansion block (lines 219-232) is duplicated but has no dedicated test. The refactor
from #5 covers this transitively; otherwise add one.

---

## Overall Assessment

The feature works and is unit-tested. The two most important concerns are:

- Duplication (#5) between `resolve_profile` and `resolve_profile_with_config`. The extract-helper
  refactor resolves it cleanly and simultaneously fixes the missing cycle-seed (#2) and glob-drift
  (#3) issues.
- Repeated `exists()` disambiguation across three call sites (#8, #9), best solved by returning
  a `ContextSource` from `load_context_or_config`.

Neither is a correctness bug on the happy path (severities are high/medium, not critical). The
end-to-end render test (#11) should be added to directly validate the headline requirement.
The included-scope fallback asymmetry (#1) should be clarified in the spec if not addressed in
code.
