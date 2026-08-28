---
title: Fall back on default context from config
priority: critical
status: complete
branch:
  - feat/use-default-context-from-config
---
If the context does not contain its own `context.json`, let `cue context`
commands use the defaults from config, i.e. `cue context render` should not
print empty when `context.json` is missing in the context, it should default to
a default context if specified in the config

## Acceptance Criteria

1. **Tests pass.**
   - Verify by: `cargo test`
   - Evidence: all tests pass (confirmed by user, 2026-07-22)

2. **Manual QA passed.**
   - Verify by: human attestation
   - Evidence: confirmed by user, 2026-07-22

