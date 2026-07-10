# Project Log

## [8b1483c] Slice 0 complete: cue CLI list-valued frontmatter

Implemented the CLI foundation for array-valued frontmatter (commit 8b1483c on feat/frontmatter-array-values). `cue add -f key=a -f key=b` now produces a YAML Sequence instead of overwriting; single occurrence stays scalar. Field-agnostic — the CLI has no knowledge of `refs` specifically.

- **Found:** build_frontmatter_bytes (crates/cue/src/add/mod.rs:105) previously forced any parsed Sequence/Mapping/Tagged back to a String scalar AND silently overwrote duplicate keys (last-wins). Both are now fixed.
- **Found:** serde_yaml::Mapping is IndexMap-backed, preserving insertion order; clap 4.6.1 preserves same-flag occurrence order. Both needed for deterministic key/element ordering.
- **Found:** The change is fully contained in build_frontmatter_bytes. parse_frontmatter_field, the --frontmatter flag, AddOptions, main.rs, and commands/add.rs are all untouched.
- **Decided:** Option A (repeated key -> Sequence) over dedicated --ref flag, flow-style, or JSON value syntax. Confirmed by @consultant-opus: leaves parse_frontmatter_field and the colon-protection guard untouched, no new escaping grammar.
- **Decided:** Dropped empty-array support entirely. Rule: no flags -> no frontmatter value (key absent). The future acumen consumer normalizes scalar -> single-element list.
- **Decided:** Extracted coerce_scalar helper; empty value now yields empty string, not YAML null.
- **Open:** cue-plugins tools need a refs param (default []) + a shared frontmatterFlags helper that expands arrays into repeated -f.
- **Open:** cue skill must document refs as mandatory on all artifacts (note: empty refs -> omit, consumer normalizes).
- **Open:** Pre-existing fmt drift in add/mod.rs:1 import ordering left untouched (unrelated to this change).

## [8b1483c] refs-frontmatter story: all 3 repos committed

All three slices of the refs-frontmatter-array-values story are implemented and committed on the `feat/frontmatter-array-values` branch in each repo:

- cue: 8b1483c — `build_frontmatter_bytes` repeated-key -> YAML Sequence (field-agnostic). 6 tests, cargo test -p cue green.
- cue-plugins: baedf29 — shared `frontmatterFlags` helper; `refs` param on cue-plan/task/todo/note; widened cue-add `frontmatter` to string|string[]; SKILL.md documents refs as mandatory frontmatter discipline. tsc --noEmit green.
- cue.nvim: 8f430b6 — `core.add` array-valued frontmatter handling. luacheck green.

Bookkeeping: executive plan `refs-frontmatter-cli.md` marked complete; task `refs-frontmatter-array-values.md` evidence filled for automated criteria (1-4, 6-luacheck). Criteria 5 (skill review) and 6-functional (nvim QA) await user attestation, so task status left open.

- **Found:** cue-plugins devshell requires `nix develop -c` to get node/tsc/bun; the local node_modules/.bin/tsc needs node on PATH which only the flake devshell provides.
- **Found:** cue.nvim has an untracked .agents/ dir that must be excluded from commits.
- **Decided:** Dropped empty-array support everywhere: no flags -> no frontmatter value. Reversed the earlier 'seed refs: [] for inline editing' for cue.nvim since the CLI can no longer emit an empty list; nvim omits refs when none provided, consumer normalizes.
- **Decided:** Task #6 scope revised from 'seeds refs: []' to 'emits list-valued frontmatter via core.add'.
- **Open:** User QA pending: cue skill review (#5) and functional nvim test (#6). Task cannot flip to complete until those Evidence cells are filled by user attestation.
- **Open:** Pre-existing fmt drift in cue crates/cue/src/add/mod.rs:1 (anyhow import order) and crates/acuity/src/tests.rs — unrelated, left untouched.

## [235ceeb] Fix: null-like frontmatter values stay strings (review M1)

Addressed finding M1 from the diff-reviewer-opus review of feat/frontmatter-array-values, verified independently by consultant-opus.

coerce_scalar (crates/cue/src/add/mod.rs:113-126) only guarded YAML collection types (Mapping/Sequence/Tagged). Null tokens (null, ~, Null, NULL), comment-only (#c), and whitespace-only input silently coerced to YAML null. With the new array promotion, a stray null element also injected mid-sequence: refs=a.md -f refs=~ -f refs=b.md -> [a.md, null, b.md].

Added Ok(serde_yaml::Value::Null) to the collection guard so these inputs fall back to a literal string, matching the existing empty-value intent. This was pre-existing on master (not a regression) but amplified element-wise by the array feature.

consultant-opus downgraded severity Major -> Minor; recommended as a small hardening, which we applied. Residual whitespace-stripping (" a " -> "a") and hex coercion (0x1F -> 31) left as documented limitations per opus.

TDD: two regression tests added (scalar + list paths), confirmed RED then GREEN. All 38 add tests pass; clippy clean (pre-existing dir_flag warning only).

- **Found:** serde_yaml 0.9.34 follows YAML 1.1 (recognizes 0x hex, null tokens, # comments)
- **Found:** serde_yaml::from_str::<Value>('') returns Ok(Null) not Err; empty-string fallback reached only via explicit is_empty() guard at add/mod.rs:114
- **Found:** fmt import-ordering complaint in add/mod.rs and log/mod.rs is pre-existing at merge base, not introduced by this change
- **Decided:** Minimal fix: extend coerce_scalar guard with Ok(Null) arm rather than a broader literal-string-fidelity redesign
- **Decided:** Left whitespace-stripping and hex coercion as documented known limitations (lower impact, larger design change)
- **Decided:** Kept commit atomic: did not reformat pre-existing import-ordering fmt complaints unrelated to this fix

## [2aee220] Refactor: tighten frontmatter sequence promotion

Refactored the frontmatter sequence-promotion block in build_frontmatter_bytes (crates/cue/src/add/mod.rs:146-163).

Old form: nested if/else where the else arm did mem::replace(existing, Sequence) then re-matched the same reference in a second if-let that could never fail (a phantom branch that would silently drop first+elem if a future refactor broke the invariant).

New form: a total match — Sequence(seq) => push; other => mem::take(other) then *other = Sequence(vec![first, elem]). Exhaustive, no impossible branch, no Vec::with_capacity(2) micro-opt.

Used mem::take (confirms serde_yaml::Value: Default -> Null). Pure behavior-preserving refactor; all 38 add tests pass, clippy clean (pre-existing dir_flag warning only), my region fmt-clean.

Addresses review nits m1 (phantom branch) and m2 (with_capacity micro-opt).

- **Decided:** Switched from mem::replace(existing, Sequence(...)) to mem::take(other) + *other = Sequence(vec![...]) for a total, no-impossible-branch pattern
- **Decided:** Dropped the Vec::with_capacity(2) micro-optimization (review nit m2) since vec![] is equally clear for tiny one-shot lists

## [9a19ab5] Test: close M2 negative-path coverage

Closed the remaining M2 negative-path coverage gaps requested by the diff review. Added two regression tests in crates/cue/tests/add.rs:

1. test_add_frontmatter_list_collection_element_degrades_to_string: -f tags=alpha -f tags=[x, y] -> ["alpha", "[x, y]"]. Proves the collection guard applies element-wise (not just the colon case), so a collection-like element is forced to a literal string instead of nesting.

2. test_add_frontmatter_list_empty_element_stays_empty_string: -f tags= -f tags=b -> ["", "b"]. Proves the is_empty guard works element-wise inside a sequence (empty element is "" not null, and the slot is not skipped).

Both pass immediately (pinning already-correct behavior). All 40 add tests pass; clippy clean.

Review findings status: M1 fixed, M2 fully closed, m1/m2 fixed, m4 fixed. Only m3 (parse_fm test helper robustness) remains and is test-only/cosmetic.

- **Found:** Collection-like list element ([x, y]) correctly degrades to a literal string via the element-wise collection guard
- **Found:** Empty repeated-key element (tags=) yields the empty string element-wise via the is_empty guard, not null or a skipped slot

## [80601ab] fix(config): default artifact_types to canonical types (80601ab)

Root cause of "Unknown artifact type 'task'. Valid types: spec, trace, tmp, note": `Config::default().artifact_types` (crates/cuelib/src/config.rs:38) was a hardcoded 4-type subset, completely disconnected from `CANONICAL_TYPES` (crates/cuelib/src/artifact.rs:11-13, all 10 types). The `add` command validates against `config.artifact_types` (crates/cue/src/add/mod.rs:39), NOT against CANONICAL_TYPES. So out of the box `cue add -t task` (and plan/todo/doc/bin/ref) failed even though those types are canonical. With no cue.json present, Config falls to this default.

Fix: derive the default from CANONICAL_TYPES (`CANONICAL_TYPES.iter().map(|&s| s.to_string()).collect()`). Now every canonical type is a valid add target by default; configuring a subset remains opt-in via cue.json.

TDD: updated the existing `test_default_artifact_types` spec to assert all 10 canonical types, confirmed RED (left=4, right=10), applied fix, GREEN. Full workspace tests green (cuelib 50, cue all suites). cuelib clippy clean.

- **Found:** CANONICAL_TYPES (artifact.rs:11-13) existed with all 10 types but was only referenced in its own unit tests — dead to the add/validation path.
- **Found:** add validates against config.artifact_types (add/mod.rs:39), which comes from Config::default() when no cue.json overrides it.
- **Found:** Parallel disconnect still present: Config::default().ignored_types = ["tmp"] vs DEFAULT_IGNORED_TYPES const (artifact.rs:16) = ["tmp", "bin"]. Left out of scope (user instruction was specifically artifact_types).
- **Found:** Running `cue add` from outside the repo used a prebuilt nix-store binary (cue 0.1.0); the default is baked in at compile time, so the fix requires a rebuild/reinstall to take effect in that environment.
- **Decided:** Derived the default from CANONICAL_TYPES via iter/map rather than re-hardcoding the full list — single source of truth.
- **Decided:** Let `cargo fmt -p cuelib` normalize the whole file, which also tidied pre-existing figment import drift (Figment, before providers::) in the same use block. Deviates from the usual 'leave pre-existing drift' convention, but the partial multi-line vec edits were not persisting and the git-commit skill mandates running the formatter.
- **Open:** Config::default().ignored_types (["tmp"]) is still disconnected from DEFAULT_IGNORED_TYPES (["tmp", "bin"]) — same bug class, intentionally left for a separate change if desired.
- **Open:** Pre-built nix cue binary (0.1.0) needs rebuild/reinstall for this fix to reach users running `cue` outside the dev tree.

## [4eeda24] fix(config): default ignored_types to canonical (tmp, ref) — 4eeda24

Addressed issue 1 from the prior config-defaults investigation (the parallel disconnect I had left out of scope). Same bug class as 80601ab: Config::default().ignored_types (config.rs:40) was hardcoded ["tmp"], disconnected from the DEFAULT_IGNORED_TYPES const (artifact.rs:16).

Two-part fix:
1. Corrected the const itself: DEFAULT_IGNORED_TYPES changed from ["tmp", "bin"] to ["tmp", "ref"]. The const was the OUTLIER — multiple signals already pointed to tmp+ref: (a) the repo .gitignore ignores .ref + tmp (not bin); (b) the codebase's own test examples (config.rs test_ignored_types_json_override and init.rs test_init_gitignore_respects_config) already used ["tmp", "ref"]; (c) user's `cue config show` showed ["tmp", "ref"]. So `bin` should be a visible/committable artifact type; `ref` (externally-sourced reference material) belongs alongside `tmp` as ignored by default.
2. Config::default().ignored_types now derives from the const (DEFAULT_IGNORED_TYPES.iter().map(...).collect()), single source of truth — mirroring the 80601ab fix for artifact_types.

Effect: `cue init` (init/mod.rs:56-67) now generates .gitignore/.rgignore with `*/tmp/` and `*/ref/` patterns by default; `cue list` (list/mod.rs:235) hides tmp + ref by default.

TDD: updated artifact.rs const test + config.rs default test to assert ["tmp","ref"], confirmed RED for both (left had old values), applied fix, GREEN. Strengthened init.rs test_init_fresh_repo to assert both */tmp/ and */ref/ in gitignore + rgignore. Full workspace green (cuelib 50, cue all suites incl init 8). cuelib fmt clean, clippy clean (cue: only pre-existing dir_flag doc-comment warning).

Tooling note: the edit tool again wrote from a stale in-memory buffer of config.rs (predating 80601ab's `cargo fmt`), which reverted the figment import ordering on disk. Re-ran `cargo fmt -p cuelib` to restore; scoped to cuelib package only so the pre-existing add/mod.rs:1 and log/mod.rs:1 drift in the `cue` package was left untouched.

- **Found:** DEFAULT_IGNORED_TYPES=["tmp","bin"] was the lone outlier; .gitignore, test examples, and the user's own config all already used tmp+ref.
- **Found:** ignored_types has two runtime effects: `cue init` generates */<type>/ gitignore+rgignore patterns (init/mod.rs:56-67) and `cue list` hides them (list/mod.rs:235).
- **Found:** The edit tool persists from a stale in-memory buffer when the on-disk file was changed externally (by cargo fmt); re-running cargo fmt on disk reliably resyncs.
- **Decided:** Corrected DEFAULT_IGNORED_TYPES from ["tmp", "bin"] to ["tmp", "ref"] per user instruction — bin is a first-class visible artifact type; ref is the ignored companion to tmp.
- **Decided:** Derived Config::default().ignored_types from the const (single source of truth) rather than re-hardcoding the list.
- **Decided:** Scoped `cargo fmt -p cuelib` to the cuelib package to avoid reformatting pre-existing add/mod.rs:1 and log/mod.rs:1 drift in the `cue` package.

