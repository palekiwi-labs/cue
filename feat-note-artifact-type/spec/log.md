# Project Log

## [5fbb684] note artifact type — cuelib/cue changes committed

The `feat/note-artifact-type` branch ships the Rust-side changes for the new `note` artifact type. A note is a spontaneous idea capture and conversation anchor — exploratory, not action-oriented. It dissolves into its outcome artifact (task, spec, doc) and is then closed. It has no `complete` status by design.

- **Decided:** NoteStatus lifecycle: open | in-progress | closed (no complete — notes dissolve, they do not complete)
- **Decided:** is_kanban_visible() returns false unconditionally for all NoteStatus variants — notes are not work items and never appear on the curator kanban board
- **Decided:** note added to both CANONICAL_TYPES and Config::default() artifact_types so it works out-of-box without cue.json
- **Decided:** NoteStatus rejects 'complete' as a valid status string — the test explicitly asserts this
- **Decided:** Fifth altitude named THINK for the skill documentation
- **Open:** cue-plugins side: create cue-note MCP tool, register in opencode.json, update SKILL.md with the THINK altitude and note artifact section
- **Open:** curator: a dedicated notes view (not kanban) may be worth building later — NoteStatus.is_kanban_visible() is a forward-compatible placeholder

## [5fbb684] cue-plugins note support committed on master

The cue-plugins side of the note artifact type is committed on master. The cue-note MCP tool, opencode.json registration, and comprehensive SKILL.md documentation (THINK altitude, note type section, tool docs, examples, hygiene rules) are all live.

- **Decided:** cue-note tool has no priority parameter (notes are not urgent)
- **Decided:** cue-note status enum is open|in-progress|closed with no complete state
- **Decided:** Fifth altitude named THINK in the skill documentation
- **Decided:** Out-of-scope guidance updated to distinguish todo (action-oriented) from note (exploration-oriented)

## [9fb2e64] Notes default to root-level storage with subdirectory grouping

The cue-note MCP tool now defaults to root-level storage (--root) instead of nesting under timestamp-hash directories. The nesting model provides no value for authored documents with meaningful filenames and actively prevents subdirectory organization. This enables note threads — related notes grouped under a named directory (e.g., note/auth-redesign/index.md).

- **Decided:** Notes default to root-level storage; nesting under <ts>-<hash> provides no value for authored documents
- **Decided:** Subdirectory grouping enables note threads: cue-note accepts paths like 'auth-redesign/index.md'
- **Decided:** The infrastructure already supported this — cue add --root with path separators works (validate_filename allows Normal components, fs::create_dir_all creates parents), and cue list's depth-aware parser correctly falls through to flat-artifact handling for non-timestamp directory names
- **Decided:** cue-note.ts passes --root unconditionally; no opt-out parameter needed. Users wanting anchoring can use cue-add --type note directly.

