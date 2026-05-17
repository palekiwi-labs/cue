# Implementation Plan: `mem context`

## Phase 1 — CLI Scaffolding

Lay out the full command structure with stubs — argument parsing and dispatch only,
no logic. Goal: `cargo build` passes before any real implementation.

- [ ] Add `Context { command: ContextCommands }` variant to `Commands` in `cli.rs`
- [ ] Define `ContextCommands` enum in `cli.rs`:
  - `Init { force: bool }`
  - `Show`
  - `Profiles`
  - `Render { profile: Option<String> }`
- [ ] Add `-p / --profile` flag to `Render` (default: `"default"`)
- [ ] Create `src/commands/context/` with stub modules:
  - `mod.rs` — dispatch + shared types placeholder
  - `init.rs` — stub `handle()`
  - `show.rs` — stub `handle()`
  - `profiles.rs` — stub `handle()`
  - `render.rs` — stub `handle()`
- [ ] Add `pub mod context;` to `src/commands/mod.rs`
- [ ] Add `Commands::Context` arm to match block in `src/main.rs`
- [ ] Verify `cargo build` compiles cleanly

## Phase 2 — Shared Types

Define the data model shared by all subcommands.

- [ ] Define `ContextProfile` (serde `Deserialize` + `Serialize`, `Default`):
  - `artifacts: Vec<String>` (serde default = empty vec)
  - `diff: Option<String>`
  - `include: Vec<String>` (serde default = empty vec)
- [ ] Define `type ContextConfig = HashMap<String, ContextProfile>`
- [ ] Add `load_context_config(path: &Path) -> Result<ContextConfig>` helper
- [ ] Add `context_json_path(mem_path: &Path, branch_dir: &str) -> PathBuf` helper
- [ ] Unit tests: deserialise full schema, partial schema, unknown fields tolerated

## Phase 3 — `mem context init`

- [ ] Scan `.mem/<branch>/spec/` for all existing files (sorted)
- [ ] Build `"./spec/<filename>"` artifact path strings
- [ ] Serialise a minimal `ContextConfig` with a single `"default"` profile
- [ ] Write `context.json`; error if it exists and `--force` not set
- [ ] Print created path to stdout (consistent with `mem add` output style)
- [ ] Integration test: creates `context.json` with spec/ files auto-populated
- [ ] Integration test: `--force` overwrites existing file
- [ ] Integration test: fails without `--force` when file exists
- [ ] Integration test: empty spec/ produces `"artifacts": []`

## Phase 4 — `mem context show` and `mem context profiles`

- [ ] `show.rs`: load `context.json`, `serde_json::to_string_pretty`, print to stdout
- [ ] `profiles.rs`: load `ContextConfig`, collect keys, sort, print one per line
- [ ] Integration test: `show` round-trips valid pretty-printed JSON
- [ ] Integration test: `profiles` lists correct profile names, sorted
- [ ] Integration test: both commands error clearly when `context.json` missing

## Phase 5 — `mem context render` — Path Resolution & Security

- [ ] Implement `parse_artifact_path(raw: &str, current_branch_dir: &str, mem_path: &Path)`
  - `./...` → validate + resolve to absolute path under `.mem/<current-branch>/`
  - `@branch:path` → validate branch name + path, resolve to `.mem/<branch>/path`
  - Error on unrecognised format
- [ ] Security validation (shared with both notations):
  - Reject any `..` path components
  - Reject root-anchored paths
  - Reject raw slashes in branch component of sigil
- [ ] Unit tests: current-branch resolution, cross-branch resolution
- [ ] Unit tests: `..` rejected, absolute path rejected, malformed sigil rejected

## Phase 6 — `mem context render` — Include Resolution & Cycle Detection

- [ ] Implement `resolve_profile(branch_dir, profile_name, mem_path, visited) -> Result<Vec<PathBuf>>`
  - Check `visited: HashSet<(String, String)>` for cycle
  - Parse each `include` entry: `@branch` → `(branch, "default")`, `@branch:profile` → `(branch, profile)`
  - DFS recurse for each include, accumulate paths (included paths prepended)
  - Append current profile's own artifact paths
  - Return deduplicated list (first occurrence wins)
- [ ] Cycle detection emits a clear error message showing the cycle
- [ ] Unit tests: DFS ordering correct across two-level include chain
- [ ] Unit tests: deduplication — first occurrence wins
- [ ] Unit tests: cycle `@A → @B → @A` detected and rejected
- [ ] Unit tests: `@A:brief → @B → @A:default` is NOT a cycle (different profiles)
- [ ] Integration test: multi-branch include chain renders in correct DFS order

## Phase 7 — `mem context render` — Diff Integration

- [ ] Run `git diff <diff-args>` using existing `git::run_git` helper
- [ ] Apply `diff_exclude_paths` from `Config` (filter matching paths from diff output)
- [ ] Wrap output in `<diff args="...">...</diff>`
- [ ] Omit `<diff>` block entirely when `diff` field absent from profile
- [ ] On `git diff` failure: warn to stderr, skip `<diff>` block, continue
- [ ] Unit tests: `diff_exclude_paths` glob filtering
- [ ] Integration test: `"diff": "HEAD"` produces `<diff>` block
- [ ] Integration test: absent `diff` produces no `<diff>` block

## Phase 8 — `mem context render` — Output Assembly

- [ ] Assemble final output:
  1. Included artifacts (DFS result from Phase 6)
  2. Current profile's own artifacts
  3. `<diff>` block if present (Phase 7)
- [ ] Wrap each artifact: `<artifact path="<rel-to-root>">\n..content..\n</artifact>`
- [ ] Path in `path=` attribute: relative to git root (consistent with `mem list` output)
- [ ] Missing file: skip, emit warning to stderr, continue — stdout unaffected
- [ ] Write assembled output to stdout
- [ ] Integration test: full render — includes, cross-branch refs, diff, correct order
- [ ] Integration test: missing artifact skipped, warning on stderr, rest renders
- [ ] Integration test: `-p brief` selects correct profile
- [ ] Integration test: `-p nonexistent` produces clear error

## Phase 9 — `mem.json` Config Additions

- [ ] Add `diff_exclude_paths: Vec<String>` to `Config` struct (default: `[]`)
- [ ] Add `base_branch_cmd: Option<String>` to `Config` struct (default: `None`)
- [ ] Update `Config::default()` and serialisation
- [ ] Confirm existing tests pass unchanged (no breaking change to defaults)

## Phase 10 — Polish & Sign-off

- [ ] Audit all error messages for consistency with existing `mem` style (`anyhow::bail!`)
- [ ] Confirm stdout is pipe-clean throughout (no debug output from render)
- [ ] Confirm all stderr warnings have consistent prefix style
- [ ] Run full test suite: `cargo test`
- [ ] Log progress: `mem log add`
