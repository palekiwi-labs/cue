# Design Document: Configurable Context Templates & Glob Support

This document outlines the design decisions and implementation strategy for enhancing `mem context` with configurable templates, glob expansion, and dynamic diff resolution.

## 🎯 Objectives

1. Make `mem context init` template-driven via `mem.json`.
2. Support glob patterns in artifact paths, resolved at **render time**.
3. Implement the `@base` sigil for `diff` strings to dynamically resolve the base branch.
4. Hardened security against path traversal and flag injection.

## 🛠️ Implementation Plan

### 1. Configuration (`mem.json`)

- **Context Templates**: Add an optional `context` field to the `Config` struct.
- **Base Branch Command**: Use the existing `base_branch_cmd` field.
- **Security Constraint**: The `base_branch_cmd` **must only** be respected if defined in the **global** config (`~/.config/mem/mem.json`) to prevent RCE from untrusted repositories.

### 2. Context Initialization (`mem context init`)

- If a template is present in `Config`, use it to populate `.mem/<branch>/context.json`.
- Skip auto-discovery of `./spec/` files when a template is used.
- Write literal glob patterns to the JSON file to ensure dynamic resolution at render time.

### 3. Context Rendering (`mem context render`)

- **Glob Expansion**: Use the `ignore` crate (or equivalent) to expand patterns at render time.
- **Filtering**: Expanded patterns must automatically respect `.gitignore` and skip hidden directories.
- **Limits**: Implement a hard limit on the number of expanded files (e.g., 50) and total context size to prevent token explosion.
- **Diff Resolution**:
  - Replace `@base` with the trimmed output of `base_branch_cmd`.
  - Fail immediately if the command is missing or returns an error.

### 4. Security Hardening (from @consultant-gemini)

- **Path Traversal**: All artifact paths (fixed or globbed) must be canonicalized and verified to reside within the Git repository root.
- **Flag Injection**: Tightly sanitize `diff` arguments. Enforce a structure that prevents injection of dangerous flags like `--ext-diff` or `--output`.
- **Parsing**: Standardize on `split_once(':')` for cross-branch references.

## 💬 Consultant Feedback (Gemini Summary)

- **Scaling Risk**: Warned about "context explosion" with globs. Recommended hard limits on file count and payload size.
- **RCE Risk**: Identified critical vulnerability where a local `mem.json` could execute arbitrary code via `base_branch_cmd`. Restricted this field to global config only.
- **Initialization Clarity**: Recommended "Template Wins" approach for `init` to keep the configuration clean and predictable.
- **Git Robustness**: Highlighted the need to `.trim()` command output and sanitize diff arguments against flag injection.

## 📅 Status

This plan is preserved for future implementation. The `feat/context` branch currently remains feature-complete for static paths.
