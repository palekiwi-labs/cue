---
status: open
refs: .cue/master/task/use-default-context-from-config.md
---
# Plan: Fall back on default context from config

## Problem

When a task scope has no `context.json`, all `cue context` commands fail with
"Context file not found". The config (`cue.json`) already has a `context` field
(`Config.context: ContextConfig`) used as a template by `cue context init`, but
it is never used as a runtime fallback.

## Constraints

- The fallback must only trigger when `context.json` is absent, not on parse errors.
- `resolve_profile` is recursive and scope-agnostic; do not thread a root-only
  fallback parameter through it.
- `handle_path` reports the physical file path — leave it strict.

## Chosen Approach (from Opus consultation)

### 1. New helper: `load_context_or_config`

Add one function in `crates/cue/src/context/mod.rs`:

```rust
pub fn load_context_or_config(
    context_path: &Path,
    config_context: &ContextConfig,
) -> anyhow::Result<ContextConfig> {
    if context_path.exists() {
        load_context_config(context_path)
    } else if !config_context.is_empty() {
        Ok(config_context.clone())
    } else {
        anyhow::bail!("Context file not found: {}", context_path.display());
    }
}
```

Branch on `path.exists()` (not on `Err(_)`) so parse errors on a real file are
never swallowed.

### 2. `gather_context` — use the helper

- Load `root_config` once via `load_context_or_config` at the top.
- Use it for the instructions lookup (replacing the second `load_context_config`
  call at the bottom, which would otherwise hard-error for the same absent file).
- For artifact path resolution (`resolve_profile`), add a thin `resolve_profile_with_config`
  that takes the already-loaded root `ContextConfig` for the root scope, delegates
  its `include`s to the existing `resolve_profile`, and accumulates local artifacts
  directly — so render emits actual artifacts from the config default when no file exists.

### 3. `handle_show` and `handle_profiles` — use the helper

Both already have `config` in scope after `Config::load`. Replace
`load_context_config(&config_path)?` with
`load_context_or_config(&config_path, &config.context)?`.

### 4. Fix error message source

After a fallback, the "Profile not found in {path}" message currently points at a
nonexistent file. Update to say "in config default" when the fallback is active.

### 5. Leave `handle_path` strict

It answers "where is the file" — no physical path exists for the config fallback.

## Key Design Decisions

- Mirror `init_context`'s existing `is_empty()` convention for "is there a usable
  config default" — single consistent policy.
- Do NOT add a fallback parameter to `resolve_profile` — recursive + root-only
  special case = rot.
- Consider adding a stderr note `(no context.json; showing config default)` in
  `handle_show`/`handle_profiles` for UX clarity (optional).
