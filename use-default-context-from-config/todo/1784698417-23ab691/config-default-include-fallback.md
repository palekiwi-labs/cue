---
status: closed
priority: low
refs: .cue/use-default-context-from-config/trace/1784691068-0b8a772/code-review-opus.md
---
# Config-default fallback does not propagate to included scopes

When a scope has no `context.json` and the profile is resolved from
`config.context`, any `include` entries in that profile delegate to
`resolve_profile`, which reads `context.json` from disk for the
included scope. If the included scope also lacks `context.json`, the
fallback is not consulted — `resolve_profile` warns and returns empty.

This means the fallback applies only to the root scope, not to scopes
that are pulled in via `include`. The asymmetry is documented in
`resolve_profile_with_config`'s doc comment but is not yet addressed
in code.

## To fix

Thread `config_context: &ContextConfig` through `resolve_profile` so
that each scope can fall back to the config default when its own
`context.json` is absent. This would require changing the public
signature of `resolve_profile` and updating all callers.

Source: opus code review, item #1
`crates/cue/src/context/mod.rs`

Superseded by task: `.cue/master/task/remove-context-profile-include.md`
The decision is to remove `include` entirely rather than fix the asymmetry.
