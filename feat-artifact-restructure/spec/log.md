# Project Log

## [221149f] Research complete: Artifact Restructure Context

- **Found:** `src/commands/add.rs` hardcodes nesting for `trace` and `tmp` types.
- **Found:** `src/commands/list.rs` only attempts to parse metadata if the category is `trace` or `tmp`.
- **Found:** `src/context/mod.rs` assumes the `spec/` directory is flat, which will break if specs are traced.
- **Decided:** Unify artifact storage paths to `.mem/<branch>/<type>/<filename>` by default.
- **Decided:** Introduce a flag (e.g., `--traced`) to enable nesting under `<timestamp>-<hash>` for any artifact type.
- **Decided:** Refactor `list` command to parse metadata based on directory structure rather than artifact category.

## [b86d141] Artifact restructure implementation complete

All implementation steps from the plan are done and committed in two atomic commits on feat/artifact-restructure.

- **Found:** collect_spec_files was added unnecessarily; the auto-discovery logic it supported was removed as part of the simplification
- **Found:** ref and bin are no longer default artifact types; tests that use them now register them via mem.json
- **Decided:** Remove MemType enum entirely; artifact types are plain strings validated against config.artifact_types
- **Decided:** Introduce --pin flag as the only way to get ts-hash nesting; no type has implicit auto-pin behavior
- **Decided:** Default artifact_types: [spec, trace, tmp]; default ignored_types: [tmp]
- **Decided:** Pinned artifact detection in list is structural (depth >= 4, parseable ts-hash dir) not type-based
- **Decided:** context init no longer auto-discovers spec files; initializes empty default profile unless config.context is defined

## [584f0bd] Replaced --pin with --root, inverted default storage behavior

- **Decided:** Default storage is now ts-hash nested — all artifacts are point-in-time by default
- **Decided:** --root flag saves flat at the root of the type directory, intended for stable anchor documents
- **Decided:** Renamed from --pin because 'root' conveys the structural meaning more clearly than 'pinned'
- **Decided:** Updated mem skill to document root vs point-in-time distinction and update all mem-add examples

