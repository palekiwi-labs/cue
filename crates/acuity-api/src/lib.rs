//! `acuity-api` — read/response types for acuity's query API and SSE stream.
//!
//! Used by both `acuity` (serialization) and `curator` (deserialization).
//! Kept dependency-light so `curator` can depend on this crate without
//! pulling in the server stack.
//!
//! `AcuityEvent` is re-exported here so consumers that need to parse the
//! `payload` field of an `EventRecord` only need to depend on `acuity-api`,
//! not on both `acuity-api` and `acuity-schema`.

use serde::{Deserialize, Serialize};

pub use acuity_schema::AcuityEvent;

/// One row from the `events` table.
///
/// The `payload` field contains the raw JSON wire bytes as they arrived from
/// the plugin — a faithful copy of the request body. Callers that need the
/// structured event can deserialize it with
/// `serde_json::from_str::<AcuityEvent>(&record.payload)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventRecord {
    pub seq: i64,
    /// ISO-8601 UTC timestamp, e.g. `"2026-06-22T10:00:00Z"`.
    pub received_at: String,
    /// Snake-case discriminant: `"session_idle"`, `"agent_turn_completed"`,
    /// `"tool_call_requested"`, or `"tool_call_completed"`.
    pub event_type: String,
    pub session_id: String,
    pub turn_id: Option<String>,
    /// Raw JSON wire bytes (the original request body, not re-serialized).
    pub payload: String,
}

/// Response body for `GET /events`.
///
/// Pagination contract: loop, calling `GET /events?after=<cursor>`, until
/// `next_after` is `None`. After each page set `after` to `next_after` (the
/// `seq` of the last returned record). `next_after` is `Some(seq)` when the
/// page was full — meaning more matching rows may exist — and `None` once a
/// short page is returned (the final page).
///
/// The server decides "is there more?" so the client never depends on the
/// server's internal page-size clamp. Do NOT infer the end of the result set
/// from `events.len() == requested limit`: the server may clamp `limit` below
/// what the client requested.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventsPage {
    pub events: Vec<EventRecord>,
    /// `Some(last_record.seq)` when this page is full (more rows may follow);
    /// `None` on the final page. Use as the next `after` cursor.
    #[serde(default)]
    pub next_after: Option<i64>,
}
