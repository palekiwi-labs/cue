//! SQLite persistence layer for acuity events.

use std::path::Path;

use acuity_schema::AcuityEvent;
use chrono::{DateTime, SecondsFormat, Utc};
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};

/// DDL executed once at startup to ensure the events table and its indexes
/// exist. `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS` make
/// this idempotent on subsequent starts.
pub const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS events (
    seq         INTEGER PRIMARY KEY AUTOINCREMENT,
    received_at TEXT    NOT NULL,
    event_type  TEXT    NOT NULL,
    session_id  TEXT    NOT NULL,
    turn_id     TEXT,
    payload     TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_session
    ON events(session_id);
CREATE INDEX IF NOT EXISTS idx_events_turn
    ON events(turn_id) WHERE turn_id IS NOT NULL;
";

/// Open (or create) a SQLite pool at `path` and apply the schema.
///
/// `create_if_missing(true)` is required — without it `connect()` fails when
/// the DB file does not yet exist.
pub async fn connect(path: &Path) -> anyhow::Result<SqlitePool> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let opts = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(opts).await?;
    sqlx::query(SCHEMA_SQL).execute(&pool).await?;
    Ok(pool)
}

/// Insert one `AcuityEvent` row and return its `rowid` / `seq`.
///
/// `received_at` is formatted to an ISO-8601 UTC string with seconds
/// precision and a `Z` suffix (e.g. `"2026-06-22T10:00:00Z"`) inside
/// this function — the type enforces correct format at the call boundary.
/// `payload` must be the raw request body bytes cast to UTF-8 — a faithful
/// copy of the wire bytes, **not** a re-serialization of the deserialized enum.
pub async fn insert_event(
    pool: &SqlitePool,
    event: &AcuityEvent,
    received_at: DateTime<Utc>,
    payload: &str,
) -> sqlx::Result<i64> {
    let received_at_str =
        received_at.to_rfc3339_opts(SecondsFormat::Secs, true);
    let result = sqlx::query(
        "INSERT INTO events (received_at, event_type, session_id, turn_id, payload)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(received_at_str)
    .bind(event.event_type())
    .bind(event.session_id())
    .bind(event.turn_id())
    .bind(payload)
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

/// Test-only: create an in-memory SQLite pool with the schema applied.
///
/// `max_connections(1)` is required — each connection to `:memory:` gets its
/// own isolated database, so multiple connections would silently diverge.
#[cfg(test)]
pub async fn memory_pool() -> SqlitePool {
    let opts = SqliteConnectOptions::new()
        .filename(":memory:")
        .create_if_missing(true);

    let pool = sqlx::pool::PoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("failed to open in-memory SQLite pool");

    sqlx::query(SCHEMA_SQL)
        .execute(&pool)
        .await
        .expect("failed to apply schema to in-memory pool");

    pool
}

#[cfg(test)]
mod tests {
    use sqlx::Row as _;

    use super::*;
    use acuity_schema::{
        AgentTurnCompleted, AcuityEvent, SessionIdle, ToolCallCompleted,
        ToolCallRequested,
    };
    use serde_json::json;

    fn test_ts() -> DateTime<Utc> {
        "2026-06-22T10:00:00Z"
            .parse::<DateTime<Utc>>()
            .expect("fixed test timestamp must parse")
    }

    // --- helpers ---

    async fn fetch_row(
        pool: &SqlitePool,
        seq: i64,
    ) -> (String, String, Option<String>, String, String) {
        sqlx::query(
            "SELECT event_type, session_id, turn_id, payload, received_at \
             FROM events WHERE seq = ?",
        )
        .bind(seq)
        .map(|row: sqlx::sqlite::SqliteRow| {
            (
                row.get::<String, _>("event_type"),
                row.get::<String, _>("session_id"),
                row.get::<Option<String>, _>("turn_id"),
                row.get::<String, _>("payload"),
                row.get::<String, _>("received_at"),
            )
        })
        .fetch_one(pool)
        .await
        .unwrap()
    }

    // --- C2 tests ---

    #[tokio::test]
    async fn insert_session_idle_turn_id_null() {
        let pool = memory_pool().await;
        let ev = AcuityEvent::SessionIdle(SessionIdle {
            session_id: "s1".into(),
            project_dir: "/home/pl/code".into(),
            session_title: Some("hack".into()),
        });
        let raw = r#"{"type":"session_idle","session_id":"s1","project_dir":"/home/pl/code","session_title":"hack"}"#;

        let seq = insert_event(&pool, &ev, test_ts(), raw).await.unwrap();
        let (event_type, session_id, turn_id, payload, received_at) =
            fetch_row(&pool, seq).await;

        assert_eq!(event_type, "session_idle");
        assert_eq!(session_id, "s1");
        assert!(turn_id.is_none(), "turn_id must be NULL for SessionIdle");
        assert_eq!(payload, raw);
        // ISO-8601 UTC with seconds precision and Z suffix
        assert_eq!(received_at, "2026-06-22T10:00:00Z");
    }

    #[tokio::test]
    async fn insert_agent_turn_completed_turn_id_populated() {
        let pool = memory_pool().await;
        let ev = AcuityEvent::AgentTurnCompleted(AgentTurnCompleted {
            session_id: "s1".into(),
            turn_id: "t1".into(),
            input_tokens: Some(120),
            output_tokens: Some(340),
        });
        let raw = r#"{"type":"agent_turn_completed","session_id":"s1","turn_id":"t1","input_tokens":120,"output_tokens":340}"#;

        let seq = insert_event(&pool, &ev, test_ts(), raw).await.unwrap();
        let (event_type, session_id, turn_id, _payload, _received_at) =
            fetch_row(&pool, seq).await;

        assert_eq!(event_type, "agent_turn_completed");
        assert_eq!(session_id, "s1");
        assert_eq!(turn_id.as_deref(), Some("t1"));
    }

    #[tokio::test]
    async fn insert_tool_call_requested_fields_correct() {
        let pool = memory_pool().await;
        let ev = AcuityEvent::ToolCallRequested(ToolCallRequested {
            session_id: "s1".into(),
            turn_id: "t1".into(),
            tool_call_id: "c1".into(),
            tool_name: "read".into(),
            args: json!({"path": "/x", "limit": 50}),
        });
        let raw = r#"{"type":"tool_call_requested","session_id":"s1","turn_id":"t1","tool_call_id":"c1","tool_name":"read","args":{"path":"/x","limit":50}}"#;

        let seq = insert_event(&pool, &ev, test_ts(), raw).await.unwrap();
        let (event_type, session_id, turn_id, payload, _received_at) =
            fetch_row(&pool, seq).await;

        assert_eq!(event_type, "tool_call_requested");
        assert_eq!(session_id, "s1");
        assert_eq!(turn_id.as_deref(), Some("t1"));
        assert_eq!(payload, raw);
    }

    #[tokio::test]
    async fn insert_tool_call_completed_is_error_in_payload() {
        let pool = memory_pool().await;
        let ev = AcuityEvent::ToolCallCompleted(ToolCallCompleted {
            session_id: "s1".into(),
            turn_id: "t1".into(),
            tool_call_id: "c1".into(),
            tool_name: "bash".into(),
            is_error: true,
            error_text: Some("command not found: fd".into()),
        });
        let raw = r#"{"type":"tool_call_completed","session_id":"s1","turn_id":"t1","tool_call_id":"c1","tool_name":"bash","is_error":true,"error_text":"command not found: fd"}"#;

        let seq = insert_event(&pool, &ev, test_ts(), raw).await.unwrap();
        let (event_type, _session_id, turn_id, payload, _received_at) =
            fetch_row(&pool, seq).await;

        assert_eq!(event_type, "tool_call_completed");
        assert_eq!(turn_id.as_deref(), Some("t1"));
        // `is_error` must be preserved verbatim in the payload column
        assert!(payload.contains(r#""is_error":true"#));
        assert_eq!(payload, raw);
    }
}
