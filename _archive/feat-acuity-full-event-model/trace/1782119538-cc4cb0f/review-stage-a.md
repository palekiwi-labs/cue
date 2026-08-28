# Code Review: acuity-schema Stage A

Reviewed by: diff-reviewer-sonnet
Commit: cc4cb0f
Branch: feat/acuity-full-event-model

---

## Summary

The implementation is solid for a first pass. All 13 tests pass, the serde
tag/rename setup is correct, and `export_all` on `AcuityEvent` correctly pulls
in all four inner struct types via ts-rs 12. The major concern is a semantic gap
in `ToolCallCompleted` (missing a success-path output field), two notable
correctness risks around `#[serde(deny_unknown_fields)]` and internally-tagged
enum + `Value` deserialization, and a cluster of smaller issues around
`serde_json` being a runtime dependency in a pure schema crate, `Clone`/
`PartialEq` justification, and test coverage gaps.

---

## Issues

### Critical

None confirmed.

---

### Major

#### M1. `ToolCallCompleted` missing success-path output field
`crates/acuity-schema/src/lib.rs:37-44`

The struct models the error path but has no field for the tool's output on the
success path. A caller receiving `ToolCallCompleted` where `is_error = false`
and `error_text = None` has no output to inspect. If this is intentional (output
is stored raw in `payload` and not needed in the schema), it must be explicitly
documented. If output will be needed later, adding it now avoids a breaking
schema change.

Suggested: either add `pub output: Option<Value>` or add a doc comment
explaining the omission.

---

#### M2. Internally-tagged enum + `serde_json::Value` — latent deserialization hazard
`crates/acuity-schema/src/lib.rs:51-58`

Tests only exercise the round-trip path (serialize-then-deserialize the Rust
value). They do not exercise the Axum handler path: deserializing raw bytes
arriving from the plugin. This is the primary production use case. A raw-wire
input test is essential.

Suggested additional test:

```rust
#[test]
fn tool_call_requested_deserializes_from_raw_wire_bytes() {
    let raw = r#"{"type":"tool_call_requested","session_id":"s1","turn_id":"t1",
        "tool_call_id":"c1","tool_name":"read","args":{"path":"/x","limit":50}}"#;
    let ev: AcuityEvent = serde_json::from_str(raw).unwrap();
    assert_eq!(ev.session_id(), "s1");
    assert_eq!(ev.event_type(), "tool_call_requested");
}
```

---

#### M3. `serde_json` as a runtime dependency — undocumented decision
`crates/acuity-schema/Cargo.toml:8`

Pulls `serde_json` into every downstream crate. The decision to use `Value`
directly (rather than a feature-gated newtype or `String`) is reasonable but
should be documented at the crate level or via a comment on the field.

---

### Minor

#### m1. `#[ts(export_to = "types.ts")]` only on enum — confirmed sufficient, but distribution concern

Each inner struct emits to its own `.ts` file; `types.ts` imports from siblings.
If the consuming plugin bundles only `types.ts`, it will get broken TypeScript.
The codegen README or plugin build step must document that all files in `dist/`
must be copied, not just `types.ts`.

No code change required; documentation recommended.

---

#### m2. No `#[serde(deny_unknown_fields)]` — forward-compatibility by omission

Without `deny_unknown_fields`, unknown fields are silently ignored — the right
forward-compatibility choice. But this must be a documented decision, not an
accidental omission, to prevent a future developer from adding it.

Suggested doc comment on `AcuityEvent`:

```
// Unknown fields are silently ignored to allow forward-compatible evolution
// of the plugin schema without requiring a server redeploy.
```

---

#### m3. `PartialEq` on `Value` fields is key-order-sensitive

`serde_json::Value::Object` compares unequal if key insertion order differs.
Round-trip tests pass today because serialize/deserialize preserves order, but
any test constructing `args` via two `json!()` literals with different key orders
will produce false negatives. Document the footgun or use order-insensitive
assertions for `Value` comparisons.

---

### Nit

#### n1. `ToolCallRequested.args: Value` vs `Option<Value>` — correct but undocumented

`Value::Null` is a valid "no args" representation. Add a doc comment:
`/// Raw tool arguments as a JSON value. Use Value::Null when the tool takes no arguments.`

---

#### n2. `SCHEMA_VERSION` has no doc comment

Not embedded in event payloads. Needs a comment clarifying it is an out-of-band
wire version indicator used by the handler/DB, not a serialized field.

---

#### n3. Test fixtures share identical `session_id`/`turn_id` values

All use `"s1"` and `"t1"`. Per-fixture distinct identifiers improve failure
diagnostics.

---

#### n4. `session_id_accessible_all_variants` — lumps 4 variants in one test

Inconsistent with `turn_id` tests which are one-per-variant. Split for
consistent failure attribution.

---

## Verdict

**Approve with changes.** The foundation is correct. Two items should be
addressed before this schema is used in production:

1. Clarify or add a success-path output field to `ToolCallCompleted`.
2. Add at least one test deserializing from a raw wire-format string (not a
   round-tripped Rust value) to validate the primary Axum handler use case.

Remaining items are documentation gaps and minor style issues.
