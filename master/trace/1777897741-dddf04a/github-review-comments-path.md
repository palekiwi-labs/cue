# Analysis: Adding GitHub Review Comments to `mem add`

This document outlines the proposed path for implementing `--github review-comments` in `mem add`.

## 1. CLI Changes (`src/cli.rs`)

To add the `--github` flag, we should update the `Add` command in `src/cli.rs`. Using an enum for the argument is recommended to allow for future expansion (e.g., `--github issues`, `--github pr-diff`).

**Proposed Changes:**
```rust
#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum GithubSource {
    ReviewComments,
}

// In Commands::Add
#[arg(long, value_enum)]
pub github: Option<GithubSource>,
```

**Reasoning:**
- `ValueEnum` integrates well with `clap` and provides built-in validation and help messages.
- Placing it in `Commands::Add` keeps it scoped to the artifact creation workflow.

## 2. Workflow Injection (`src/main.rs`)

The content fetching logic should be injected into the `match cli.command` block in `src/main.rs`. Currently, it handles clipboard, file, and stdin. `github` would be a fourth source of content.

**Current logic (src/main.rs:28):**
```rust
let resolved_content: Vec<u8> = if clipboard {
    resolve_clipboard(&filename)?
} else if let Some(path) = file {
    std::fs::read(&path)...
} else if let Some(gh_source) = github {
    resolve_github(gh_source)?
} else {
    // Stdin or direct content
}
```

**Proposed `resolve_github` function:**
This function would handle shelling out to `gh` and potentially formatting the JSON.

## 3. External Tool Handling (`src/git.rs` & `src/external.rs`)

While `src/git.rs` handles `git` commands, it is cleaner to create a new `src/github.rs` or `src/external.rs` for `gh` utility interactions. This avoids cluttering the git-specific module with GitHub-specific logic (like PR detection and JSON parsing).

**Proposed `src/github.rs`:**
```rust
pub fn fetch_review_comments(cwd: &Path) -> anyhow::Result<Vec<u8>> {
    // 1. Detect current PR for the branch
    // 2. Run `gh pr view --json reviews,comments` or similar
    // 3. Return as bytes
}
```

## Sourced Findings

### `src/cli.rs`
The `Add` command structure:
```rust
25:     Add {
26:         /// Name of the artifact file
27:         filename: String,
...
39:         mem_type: MemType,
40:         /// Overwrite existing file
41:         #[arg(long)]
42:         force: bool,
43:     },
```

### `src/main.rs`
The content resolution flow:
```rust
28:             let resolved_content: Vec<u8> = if clipboard {
29:                 resolve_clipboard(&filename)?
30:             } else if let Some(path) = file {
31:                 std::fs::read(&path).with_context(|| format!("Failed to read file {}", path))?
32:             } else {
...
```

### `src/git.rs`
Existing command execution pattern:
```rust
12:     let output = Command::new("git")
13:         .args(&args_vec)
14:         .current_dir(cwd)
15:         .output()
```

## Conclusion
The implementation should:
1. Add `GithubSource` enum to `src/cli.rs`.
2. Implement `resolve_github` in `src/main.rs`.
3. Create `src/github.rs` to encapsulate `gh` commands and JSON fetching.
