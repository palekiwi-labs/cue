---
status: complete
---
# Review Response Plan — Stage A

## Foreword

This plan evaluates each finding from the Stage A code review
(`.cue/feat-acuity-full-event-model/trace/1782119538-cc4cb0f/review-stage-a.md`)
against the Phase 3 design decisions recorded in `spec/log.md` and
`plan/phase-3.md`. For each finding, a disposition is assigned and action
items are listed.

The goal is not to address every nit in isolation, but to decide — before
touching code — which findings are genuine corrections and which are already
answered by the established design.

---

## Finding Dispositions

### M1 — `ToolCallCompleted` missing success-path output field

**Disposition: Accepted as a documentation fix; field addition deferred.**

The Phase 3 design log states: `payload column = raw request bytes (faithful
copy)`. The server stores the full raw JSON body verbatim. The schema structs
are deserialization targets, not data-access objects; a consumer wanting the
tool output reads `payload`, not the struct.

`ToolCallCompleted` omits `output` because:
1. Tool output can be arbitrarily large (stdout, file contents). Pulling it
   into a typed field forces the schema to carry a potentially huge `Value`.
2. Downstream analytics query `payload` directly (SQLite JSON functions);
   the typed field is not the retrieval path.

Action: Add a `// NOTE:` doc comment to `ToolCallCompleted` explaining the
intentional omission. No new field.

---

### M2 — Missing raw-wire deserialization test

**Disposition: Accept. Add one raw-wire test per variant.**

The plan prescribes "assert round-trip serde" and "event_type() equals the
`type` field produced by `serde_json::to_value`" — both are Rust-value-centric.
The review correctly identifies that the Axum handler will call
`serde_json::from_slice` on raw network bytes, not on a re-serialized Rust
value. Adding one raw-wire test per variant is low cost and closes a real
coverage gap.

Action: Add four `*_deserializes_from_raw_json` tests in `lib.rs`.

---

### M3 — `serde_json` as runtime dep — undocumented

**Disposition: Accept as documentation only.**

The design explicitly requires `serde_json::Value` for `args`. The dependency
is intentional. No feature-gating is warranted for a single-crate workspace
crate with no external consumers yet.

Action: Add a crate-level `//!` doc comment in `lib.rs` noting that
`serde_json` is a direct dep because `ToolCallRequested.args` requires
`Value`.

---

### m1 — TS distribution concern (all files, not just `types.ts`)

**Disposition: Defer. Out of scope for Stage A.**

The TS distribution strategy belongs to the plugin integration work
(Phase 4 / codegen consumer task). No code change in this PR.

---

### m2 — No `deny_unknown_fields` — forward-compat by omission

**Disposition: Accept as documentation.**

The design log decided: `no deny_unknown_fields` (forward-compatible
evolution). The review correctly flags that this must not be accidental.

Action: Add a doc comment to `AcuityEvent` noting the intentional omission.

---

### m3 — `PartialEq` on `Value` is key-order-sensitive

**Disposition: Accept as a comment / awareness note only.**

The round-trip tests are safe because serde_json preserves insertion order
through the serialize→deserialize cycle. No live bug exists. Adding a comment
noting the footgun near the `args` field is sufficient.

---

### n1 — `args` doc comment missing

**Disposition: Accept.**

Action: Add doc comment to `ToolCallRequested.args`.

---

### n2 — `SCHEMA_VERSION` doc comment missing

**Disposition: Accept.**

Action: Add doc comment.

---

### n3 — Test fixtures share `"s1"` / `"t1"`

**Disposition: Defer. Low value; distracting churn.**

The test names already identify the variant. Changing fixture values adds churn
without meaningfully improving diagnostics. Skip.

---

### n4 — `session_id_accessible_all_variants` — split into per-variant tests

**Disposition: Defer. Low value; minor style preference.**

The test is concise and clear. The inconsistency with `turn_id` tests is a
style preference, not a correctness issue. Skip for now.

---

## Action Items (Ordered)

- [x] **R1.** Add `//!` crate-level doc comment to `lib.rs` noting the
      `serde_json` dep is intentional (for `Value`).
- [x] **R2.** Add doc comment to `AcuityEvent` noting unknown fields are
      silently ignored for forward compatibility.
- [x] **R3.** Add doc comment to `ToolCallCompleted` explaining why
      `output` is intentionally absent (raw payload stores it).
- [x] **R4.** Add doc comment to `ToolCallRequested.args` noting `Value::Null`
   x  is the correct "no args" representation and the key-order footgun.
- [x] **R5.** Add doc comment to `SCHEMA_VERSION` clarifying it is an
   x  out-of-band wire version indicator, not a serialized event field.
- [x] **R6.** Add four raw-wire deserialization tests (one per variant)
   x  in `lib.rs` `#[cfg(test)]` mod.
- [x] **R7.** `cargo test -p acuity-schema` — green.
- [x] **R8.** `cargo clippy -p acuity-schema -- -D warnings` — clean.
- [x] **R9.** Commit with message `docs+test(acuity-schema): address Stage A review`.
