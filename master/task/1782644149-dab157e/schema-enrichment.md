---
title: 'Schema enrichment: project_dir + harness on all event types'
status: open
priority: high
---
# Schema enrichment: project_dir + harness on all event types

Add `project_dir` and `harness` to all four `acuity-schema` event structs and
regenerate the TypeScript types.

## Source

- spec: `spec/1782644149-dab157e/curator-improvements.md` (F1)
- plan: `plan/1782644149-dab157e/curator-improvements.md` (Slice 1)

## Acceptance Criteria

| # | Criterion | Verify by | Evidence |
|---|-----------|-----------|----------|
| 1 | `project_dir: String` and `harness: String` fields present on all four event structs | code review | |
| 2 | `AcuityEvent::project_dir()` and `harness()` accessors implemented | code review | |
| 3 | All four serde round-trip tests pass with new fields | `cargo test -p acuity-schema` | |
| 4 | Raw-JSON deserialization tests assert new fields present | `cargo test -p acuity-schema` | |
| 5 | `types.ts` regenerated and committed to cue-plugins | `nix run .#update-types` + manual diff | |
