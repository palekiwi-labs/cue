---
status: complete
---
# Slice 0: cue CLI — frontmatter array values

## Foreword

Implements the CLI foundation for the `refs` story (task
`refs-frontmatter-array-values.md`). Makes `cue add -f KEY=VALUE` support
list-valued frontmatter via repeated-key semantics, so any field can be an
array of strings without a dedicated flag. Field-agnostic: the CLI knows
nothing about `refs` specifically.

The change is fully contained in `crates/cue/src/add/mod.rs::build_frontmatter_bytes`.
`parse_frontmatter_field`, the `--frontmatter` flag declaration, `AddOptions`,
`main.rs`, and `commands/add.rs` are all untouched (the `Vec<(String,String)>`
type is unchanged).

Consulted @consultant-opus: confirmed Option A (repeated key -> Sequence) is
the best general approach; serde_yaml::Mapping preserves insertion order;
clap 4.6.1 preserves same-flag occurrence order; no existing test relies on
the old last-wins overwrite.

Rules:
- single occurrence -> scalar (existing coerce logic)
- repeated key -> Sequence of coerced scalars (preserve order)
- zero occurrences -> key absent (NO empty-array support)

## Steps

- [x] Write failing tests in `crates/cue/tests/add.rs` (RED):
      repeated-key->list, single stays scalar, list element coercion,
      key ordering preserved, no-overwrite regression, list element with
      colon stays quoted, `=` inside list values.
- [x] Run tests, confirm RED. (5 list tests failed; scalar test passed.)
- [x] Extract `coerce_scalar` helper from current lines 113-119 of
      `add/mod.rs` (also: empty string -> `""` not `null`).
- [x] Rewrite `build_frontmatter_bytes`: single-pass group-by-key into the
      serde_yaml::Mapping via `get_mut` (first-seen key order, in-place
      promote scalar -> Sequence on second occurrence).
- [x] Run `cargo test -p cue`, confirm GREEN. (36/36 pass.)
- [x] `cargo clippy` + `cargo fmt`. (No new warnings; pre-existing fmt drift
      in add/mod.rs:1 import left untouched.)
- [x] Commit (GREEN). -> 8b1483c
