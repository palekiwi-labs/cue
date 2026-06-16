# Code Review: mem context feature

## Overview
Review performed by @consultant-gemini on the full `mem context` implementation (scaffolding through render engine).

## Findings

### 1. Critical Security: Absolute Path Override Bypass
**Issue:** `PathBuf::join()` replaces the entire path if the second argument is absolute. The current check `Path::new(raw).is_absolute()` can be bypassed by using a relative prefix (e.g., `.//etc/passwd`) which evaluates to false but then yields an absolute path after `strip_prefix`.
**Recommendation:** Move absolute path validation to after the prefix is stripped.

### 2. Logic Bug: Cycle Detection (Diamond Dependencies)
**Issue:** `visited` set is passed mutably and never pruned. This flags valid Directed Acyclic Graphs (DAGs) as cycles if a branch is reachable via multiple paths.
**Recommendation:** Implement backtracking by removing the key from `visited` after the recursive call.

### 3. Edge Case: Directory Paths in Render
**Issue:** `path.exists()` returns true for directories, but `std::fs::read_to_string` fails.
**Recommendation:** Use `path.is_file()` to skip directories.

### 4. Git Diff Argument Conflict
**Issue:** Unconditionally pushing `--` to `git diff` args for exclusions. If `diff_args` already contains `--`, the command will fail.
**Recommendation:** Check for existing `--` before appending.

### 5. Idiomatic Rust Improvements
- Redundant JSON parsing in `render.rs`.
- Use `branch.replace(['/', '\\'], "-")` for more efficient sanitization.
- Normalize backslashes in XML output path attributes for cross-platform consistency.
- Use `Path::components()` to check for `ParentDir` instead of string `contains("..")`.