---
status: open
priority: normal
---
---
status: open
priority: low
---
# Cross-binary --dir / --root naming inconsistency

Discovered during code review of `feat/cue-dir-flag` (consultant-opus).

## Problem

Two binaries in this repo implement the same concept — "override the
working directory" — with different flag names and value names:

| Binary   | Flag     | value_name | Validation              |
| -------- | -------- | ---------- | ----------------------- |
| `cue`    | `--dir`  | `PATH`     | metadata + canonicalize |
| `curator`| `--root` | `DIR`      | none (raw path)         |

`cue`'s `--dir` was added in commit a0f4f87 with validation and
canonicalization. `curator`'s `--root` predates it
(`crates/curator/src/main.rs:22-23`) and does no validation.

## Suggested resolution

Align both binaries on one flag name + value_name. Options:

1. Rename `curator --root` to `curator --dir` (breaking change for
   curator users — though curator is pre-1.0 and has few users).
2. Keep `--root` in curator and add `--dir` as an alias.
3. Leave as-is; document the inconsistency.

Whichever is chosen, `curator --root` should also gain the
metadata/canonicalize validation that `cue --dir` now has, so the
behaviour is consistent across both binaries.

## Defer reason

Not introduced by the `--dir` PR; pre-existing. Address when curator
is next touched (likely Phase 6).
