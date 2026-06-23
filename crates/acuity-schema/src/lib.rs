//! `acuity-schema` — wire types for the acuity event stream.
//!
//! `serde_json` is a direct dependency (not merely transitive) because
//! [`ToolCallRequested::args`] requires [`serde_json::Value`] to represent
//! arbitrary tool arguments without a fixed schema.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// Out-of-band wire version indicator sent in the `X-Acuity-Schema` HTTP
/// header. This constant is **not** embedded in serialized event payloads;
/// it is checked by the server before deserialization.
pub const SCHEMA_VERSION: u8 = 1;

/// Represents an opencode session that has gone idle.
/// Emitted by the acuity opencode plugin and consumed by the acuity server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export_to = "types.ts")]
pub struct SessionIdle {
    pub session_id: String,
    pub project_dir: String,
    pub session_title: Option<String>,
}

/// Emitted when an agent turn (LLM inference + tool calls) completes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export_to = "types.ts")]
pub struct AgentTurnCompleted {
    pub session_id: String,
    pub turn_id: String,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

/// Emitted when a tool call is dispatched by the agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export_to = "types.ts")]
pub struct ToolCallRequested {
    pub session_id: String,
    pub turn_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    /// Raw tool arguments as a JSON value.
    /// Use [`Value::Null`] when the tool takes no arguments.
    ///
    /// Note: [`Value::Object`] key order is preserved through
    /// serialize/deserialize but is not guaranteed stable across independent
    /// constructions; avoid `PartialEq` comparisons between two independently
    /// constructed `Value::Object` values if key insertion order may differ.
    pub args: Value,
}

/// Emitted when a tool call returns a result (or error).
///
/// The `output` field is intentionally absent. Tool output (stdout, file
/// contents, etc.) can be arbitrarily large and is stored verbatim in the
/// `payload` column of the events table. Consumers that need the raw output
/// read `payload` directly (e.g. via SQLite JSON functions). Duplicating it
/// here would bloat the schema type and the in-memory representation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export_to = "types.ts")]
pub struct ToolCallCompleted {
    pub session_id: String,
    pub turn_id: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub is_error: bool,
    pub error_text: Option<String>,
}

/// Harness-agnostic discriminated union of all acuity event types.
///
/// The `type` field in the JSON payload is the discriminant, using
/// snake_case values (e.g. `"session_idle"`, `"agent_turn_completed"`).
///
/// Unknown fields are silently ignored to allow forward-compatible evolution
/// of the plugin schema without requiring a server redeploy. Do **not** add
/// `#[serde(deny_unknown_fields)]` here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export_to = "types.ts")]
pub enum AcuityEvent {
    SessionIdle(SessionIdle),
    AgentTurnCompleted(AgentTurnCompleted),
    ToolCallRequested(ToolCallRequested),
    ToolCallCompleted(ToolCallCompleted),
}

impl AcuityEvent {
    /// Returns the snake_case discriminant string used in the wire format.
    pub fn event_type(&self) -> &'static str {
        match self {
            AcuityEvent::SessionIdle(_) => "session_idle",
            AcuityEvent::AgentTurnCompleted(_) => "agent_turn_completed",
            AcuityEvent::ToolCallRequested(_) => "tool_call_requested",
            AcuityEvent::ToolCallCompleted(_) => "tool_call_completed",
        }
    }

    /// Returns the `session_id` from whichever variant is active.
    pub fn session_id(&self) -> &str {
        match self {
            AcuityEvent::SessionIdle(e) => &e.session_id,
            AcuityEvent::AgentTurnCompleted(e) => &e.session_id,
            AcuityEvent::ToolCallRequested(e) => &e.session_id,
            AcuityEvent::ToolCallCompleted(e) => &e.session_id,
        }
    }

    /// Returns `None` for `SessionIdle`; `Some(&turn_id)` for all other variants.
    pub fn turn_id(&self) -> Option<&str> {
        match self {
            AcuityEvent::SessionIdle(_) => None,
            AcuityEvent::AgentTurnCompleted(e) => Some(&e.turn_id),
            AcuityEvent::ToolCallRequested(e) => Some(&e.turn_id),
            AcuityEvent::ToolCallCompleted(e) => Some(&e.turn_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn session_idle() -> AcuityEvent {
        AcuityEvent::SessionIdle(SessionIdle {
            session_id: "s1".into(),
            project_dir: "/home/pl/code".into(),
            session_title: Some("hack".into()),
        })
    }

    fn agent_turn_completed() -> AcuityEvent {
        AcuityEvent::AgentTurnCompleted(AgentTurnCompleted {
            session_id: "s1".into(),
            turn_id: "t1".into(),
            input_tokens: Some(120),
            output_tokens: Some(340),
        })
    }

    fn tool_call_requested() -> AcuityEvent {
        AcuityEvent::ToolCallRequested(ToolCallRequested {
            session_id: "s1".into(),
            turn_id: "t1".into(),
            tool_call_id: "c1".into(),
            tool_name: "read".into(),
            args: json!({"path": "/x", "limit": 50}),
        })
    }

    fn tool_call_completed() -> AcuityEvent {
        AcuityEvent::ToolCallCompleted(ToolCallCompleted {
            session_id: "s1".into(),
            turn_id: "t1".into(),
            tool_call_id: "c1".into(),
            tool_name: "bash".into(),
            is_error: true,
            error_text: Some("command not found: fd".into()),
        })
    }

    // --- round-trip serde ---

    #[test]
    fn session_idle_round_trip() {
        let ev = session_idle();
        let json = serde_json::to_string(&ev).unwrap();
        let back: AcuityEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(ev, back);
    }

    #[test]
    fn agent_turn_completed_round_trip() {
        let ev = agent_turn_completed();
        let json = serde_json::to_string(&ev).unwrap();
        let back: AcuityEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(ev, back);
    }

    #[test]
    fn tool_call_requested_round_trip() {
        let ev = tool_call_requested();
        let json = serde_json::to_string(&ev).unwrap();
        let back: AcuityEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(ev, back);
    }

    #[test]
    fn tool_call_completed_round_trip() {
        let ev = tool_call_completed();
        let json = serde_json::to_string(&ev).unwrap();
        let back: AcuityEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(ev, back);
    }

    // --- event_type() matches the serialized "type" field ---

    fn serialized_type(ev: &AcuityEvent) -> String {
        let v = serde_json::to_value(ev).unwrap();
        v["type"].as_str().unwrap().to_string()
    }

    #[test]
    fn event_type_matches_discriminant_session_idle() {
        let ev = session_idle();
        assert_eq!(ev.event_type(), serialized_type(&ev));
    }

    #[test]
    fn event_type_matches_discriminant_agent_turn_completed() {
        let ev = agent_turn_completed();
        assert_eq!(ev.event_type(), serialized_type(&ev));
    }

    #[test]
    fn event_type_matches_discriminant_tool_call_requested() {
        let ev = tool_call_requested();
        assert_eq!(ev.event_type(), serialized_type(&ev));
    }

    #[test]
    fn event_type_matches_discriminant_tool_call_completed() {
        let ev = tool_call_completed();
        assert_eq!(ev.event_type(), serialized_type(&ev));
    }

    // --- turn_id() accessor ---

    #[test]
    fn turn_id_none_for_session_idle() {
        assert_eq!(session_idle().turn_id(), None);
    }

    #[test]
    fn turn_id_some_for_agent_turn_completed() {
        assert_eq!(agent_turn_completed().turn_id(), Some("t1"));
    }

    #[test]
    fn turn_id_some_for_tool_call_requested() {
        assert_eq!(tool_call_requested().turn_id(), Some("t1"));
    }

    #[test]
    fn turn_id_some_for_tool_call_completed() {
        assert_eq!(tool_call_completed().turn_id(), Some("t1"));
    }

    // --- session_id() accessor ---

    #[test]
    fn session_id_accessible_all_variants() {
        assert_eq!(session_idle().session_id(), "s1");
        assert_eq!(agent_turn_completed().session_id(), "s1");
        assert_eq!(tool_call_requested().session_id(), "s1");
        assert_eq!(tool_call_completed().session_id(), "s1");
    }

    // --- raw wire-format deserialization (primary Axum handler path) ---

    #[test]
    fn session_idle_deserializes_from_raw_json() {
        let raw = r#"{"type":"session_idle","session_id":"s1","project_dir":"/home/pl/code","session_title":"hack"}"#;
        let ev: AcuityEvent = serde_json::from_str(raw).unwrap();
        assert_eq!(ev.event_type(), "session_idle");
        assert_eq!(ev.session_id(), "s1");
        assert_eq!(ev.turn_id(), None);
    }

    #[test]
    fn agent_turn_completed_deserializes_from_raw_json() {
        let raw = r#"{"type":"agent_turn_completed","session_id":"s1","turn_id":"t1","input_tokens":120,"output_tokens":340}"#;
        let ev: AcuityEvent = serde_json::from_str(raw).unwrap();
        assert_eq!(ev.event_type(), "agent_turn_completed");
        assert_eq!(ev.session_id(), "s1");
        assert_eq!(ev.turn_id(), Some("t1"));
    }

    #[test]
    fn tool_call_requested_deserializes_from_raw_json() {
        let raw = r#"{"type":"tool_call_requested","session_id":"s1","turn_id":"t1","tool_call_id":"c1","tool_name":"read","args":{"path":"/x","limit":50}}"#;
        let ev: AcuityEvent = serde_json::from_str(raw).unwrap();
        assert_eq!(ev.event_type(), "tool_call_requested");
        assert_eq!(ev.session_id(), "s1");
        assert_eq!(ev.turn_id(), Some("t1"));
    }

    #[test]
    fn tool_call_completed_deserializes_from_raw_json() {
        let raw = r#"{"type":"tool_call_completed","session_id":"s1","turn_id":"t1","tool_call_id":"c1","tool_name":"bash","is_error":true,"error_text":"command not found: fd"}"#;
        let ev: AcuityEvent = serde_json::from_str(raw).unwrap();
        assert_eq!(ev.event_type(), "tool_call_completed");
        assert_eq!(ev.session_id(), "s1");
        assert_eq!(ev.turn_id(), Some("t1"));
    }

    // --- forward-compat: unknown fields are silently ignored ---

    #[test]
    fn unknown_fields_are_ignored_on_deserialization() {
        let raw = r#"{"type":"session_idle","session_id":"s1","project_dir":"/p","session_title":null,"future_field":"ignored"}"#;
        let ev: AcuityEvent = serde_json::from_str(raw).unwrap();
        assert_eq!(ev.event_type(), "session_idle");
        assert_eq!(ev.session_id(), "s1");
    }
}
