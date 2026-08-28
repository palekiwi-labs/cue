# Implementation Plan - Artifact Frontmatter Parsing

This plan covers the addition of YAML frontmatter parsing to the `mem list` command and the stabilization of the test suite against environment pollution.

## Phase 1: Preparation & Dependencies
- [x] Add `serde_yaml = "0.9"` to `Cargo.toml`.
- [x] Add `temp-env = "0.3"` to `Cargo.toml` dev-dependencies.
- [x] Verify build and current `mem list` behavior.

## Phase 2: Core Parsing Logic
- [x] Implement `extract_frontmatter_yaml` in `src/commands/list.rs`.
    - [x] Use `BufReader` for efficient, bounded reads.
    - [x] Implement "64-line budget" for the frontmatter block.
    - [x] Implement "early-abort" if the first line is not `---`.
- [x] Test parsing logic with various file inputs (no frontmatter, valid frontmatter, malformed frontmatter, oversized frontmatter).

## Phase 3: Data Model & Integration
- [x] Update `MemFile` struct in `src/commands/list.rs`:
    - [x] Add `frontmatter: Option<serde_json::Value>`.
    - [x] Use `#[serde(skip_serializing_if = "Option::is_none")]`.
- [x] Implement `enrich_frontmatter` function to bridge the parser and the data model.
- [x] Update `handle` function in `src/commands/list.rs`:
    - [x] Add `frontmatter` flag to the CLI argument struct.
    - [x] Logic to imply `json` if `frontmatter` is set.
    - [x] Conditionally call `enrich_frontmatter` during the JSON mapping loop.
- [x] Wire up `cli.rs` and `main.rs` to support the new `--frontmatter` flag.

## Phase 4: Test Stabilization (Environment Isolation)
- [x] Isolate unit tests in `src/config.rs` using `temp_env::with_var_unset`.
- [x] Implement `helpers::mem_cmd()` in `tests/helpers.rs` to isolate subprocesses:
    - [x] Set `MEM_CONFIG_DIR` to a temp directory.
    - [x] Clear `MEM_ARTIFACT_TYPES` and `MEM_IGNORED_TYPES`.
- [x] Refactor all integration tests (`tests/*.rs`) to use `helpers::mem_cmd()`.
- [x] Verify 100% test pass rate (75/75 tests).

## Phase 5: Validation & Performance
- [x] Manual verification with `mem list --frontmatter`.
- [x] Confirm no performance regression in default `mem list` path.
- [ ] (Optional) Benchmark with large artifact sets (1000+ files) to evaluate need for `rayon` parallelization.

## Phase 6: Frontmatter on Create (`mem add -f`)
- [x] Add `parse_frontmatter_field` value parser in `src/cli.rs`.
- [x] Replace `-f` short flag on `--file` with long-only; add `frontmatter: Vec<(String, String)>` field with `-f` short flag to the `Add` variant.
- [x] Update `Commands::Add` destructuring in `src/main.rs` to capture the new `frontmatter` field and pass it to `commands::add::handle`.
- [x] Add `frontmatter: Vec<(String, String)>` parameter to `commands::add::handle` in `src/commands/add.rs`.
- [x] Implement `build_frontmatter_bytes(fields: &[(String, String)]) -> Result<Vec<u8>>` helper inside `add.rs`.
    - Build a `serde_yaml::Mapping` (preserving insertion order) from the pairs.
    - Serialize to YAML string via `serde_yaml::to_string`.
    - Wrap with `---\n` / `---\n` fences and return as bytes.
- [x] Before writing, if `frontmatter` is non-empty, prepend the YAML block to `content`.
- [x] Add integration tests in `tests/add.rs`:
    - Single `-f` pair creates correct YAML fences.
    - Multiple `-f` pairs create a multi-key YAML block.
    - `-f` combined with inline content: frontmatter appears before content.
    - `mem list --frontmatter` round-trips and parses the written fields correctly.
- [x] Verify 100% test pass rate.

## Status: Complete
