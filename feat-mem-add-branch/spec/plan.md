## Implementation Plan

The goal is to support an optional `--branch` flag for the `mem add` command. This flag will allow users to specify which branch's memory directory the artifact should be saved to.

### 1. Update CLI Definition
In `src/cli.rs`, we need to add a new `branch` field to the `Add` variant of the `Commands` enum. We will also add the `-b` short flag for `branch` and ensure `clipboard` has `-c`. (Wait, checking `src/cli.rs`, `clipboard` already has `-c` at line 36).

```rust
Add {
    // ... existing fields
    /// Save artifact to a specific branch instead of current
    #[arg(short = 'b', long)]
    branch: Option<String>,
}
```

### 2. Update Main Entry Point
In `src/main.rs`, we need to extract this `branch` argument and pass it to the `commands::add::handle` function.

### 3. Update Add Command Logic
In `src/commands/add.rs`, we need to:
- Update the `handle` function signature to accept `branch_name: Option<String>`.
- Use this `branch_name` to determine the directory.
- If `branch_name` is `None`, use `git::get_current_branch` as before.
- Apply the same sanitization (replacing slashes with hyphens) to the provided branch name.

### 4. Metadata handling
For `Trace` and `Tmp` types, we will continue to use the current `HEAD`'s timestamp and hash for the directory name, even if a different branch is specified for storage. This reflects that the artifact was generated from the current state of the working directory.

### 5. Verification
Add a test case in `tests/add.rs` that:
1. Initializes a `mem` environment.
2. Runs `mem add --branch alternative-branch test.txt "content"`.
3. Verifies that the file is created at `.mem/alternative-branch/spec/test.txt`.
