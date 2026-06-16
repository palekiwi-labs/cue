# Code Review: `mem context` Implementation

**Reviewer:** `consultant-gemini` (via `Task` tool)
**Date:** 2026-05-18

## Summary
Overall, the design elegantly solves context assembly using profiles and recursive includes. However, there are two **critical security vulnerabilities** and a **performance bug** in the graph resolution that need to be addressed before merging.

### 1. Security of Path Resolution in `src/context/mod.rs`
**🟢 Praise:** The mechanism preventing path traversal is generally very strong. You correctly prevent absolute/root paths during initial parsing, and rely on `canonicalize()` to follow symlinks and collapse `../`. The final `canonical_path.starts_with(&canonical_git_root)` check effectively boxes resolutions into the repository.

**🔴 Vulnerability 1: Exposure of the `.git` directory**
Because `.git` lives inside `git_root`, paths like `"../../.git/config"` will survive the `starts_with(&canonical_git_root)` boundary check. If an attacker commits a `context.json` pointing to `.git/config`, `mem context render` will extract it, potentially leaking access tokens or webhook secrets.
* **Recommendation:** Add an explicit block against the `.git` directory in `gather_context`:
  ```rust
  if canonical_path.starts_with(canonical_git_root.join(".git")) {
      continue;
  }
  ```

**🔴 Vulnerability 2: Git Argument Injection (Arbitrary File Write) in `diff_args`**
While the PR description mentions removing `base_branch_cmd` to fix an RCE risk, a severe vulnerability remains in the diff block. By parsing `diff_args.split_whitespace()` and passing it directly to `git diff`, you allow arbitrary flag injection. An attacker can set `"diff": "--output=/home/user/.ssh/authorized_keys main...HEAD"` in `context.json`. When `mem context render` runs, Git will overwrite the victim's SSH keys with the diff output.
* **Recommendation:** Disallow user-provided flags. You can do this by rejecting any split argument that starts with `-` (unless it is exactly `--`), or by replacing the freeform `diff` string with structured data (e.g., `{"diff": {"base": "@base", "head": "HEAD"}}`).

### 2. Correctness of DFS Resolution and Deduplication
**🟢 Praise:** The deduplication logic iterating over `accumulator` with a `HashSet` properly retains the *first* occurrence of a file. Because included profiles are expanded *before* local artifacts, deeper upstream dependencies yield first.

**🟡 Finding: Exponential Time Complexity on Diamond Dependencies**
In `resolve_profile`, cycle detection is implemented by adding the current profile to a `visited` set and then removing it at the end of the function (`visited.remove(&key);`). This fails to memoize the resolution of valid DAGs. Profile D will be fully resolved and disk globs will be expanded twice in a diamond dependency.
* **Recommendation:** Instead of a `visited` stack, use a `HashMap<(String, String), Vec<PathBuf>>` to cache successfully resolved profiles. This fixes both cycles and repeated resolutions.

### 3. Usability of the CLI Interface
**🟢 Praise:** The `@base` sigil paired with the explicit `--base` CLI flag is an excellent, safe UX design for evaluating diffs.
**🟢 Praise:** `init --force` falling back gracefully to the legacy `spec/` auto-discovery provides an effortless migration path.

**🟡 Finding: XML Injection Risk**
The `render` command dumps raw artifact content and git diffs directly into `<artifact>` and `<diff>` tags. If a source file contains `</artifact>`, it will prematurely close the tag.
* **Recommendation:** Consider wrapping injected file contents in `<![CDATA[ ... ]]>` blocks or replacing the XML tags with markdown code blocks.

### 4. Robustness of the Integration Tests
**🟢 Praise:** High-quality coverage in `context_render.rs` and `context_init.rs`. The introduction of `MEM_CONFIG_DIR` correctly isolates test states.

**🟡 Finding: Missing Coverage**
1. **Cross-Branch Resolution:** No integration test validating stitching across two different branches.
2. **Security Regressions:** Add integration tests asserting failure for absolute paths, traversal, or unsafe git flags.
