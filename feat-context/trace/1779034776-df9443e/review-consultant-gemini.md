# Code Review Report: feat/context

**Reviewer:** @consultant-gemini
**Date:** Sun May 17 2026

## 🌟 Strengths

- **AI-Friendly Output:** The rendering logic produces excellent XML-like blocks (`<artifact>`, `<diff>`) perfectly suited for LLM context injection.
- **Robust Testing:** Comprehensive integration tests covering normal operation, cross-branch interactions, cycle detection, and diamond dependencies.
- **Clean Abstractions:** Modular architecture with clear separation between CLI routing, CLI handling, and core logic.

## 🚨 Critical Vulnerabilities & Bugs

- **Path Traversal Vulnerability:** `parse_artifact_path` allows `..` without boundary checks, potentially allowing arbitrary file reads outside the Git repository.
- **Directory Context Inconsistency:** Some handlers use `cwd` to resolve `.mem/...` paths, which fails when run from subdirectories. Should consistently use `git_root`.
- **Cross-Branch Parsing Inconsistency:** `parse_artifact_path` uses `rsplit_once(':')` which can incorrectly split valid file paths containing colons. Should standardize on `split_once(':')`.
- **Git Flag Injection:** `diff` arguments from `context.json` are passed directly to `git diff`, creating a risk of malicious flag injection (e.g., `--output`).

## 🏗️ Architectural Issues

- **Diamond Dependency Performance:** `resolve_profile` re-parses diamond dependencies because it doesn't memoize resolved profiles.
- **Inconsistent Error Handling:** `gather_context` crashes if `context.json` is missing, while `resolve_profile` handles it gracefully with a warning.
- **Ignored 'diff' Instructions:** Only the root profile's `diff` is honored; included profiles' `diff` values are ignored.

## 🎨 Idiomatic Rust & Ergonomics

- **Unnecessary Option:** `profile` argument in `cli.rs` has a default value, so it can be a `String` instead of `Option<String>`.
- **Unused Config Field:** `base_branch_cmd` was added but not used.

## 🛠️ Specific Suggestions

- Implement canonicalization and boundary checks for artifact paths.
- Standardize on `git_root` for path resolution in CLI handlers.
- Use `split_once(':')` for cross-branch references.
- Add validation for `diff` flags.
- Memoize profile resolution results.
- Simplify `Option<String>` to `String` where defaults are present.
