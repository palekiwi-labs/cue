# Gemini Review: mem context Feature Design

Consulted Google Gemini for critical analysis of the proposed `mem context` design.
Key findings below, grouped by severity.

## Critical Issues

### 1. JSON hostile for manual editing

Since CLI add/remove helpers are deferred to v2, users will hand-edit `context.json`.
JSON has no comments, no trailing commas. Comments are crucial for memory
(e.g. `// omitting trace-report.md — too noisy for this session`).
**Suggestion:** JSONC, TOML, or YAML. TOML is idiomatic in Rust.

### 2. XML escaping trap

Artifacts wrapped in `<artifact>` tags could themselves contain `</artifact>`,
especially since this project's docs describe `mem`. Options:

- Use CDATA sections: `<![CDATA[...]]>`
- Switch delimiter format to markdown fenced blocks with attributes:
  ````
  ```markdown {path=".mem/main/spec/index.md"}
  content
  ```
  ````

### 3. `diff: "branch"` is underspecified

How does `mem` know what the base branch is? `git merge-base main HEAD` breaks
for stacked PRs. Needs either:

- Explicit base in config: `"diff": "branch:main"`
- Or reading upstream tracking ref

### 4. LLM recency bias — output ordering matters

LLMs weight the end of context windows more heavily (U-shaped attention curve).
Output order must be an explicit guarantee:
`[Included/base artifacts] → [Current branch artifacts] → [Diff]`
Most actionable content must be at the bottom.

## Important Gaps

### 5. Missing artifact behaviour

If a referenced branch is deleted (post-merge PR) or a file is renamed,
behaviour is undefined. Should: warn to stderr and continue (not hard-fail),
optionally injecting `<warning>` tag in output so agent knows what's missing.

### 6. Diff explosion risk

`"branch"` diffs on large refactors or `Cargo.lock` changes can blow out
context windows. Consider path filtering: `"diff": { "mode": "branch", "paths": ["src/", "!Cargo.lock"] }`.

### 7. Cycle detection correctness

Must track `(branch, profile)` pairs, not just branch name.
`@A:brief → @B:default → @A:default` is NOT a cycle but a naive branch-only
tracker would falsely flag it.

## Naming Suggestions

### 8. `cat` → `render`

`cat` implies reading a file from disk. We're resolving a dependency graph,
fetching diffs, and transforming output. `render` communicates the transformation.

### 9. `show` is ambiguous

Does it show raw config or the rendered output? Consider:

- `mem context show` / `mem context view` — print raw JSON config
- Or open in `$EDITOR` (like `git config --edit`)

### 10. `mem context list-profiles` (or `profiles`)

An AI agent booting up has no way to know profile names like `"brief"` exist.
A discovery subcommand would let the agent introspect available contexts.

## v2+ Ideas

- **`exclude` array** — drop specific artifacts from an included context:
  `"exclude": ["@main:spec/legacy.md"]`
- **Token counting** — `mem context render --dry-run --tokens` to estimate
  context window cost without printing output
- **Line ranges** — `@branch:doc/huge.md#L100-200` to cherry-pick a chunk
- **Markdown transclusion alternative** — `context.md` instead of `context.json`,
  using `![[./spec/plan.md]]` Obsidian-style syntax with interleaved prose
  instructions to the agent
- **Diff path filtering** — `"diff": { "mode": "branch", "paths": ["src/"] }`

## Summary Verdict

| Issue                            | Recommendation                                         |
| -------------------------------- | ------------------------------------------------------ |
| JSON → JSONC/TOML                | Worth adopting in v1 (no CLI helpers = manual editing) |
| XML escaping                     | Must fix before shipping                               |
| `diff: "branch"` base resolution | Must specify                                           |
| Missing artifact → warn not fail | Must define                                            |
| `cat` → `render`                 | Worth adopting                                         |
| `list-profiles` subcommand       | Cheap, useful, add to v1                               |
| Diff explosion / path filtering  | Defer to v2                                            |
| `exclude` array                  | Defer to v2                                            |
| Token counting                   | Defer to v2                                            |
| Markdown transclusion            | Interesting, defer                                     |
