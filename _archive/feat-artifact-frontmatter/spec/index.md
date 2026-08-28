# Artifact Frontmatter Parsing

Enable `mem list` to parse and include YAML frontmatter from markdown artifacts when outputting JSON.

## Goals
- Add YAML frontmatter parsing to the `mem list` command.
- Ensure the feature is opt-in and does not degrade performance for standard listings.
- Maintain backward compatibility for existing JSON consumers.

## Requirements
- Support an optional `--frontmatter` flag for the `mem list` command.
- The `--frontmatter` flag should imply `--json`.
- Frontmatter should be parsed from the top of markdown files (between `---` delimiters).
- Parsing must be efficient (bounded reads, early-abort if no fence is found).
- Resulting metadata should be included in a `frontmatter` field in the JSON output.
- If frontmatter is missing or malformed, the field should be omitted or null (following existing patterns).

## Non-Goals
- Parsing the entire body of markdown files.
- Supporting non-YAML frontmatter formats (e.g., TOML) in this initial phase.
- Automatic extraction of specific fields into top-level JSON keys (keep it nested under `frontmatter`).

## Implementation Constraints
- Use `serde_yaml` for parsing.
- Implement early-abort logic to avoid unnecessary I/O.
- No breaking changes to existing JSON schema (use `skip_serializing_if`).
