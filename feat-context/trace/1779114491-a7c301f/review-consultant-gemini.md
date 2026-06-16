# Code Review: `mem context` Feature Implementation

Overall, this is a highly robust and well-architected PR. The core logic handles a surprising amount of complexity (DAG-based profile resolution, cross-branch sigils, path canonicalization) cleanly and idiomatically. Test coverage is thorough and effectively isolates the environment using `MEM_CONFIG_DIR`.

Below are the detailed findings, categorized into **Security & Edge Cases**, **Code Quality / Rust Idioms**, and **Design Observations**.

## 1. Code Quality & Rust Idioms

### Redundant `.unwrap()` in `strip_prefix`
In `src/context/mod.rs` inside `resolve_profile`, when handling the `None` case of the `split_once(':')`, there is a redundant `unwrap()`:

```rust
let (inc_branch, inc_profile) = if let Some(rest) = inc.strip_prefix('@') {
    match rest.split_once(':') {
        Some((b, p)) => { ... }
        None => {
            // ⚠️ `inc.strip_prefix('@')` was already matched into `rest`
            let branch = inc.strip_prefix('@').unwrap(); 
            // Better: `let branch = rest;`
```
**Feedback:** You can safely replace `inc.strip_prefix('@').unwrap()` with `rest` to avoid the panic branch and make the code slightly cleaner.

## 2. Security & Edge Cases

### Excellent Path Traversal Mitigation
Using `path.canonicalize()` followed by `canonical_path.starts_with(&canonical_git_root)` is the exact correct approach to preventing directory traversal attacks (e.g. `../../../etc/passwd`) while safely allowing intra-repo parent traversal (`../master/spec/index.md`). Great work here.

### Edge Case: Absolute Paths + Glob Crate
In `resolve_profile`:
```rust
let pattern = path.to_string_lossy();
match glob(&pattern) { ... }
```
Because `path` is an absolute path (`git_root.join(...)`), if the user's repository happens to be cloned into a directory containing glob characters (e.g., `/home/user/my[repo]/project`), `glob()` will treat `[repo]` as a character class and fail to match. 
**Feedback:** This is a known, tricky limitation of the `glob` crate. For a CLI tool, this might be an acceptable edge case, but it's worth noting. A future improvement could be `globset` + `walkdir`, or changing the current directory before globbing.

### Potential Command Injection via Git Diff (Low Risk)
In `gather_context`:
```rust
let mut args = vec!["diff"];
let split_args: Vec<&str> = diff_string.split_whitespace().collect();
args.extend(split_args.iter().cloned());
```
If a user defines `diff: "HEAD --output=/tmp/overwrite.txt"`, Git will honor the output flag. Furthermore, the `--` separator is only appended if `config.diff_exclude_paths` is not empty. 
**Feedback:** Since this configuration is controlled by the user/repo, the risk is minimal. However, forcefully appending `--` after resolving git flags (if no custom excludes are defined) could prevent unintended parsing of branch names as flags.

## 3. Design Observations

### Profile Inheritance (Diffs and Instructions)
When `Profile A` includes `Profile B`, `resolve_profile` correctly merges and deduplicates `artifacts` using a DFS traversal. However, in `gather_context`, the `diff` and `instructions` fields are strictly loaded from the **root profile** (`Profile A`). Any diff or instruction blocks defined in `Profile B` are silently ignored.
**Feedback:** This is almost certainly the intended behavior—composing git diff args or overriding system prompts recursively is messy and error-prone. However, it would be beneficial to document this behavior (e.g., in a docstring or CLI help text) so users don't expect `instructions` to inherit from included profiles.

### Minor Warning Noise
If a user runs `mem context render` on a branch that doesn't have a `context.json`:
1. `resolve_profile` will emit: `Warning: Could not load context for branch <branch>, skipping`
2. `gather_context` will subsequently fail with a hard error on `load_context_config(&context_path)?`.
**Feedback:** The double-reporting is a bit noisy but harmless. Checking for the root file's existence earlier in `gather_context` could smooth out the UX.

## Conclusion
The implementation is solid, the logic is easy to follow, and the tests validate the critical paths effectively. Resolving the minor `unwrap()` redundancy is the only strict code change I would recommend before merging; the rest are considerations for future polish. Great PR!
