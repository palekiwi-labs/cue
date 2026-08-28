# Project Log

## [a919c82] Implement --frontmatter flag for mem list

Added YAML frontmatter parsing to mem list. The feature is opt-in via --frontmatter, which implies --json. Implementation uses BufReader with a 64-line budget for bounded reads and early-abort when no frontmatter fence is present.

- **Found:** serde_yaml integrates cleanly with serde_json::Value for output
- **Found:** BufReader provides single-syscall efficiency; most files fully buffered on first line read
- **Found:** Tests with multiline '---' content must write files directly via fs::write to avoid clap flag parsing conflicts
- **Decided:** Use serde_yaml 0.9 as the YAML parser
- **Decided:** 64-line budget for frontmatter block
- **Decided:** enrich_frontmatter as a separate pure function, not embedded in to_mem_file
- **Decided:** --frontmatter implies --json (soft implication)
- **Decided:** skip_serializing_if = Option::is_none for zero schema breakage

## [7c11a47] Fix test suite environment pollution

Two pre-existing test failures were caused by MEM_ARTIFACT_TYPES and MEM_IGNORED_TYPES from the host environment leaking into tests via figment's Env::prefixed merge. Fixed by isolating unit tests with temp_env and integration tests via a new helpers::mem_cmd() constructor.

- **Found:** RUSTC_WRAPPER env var was causing cargo to fail with ctrlc handler panic — unrelated to code, clears with RUSTC_WRAPPER=""
- **Found:** MEM_ARTIFACT_TYPES and MEM_IGNORED_TYPES from host shell were overriding JSON config in tests
- **Found:** The same pattern affected add.rs, config_show.rs, init.rs, log.rs in addition to list.rs
- **Decided:** Use temp_env::with_var_unset for in-process unit tests
- **Decided:** Use helpers::mem_cmd() with env_remove for subprocess integration tests
- **Decided:** Single canonical constructor for all mem test commands to prevent future recurrence

## [6e53b11-dirty] Implement --filter flag for mem list

Added frontmatter-scoped filtering to mem list via a repeatable --filter flag. Filters are ANDed, operate on parsed YAML frontmatter, and work in both text and JSON output modes.

- **Found:** Filter struct with FromStr enables clap to validate expressions at arg-parse time, not mid-loop
- **Found:** Grouping handle() args into ListOptions struct was required to satisfy clippy::too_many_arguments (limit is 7)
- **Found:** Integration tests required --test-threads=1 due to EAGAIN process limit exhaustion in the sandbox environment
- **Decided:** --filter is frontmatter-scoped only (not full JSON field path)
- **Decided:** --filter implies frontmatter parsing but does NOT add frontmatter to output (--frontmatter still needed for that)
- **Decided:** Operators: = (eq), != (neq), ~= (substring) — numeric comparisons deferred
- **Decided:** Missing key: = evaluates false, != evaluates true
- **Decided:** Multiple filters are ANDed via .all()
- **Decided:** RHS is type-coerced via serde_json::from_str so numbers and booleans match correctly
- **Decided:** Dot notation for nested keys via get_nested()

## [e6715b6] Implement -f/--frontmatter flag for mem add

Added YAML frontmatter embedding at artifact creation time. Users can now pass repeatable -f KEY=VALUE pairs to mem add; the YAML block is prepended to the file content before writing.

- **Found:** serde_yaml::from_str coerces bool/int values for free at write time, matching the existing filter parser behaviour
- **Found:** AddOptions struct was required to satisfy clippy::too_many_arguments (limit is 7) — mirrors the ListOptions pattern already established
- **Found:** Insertion order in serde_yaml::Mapping is preserved, so fields appear in the YAML block in the order the user specified them
- **Decided:** -f reclaimed from --file (--file is now long-only, no breaking change concern per user instruction)
- **Decided:** parse_frontmatter_field as a clap value_parser rejects invalid input at arg-parse time, before any I/O
- **Decided:** build_frontmatter_bytes is a private pure function in add.rs — no need to move to a shared util yet
- **Decided:** No file-type restriction: users can add frontmatter to any file extension

## [a7e8d5b] Fix --filter help text in mem list

- **Found:** clap 4 collapses consecutive /// lines within the same paragraph into a single line in help output
- **Found:** A blank /// line between paragraphs splits short help (first paragraph) from long help (subsequent paragraphs)
- **Found:** verbatim_doc_comment preserves explicit line breaks within a paragraph in --help output
- **Decided:** Use blank /// + verbatim_doc_comment together: blank line gives -h a clean one-liner, verbatim_doc_comment preserves the examples block in --help

## [a7e8d5b] Research: Review findings verified against actual code

- **Found:** Issues A/B/C (High bugs) are NOT real bugs — the Contains/non-string behaviour is intentional and tested in contains_non_string_value_is_false unit test
- **Found:** Issue D (empty key) is REAL: parse_frontmatter_field in cli.rs does not validate empty key from -f =value
- **Found:** Issue E (silent YAML errors) is REAL design gap
- **Found:** Issue F (-f inconsistency): mem add uses -f for --frontmatter, mem log add uses -f for --file — real inconsistency
- **Found:** Issue G (double parse) is REAL: matches_filters parses frontmatter for all files, then enrich_frontmatter re-parses for files that pass — two separate File::open + BufReader + serde_yaml::from_str calls per matched file
- **Decided:** High severity issues from reviewer are not actual bugs — they are intentional, tested design choices
- **Decided:** Focus fixes on: D (empty key validation), E (silent error warning), F (-f long-flag standardization for log add), G (eliminate double parse)

## [2a49ad5] Fix review findings: empty key, double-parse, -f flag consistency

- **Found:** parse_frontmatter_field did not reject empty key from -f =value — now fixed with early return
- **Found:** matches_filters discarded parsed frontmatter then enrich_frontmatter re-read the same file — eliminated by threading cached value forward via Vec<(PathBuf, Option<Value>)>
- **Found:** log add had short = 'f' on --file conflicting with add's -f = --frontmatter
- **Decided:** Replaced matches_filters + enrich_frontmatter two-step with a single filter_map that calls parse_frontmatter once and carries the result
- **Decided:** Removed enrich_frontmatter entirely (dead code after refactor)
- **Decided:** log add --file is now long-only; -f is exclusively mem add --frontmatter across the CLI

