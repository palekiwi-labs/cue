---
title: 
  fix: quote title values containing colons in task frontmatter
status: complete
priority: high
branch: fix/title-yaml-quoting
---
# fix: quote title values containing colons in task frontmatter

When `cue task` (or the `cue-task` tool) writes YAML frontmatter and the
title string contains `: ` (colon-space), the value is written unquoted:

```yaml
title: curator: live acuity integration
```

YAML parsers interpret `colon-space` as a key-value separator, so this is
parsed as a mapping instead of a string:

```json
"title": { "curator": "live acuity integration" }
```

The correct output is a quoted string:

```yaml
title: "curator: live acuity integration"
```

Any consumer that reads `title` from frontmatter and expects a string will
silently receive an object (or fail to deserialise) when the title contains
a colon. The bug is silent — no write-time error is produced.

## Affected surface

Wherever `cue` writes the `title` frontmatter field — at minimum the
`cue task` subcommand and any internal frontmatter serialisation path.
Other string fields that accept free-form user input (e.g. `branch`) should
be audited for the same issue.

## Reproduction

```bash
cue task add --title "foo: bar" my-task.md
# inspect frontmatter: title field will be a YAML mapping, not a string
```

## Expected behaviour

Any string value written to a YAML frontmatter field must be quoted if it
contains characters that are significant in YAML (`: `, `#`, `{`, `[`, etc.).
At minimum, values containing `: ` must be emitted as quoted strings.

## Source

- Discovered during: agent session creating roadmap tasks for the cue ecosystem
- Example broken file (now manually fixed):
  `.cue/master/task/1781965432-d2f3251/curator-live-half.md`

## Acceptance Criteria

| #  | Criterion                                                                         | Verify by                                                      | Evidence |
| -- | --------------------------------------------------------------------------------- | -------------------------------------------------------------- | -------- |
| 1  | `cue task add --title "foo: bar baz" t.md` writes `title: "foo: bar baz"`        | inspect generated frontmatter                                  | `test_add_frontmatter_colon_in_string_value` asserts raw file does not contain unquoted mapping; `cargo test` passes (commit 27cecdc) |
| 2  | The written file round-trips: YAML parse of `title` yields a string, not a map   | parse frontmatter programmatically, assert `typeof title == string` | Same test parses frontmatter via `serde_yaml` and asserts `parsed["title"].as_str() == Some("foo: bar baz")`; passes (commit 542ffbb) |
| 3  | Other free-form string frontmatter fields are audited and fixed if affected       | code review                                                    | All free-form fields route through `build_frontmatter_bytes`; single fix covers full surface. `branch` field also tested in the new test. `Tagged` variant hardened in commit 542ffbb after code review. |
| 4  | Existing tests cover frontmatter serialisation with colon-containing strings      | `cargo test`                                                   | 27 tests in `add.rs` pass including `test_add_frontmatter_type_coercion` (scalar coercion preserved); full suite: 0 failures (commit 542ffbb) |
