---
status: complete
---

# Plan - Frontmatter Filtering for `mem list`

Add the ability to filter artifacts based on YAML frontmatter values using a simple CLI syntax.

## Foreword
This plan extends the `mem list` command to support a `--filter` flag. This allows users to query
artifacts by their frontmatter metadata, such as `status`, `priority`, or `author`. The
implementation focused on performance and minimal dependencies. Consulted with Sonnet on CLI UX and
implementation strategy before coding.

## Key Decisions
- `--filter` is frontmatter-scoped only (not full JSON field path).
- `--filter` implies frontmatter parsing but does NOT add frontmatter to output (`--frontmatter` still needed for that).
- Operators: `=` (equality), `!=` (inequality), `~=` (substring). Numeric comparisons deferred.
- Missing key: `=` evaluates false, `!=` evaluates true.
- Multiple filters are ANDed.
- RHS type-coerced via `serde_json::from_str` (numbers, booleans, strings).
- Dot notation for nested keys via `get_nested()`.
- `Filter` implements `FromStr` so clap validates expressions at arg-parse time.
- `handle()` args refactored into `ListOptions` struct to satisfy clippy::too_many_arguments.

## Phases

### Phase 1: Filter Logic & Parsing
- [x] Define `FilterOp` enum (`Eq`, `NotEq`, `Contains`).
- [x] Define `Filter` struct in `src/commands/list.rs` with:
    - `path: Vec<String>` (to support dot notation).
    - `op: FilterOp`.
    - `rhs: serde_json::Value` (type-coerced from CLI string).
- [x] Implement `FromStr` for `Filter` to allow `clap` validation and repeatable flags.
- [x] Implement `evaluate_filter(filter, frontmatter)` logic.
    - Support nested key lookups via `get_nested()`.
    - Implement `~=` substring matching for strings.
    - Handle missing keys correctly (Eq=false, NotEq=true).

### Phase 2: CLI Integration
- [x] Update `src/cli.rs` to include `filters: Vec<Filter>` in the `List` command.
- [x] Refactor `handle()` args into `ListOptions` struct.
- [x] Update `src/commands/list.rs`:
    - Integrate `matches_filters` step into the processing pipeline.
    - Works for both standard path output and `--json` output.

### Phase 3: Testing
- [x] Unit tests for `Filter::from_str` (21 unit tests added).
- [x] Unit tests for `evaluate_filter` with various JSON types, nested paths, null frontmatter.
- [x] Integration tests (14 new tests):
    - `mem list --filter "status=todo"` (equality)
    - `mem list --filter "status!=done"` (inequality)
    - `mem list --filter "title~=Meeting"` (substring)
    - `mem list --filter "meta.priority=high"` (nested key)
    - Missing frontmatter excluded by `=`, included by `!=`
    - Multiple filters ANDed
    - Combined with `--json` (no frontmatter field in output)
    - Combined with `--frontmatter` (frontmatter field present in output)

### Phase 4: Documentation
- [x] Update `mem list --help` with filter syntax and examples.
