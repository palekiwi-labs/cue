# Project Log

## [27cecdc-dirty] GREEN: fix colon-in-string YAML frontmatter bug

Committed fix for the silent YAML frontmatter bug where titles containing ': ' were emitted as mappings instead of quoted strings. Single surgical change to build_frontmatter_bytes in crates/cue/src/add/mod.rs.

- **Found:** serde_yaml::from_str('foo: bar') succeeds and returns a Mapping, so the unwrap_or_else fallback never fired
- **Found:** The bug affected ALL free-form string fields (title, branch, etc.) because they all go through the same build_frontmatter_bytes path
- **Found:** cargo fmt had pre-existing drift in several unrelated files — staged only the intentional changes
- **Decided:** Guard by inspecting the parsed Value variant: coerce Mapping and Sequence back to String, pass scalars through unchanged
- **Decided:** Stage only crates/cue/src/add/mod.rs and crates/cue/tests/add.rs — leave pre-existing fmt drift unstaged
- **Decided:** Test covers both title and branch fields to demonstrate path-wide fix (AC #1 and #2)

## [542ffbb-dirty] Review fixes committed: Tagged variant + test slice correctness

- **Found:** serde_yaml::Value::Tagged was not covered by the original match arm — would emit !Tag syntax for tagged user strings
- **Found:** Frontmatter slice in the round-trip test included the closing ---\n; test passed accidentally because YAML tolerates doc-end markers
- **Decided:** Added Tagged to the forced-String arm alongside Mapping and Sequence
- **Decided:** Rewrote fm_start/fm_end extraction to use correct byte offsets (stop at delimiter, not past it)
- **Decided:** Comment updated to say 'collection syntax' rather than just ': '

