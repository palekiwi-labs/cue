# Plan: Native GitHub Integration in `mem`

## Overview
This plan outlines the implementation for adding native, optional features to `mem`, specifically targeting a `mem add --github review-comments` command. This approach embeds domain-specific knowledge (like shelling out to `gh`) directly into the application, rather than relying on generic shell aliases.

## 1. CLI Updates (`src/cli.rs`)
Add a new flag to the `Add` command to support GitHub integrations.

- Introduce a new `ValueEnum` to specify the type of GitHub data requested.
```rust
#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum GithubSource {
    ReviewComments,
}
```
- Add the argument to `Commands::Add`:
```rust
/// Fetch data from GitHub for the current PR
#[arg(long, value_enum, conflicts_with_all = &["content", "file", "clipboard"])]
github: Option<GithubSource>,
```

## 2. GitHub Service Module (`src/github.rs`)
Create a dedicated module to encapsulate interactions with the `gh` CLI. This mirrors the pattern already established in `src/git.rs`.

- Create `src/github.rs`.
- Implement a helper function to run `gh` commands, handling missing binary errors gracefully.
- Implement a specific function to fetch PR review comments:
  - Use `gh pr view --json comments,reviews` or the `gh api` endpoint to fetch the relevant JSON.
  - Optionally determine the current PR using the branch name (managed by `git.rs`).
  - Return the raw JSON bytes.

## 3. Workflow Integration (`src/main.rs`)
Integrate the new GitHub resolver into the `mem add` workflow.

- Update the `resolved_content` assignment block in `Commands::Add` to handle the new `github` argument.

```rust
let resolved_content: Vec<u8> = if clipboard {
    resolve_clipboard(&filename)?
} else if let Some(path) = file {
    std::fs::read(&path).context(...)
} else if let Some(github_source) = github {
    // Call the new module
    github::resolve_source(github_source, &cwd)?
} else {
    // stdin/raw handling
}
```

## 4. Testing
- Consider creating a mock or using the existing test framework to verify the CLI parsing.
- Note: Testing external dependencies like `gh` in unit tests might be flaky, so focus tests on argument parsing and the dispatch logic, potentially mocking the output of `github::resolve_source`.

## Architectural Notes (SRP vs. Convenience)
While embedding specific domain knowledge (GitHub, RSpec) stretches the strict "Single Responsibility Principle" of a pure storage tool, it heavily aligns with the tool's goal as an "agent context manager." If `mem` exists to curate context, teaching it *how* to gather common context types (like PR reviews) is a pragmatic enhancement. Opting for native implementation provides better error handling and UX than relying on users piping external tools manually.
