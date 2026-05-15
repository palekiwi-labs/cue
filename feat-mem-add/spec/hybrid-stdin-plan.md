We decided on a hybrid approach for `mem add` to handle content input safely for both humans and AI agents.

## Findings & Context

1. **Test Environment Issues:** Using `std::io::IsTerminal` to auto-detect stdin caused failures in test environments (and CI) because stdin is rarely a terminal in those contexts.
2. **AI Agent Ergonomics:** AI agents struggle with shell escaping when passing multi-line strings as positional arguments. They prefer using their native JSON `Write` tools to create temporary files, avoiding the shell entirely.
3. **The Solution:** A hybrid approach where we support inline content (human), explicit stdin via `"-"` (human/Unix pipelines), and a `--file` flag (AI agents).

## Implementation Plan

1. **CLI Parser (src/cli.rs):**
   - Keep positional `content: Option<String>` but note that `"-"` reads from stdin.
   - Add `file: Option<String>` flag (with `--file` and `-f` aliases).
   - Use clap's `conflicts_with` to ensure `content` and `--file` are mutually exclusive.

2. **Resolution Logic (src/main.rs):**
   - Resolve the content to `Vec<u8>` *before* passing it to the handler.
   - If `--file` is present, read the file.
   - If `content` is `"-"`, read from `stdin`.
   - If `content` is anything else, convert it to bytes.
   - If nothing is provided, return an empty byte vector.

3. **Handler (src/commands/add.rs):**
   - Change signature from `content: Option<String>` to `content: Vec<u8>`.

4. **Testing (tests/add.rs):**
   - Replace old `is_terminal` reliant tests with new matrix:
     - `test_add_inline_content`
     - `test_add_empty`
     - `test_add_from_file`
     - `test_add_from_stdin`
     - `test_add_conflict_file_and_inline`