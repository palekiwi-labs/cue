---
status: complete
---
## Foreword

This plan executes Slice 1 of `plan/1782644149-dab157e/curator-improvements.md`.

**Goal:** Add `project_dir: String` and `harness: String` to all four
`acuity-schema` event structs, add accessors to `AcuityEvent`, update all
tests, and regenerate `types.ts`.

**Branch:** `feat/curator-improvements-schema`

**Task:** `task/1782644149-dab157e/schema-enrichment.md`

**Context from current `lib.rs`:**
- `SessionIdle` already has `project_dir`; needs `harness` added.
- `AgentTurnCompleted`, `ToolCallRequested`, `ToolCallCompleted` need both
  `project_dir` and `harness` added.
- Existing accessors `session_id()` and `turn_id()` are the pattern to follow
  for the two new accessors.
- All fixture constructors in `tests` module must be updated with new fields.
- Raw-JSON strings in deserialization tests must be updated.
- Forward-compat unknown-fields test does not need changing.

**Exit condition:** `cargo test -p acuity-schema` green, `types.ts` regenerated.

---

## Steps

- [x] 1. Add `harness: String` to `SessionIdle` struct (`lib.rs:20-24`)
- [x] 2. Add `project_dir: String` and `harness: String` to `AgentTurnCompleted`
- [x] 3. Add `project_dir: String` and `harness: String` to `ToolCallRequested`
- [x] 4. Add `project_dir: String` and `harness: String` to `ToolCallCompleted`
- [x] 5. Add `project_dir(&self) -> &str` accessor to `AcuityEvent` impl
- [x] 6. Add `harness(&self) -> &str` accessor to `AcuityEvent` impl
- [x] 7. Update all four fixture constructors in `tests` module with new fields
- [x] 8. Update raw-JSON strings in deserialization tests to include new fields
- [x] 9. Add accessor tests for `project_dir()` and `harness()` across all variants
- [x] 10. Run `cargo test -p acuity-schema` — must be green
- [x] 11. Run `cargo clippy -p acuity-schema -- -D warnings` — must be clean
- [x] 12. Commit: `feat(acuity-schema): add project_dir and harness to all event types`
- [x] 13. Regenerate `types.ts` via `nix run .#update-types` and commit to cue-plugins (ab481a4 in cue-plugins)
- [x] 14. Update task status to `in-progress`; fill Evidence cells after green run
