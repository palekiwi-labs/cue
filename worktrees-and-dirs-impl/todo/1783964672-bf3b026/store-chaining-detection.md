---
status: open
priority: low
refs:
- cue/master/spec/index.md
- cue/worktrees-and-dirs-impl/plan/index.md
---
# Deferred: STORE chaining detection

The spec requires that if a STORE file's target itself contains another STORE
file, cue must error loudly ("chaining is not supported").

This validation was deferred from the initial implementation.

## What to implement

In `resolve_store`, after reading and validating the STORE target path, add:

```rust
if target.join("STORE").exists() {
    return Err(anyhow!("STORE chaining is not supported: \
        {} points to {}, which also contains a STORE file",
        cue_dir.display(), target.display()));
}
```

## Why deferred

Low risk of being triggered in practice. The rest of the implementation is
not blocked by this check. Adding it later is a one-line change in
`resolve_store`.
