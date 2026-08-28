---
title: 'Schema enrichment: project_dir + harness on all event types'
status: complete
priority: high
branch: feat/curator-improvements-schema
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
| 1 | `project_dir: String` and `harness: String` fields present on all four event structs | code review | commit e19cf22 |
| 2 | `AcuityEvent::project_dir()` and `harness()` accessors implemented | code review | commit e19cf22 |
| 3 | All four serde round-trip tests pass with new fields | `cargo test -p acuity-schema` | 20/20 green, commit e19cf22 |
| 4 | Raw-JSON deserialization tests assert new fields present | `cargo test -p acuity-schema` | 20/20 green, commit e19cf22 |
| 5 | `types.ts` regenerated and committed to cue-plugins | `nix run .#update-types` + manual diff | ab481a4 in cue-plugins |
