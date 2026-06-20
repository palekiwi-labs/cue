---
status: open
---
# Master Plan: fix/title-yaml-quoting

## Problem

`build_frontmatter_bytes` in `crates/cue/src/add/mod.rs` calls
`serde_yaml::from_str(v)` on every user-supplied string value. When the value
contains `": "` (colon-space), serde_yaml parses it as a YAML mapping rather
than a string. The `unwrap_or_else` fallback is never triggered because the
parse succeeds — it just produces the wrong type. The serialised frontmatter
then contains an unquoted mapping instead of a quoted string:

```yaml
# broken
title: foo: bar

# correct
title: "foo: bar"
```

This is a silent bug — no error at write time, but any reader of `title` gets
an object instead of a string.

## Constraints

- Boolean and integer type coercion must be preserved (`done=true` → `done: true`,
  `count=3` → `count: 3`). Existing test `test_add_frontmatter_type_coercion`
  must continue to pass.
- The fix must cover ALL free-form string frontmatter fields, not just `title`.
  Because all fields funnel through `build_frontmatter_bytes`, a single change
  at that choke point covers the whole surface.

## Approach

In `build_frontmatter_bytes`, after calling `serde_yaml::from_str`, inspect the
parsed result. If it is a `Mapping` or `Sequence`, the user intent was a plain
string; coerce it back to `Value::String`. Scalars (Bool, Int, Float, String,
Null) pass through unchanged.

```rust
let yaml_val: serde_yaml::Value = match serde_yaml::from_str(v) {
    Ok(serde_yaml::Value::Mapping(_)) | Ok(serde_yaml::Value::Sequence(_)) => {
        serde_yaml::Value::String(v.clone())
    }
    Ok(val) => val,
    Err(_) => serde_yaml::Value::String(v.clone()),
};
```

`serde_yaml` will then quote the string automatically in its output whenever
the value contains YAML-significant characters (`: `, `#`, `{`, `[`, etc.).

## Implementation Phases

### Phase 1 — TDD red/green (current)

1. Add failing integration test `test_add_frontmatter_colon_in_string_value`
   in `crates/cue/tests/add.rs`.
2. Apply the fix to `build_frontmatter_bytes`.
3. Run full test suite; all tests must pass.
4. Commit at GREEN.

### Phase 2 — Review and close

5. Spawn diff-reviewer sub-agent for code review.
6. Address any review findings.
7. Fill AC evidence in task file and mark `complete`.

## References

- Task: `.cue/master/task/1781965432-d2f3251/fix-title-yaml-quoting.md`
- Fix target: `crates/cue/src/add/mod.rs:102-115`
- Test file: `crates/cue/tests/add.rs`
