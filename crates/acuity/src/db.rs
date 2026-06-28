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
    project_dir TEXT    NOT NULL,
    harness     TEXT    NOT NULL,
    payload     TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_session
    ON events(session_id);
CREATE INDEX IF NOT EXISTS idx_events_turn
    ON events(turn_id) WHERE turn_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_events_project
    ON events(project_dir);
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

    // Startup column-mismatch guard: detect stale DB files created by an
    // older schema version. `CREATE TABLE IF NOT EXISTS` does not add new
    // columns to an existing table, so an old file silently keeps its old
    // structure. We check here rather than at insert time to fail fast with
    // a clear, actionable error message.
    {
        use sqlx::Row as _;
        let rows = sqlx::query("PRAGMA table_info(events)")
            .fetch_all(&pool)
            .await?;
        let columns: std::collections::HashSet<String> = rows
            .iter()
            .map(|row| row.get::<String, _>("name"))
            .collect();
        for col in ["project_dir", "harness"] {
            if !columns.contains(col) {
                tracing::error!(
                    "stale events.db detected — column `{}` missing; \
                     delete the file and restart",
                    col
                );
                return Err(anyhow::anyhow!(
                    "stale events.db detected — column `{}` missing; \
                     delete the file and restart",
                    col
                ));
            }
        }
    }

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
    let received_at_str = received_at.to_rfc3339_opts(SecondsFormat::Secs, true);
    let result = sqlx::query(
        "INSERT INTO events \
         (received_at, event_type, session_id, turn_id, project_dir, harness, payload) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(received_at_str)
    .bind(event.event_type())
    .bind(event.session_id())
    .bind(event.turn_id())
    .bind(event.project_dir())
    .bind(event.harness())
    .bind(payload)
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

/// Query events from the database with optional filters.
///
/// `after` is the exclusive lower bound on `seq` (use 0 to start from the
/// beginning). `limit` is clamped to `1..=500` server-side — callers must
/// not pass raw user-supplied integers. Optional `session_id` and
/// `event_type` filters apply as equality predicates.
///
/// Results are ordered by `seq` ascending so callers can use the last
/// returned `seq` as the next `after` cursor.
///
/// Returns the records plus a pagination cursor: `Some(last_record.seq)` when
/// the page was full (`records.len() == clamped_limit`, so more matching rows
/// may exist), or `None` on a short page (the final page). The caller decides
/// pagination from this cursor, not from the requested `limit`.
pub async fn query_events_after(
    pool: &SqlitePool,
    after: i64,
    limit: i64,
    session_id: Option<&str>,
    event_type: Option<&str>,
) -> sqlx::Result<(Vec<acuity_api::EventRecord>, Option<i64>)> {
    let clamped_limit = limit.clamp(1, 500);

    let mut builder = sqlx::QueryBuilder::new(
        "SELECT seq, received_at, event_type, session_id, turn_id, \
         project_dir, harness, payload \
         FROM events WHERE seq > ",
    );
    builder.push_bind(after);

    if let Some(sid) = session_id {
        builder.push(" AND session_id = ");
        builder.push_bind(sid);
    }
    if let Some(et) = event_type {
        builder.push(" AND event_type = ");
        builder.push_bind(et);
    }

    builder.push(" ORDER BY seq LIMIT ");
    builder.push_bind(clamped_limit);

    let rows = builder.build().fetch_all(pool).await?;

    use sqlx::Row as _;
    let records: Vec<acuity_api::EventRecord> = rows
        .into_iter()
        .map(|row| acuity_api::EventRecord {
            seq: row.get("seq"),
            received_at: row.get("received_at"),
            event_type: row.get("event_type"),
            session_id: row.get("session_id"),
            turn_id: row.get("turn_id"),
            project_dir: row.get("project_dir"),
            harness: row.get("harness"),
            payload: row.get("payload"),
        })
        .collect();

    // Full page -> more rows may exist; report the resume cursor. A short page
    // (incl. empty) is the final page. If exactly `clamped_limit` rows remain,
    // the client resumes and gets an empty page + None on the next call.
    let next_after = if records.len() as i64 == clamped_limit {
        records.last().map(|r| r.seq)
    } else {
        None
    };

    Ok((records, next_after))
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
        AcuityEvent, AgentTurnCompleted, SessionIdle, ToolCallCompleted, ToolCallRequested,
    };
    use serde_json::json;

    // --- schema guard tests ---

    #[tokio::test]
    async fn project_dir_column_exists_in_schema() {
        let pool = memory_pool().await;
        let rows = sqlx::query("PRAGMA table_info(events)")
            .fetch_all(&pool)
            .await
            .unwrap();
        let names: std::collections::HashSet<String> =
            rows.iter().map(|r| r.get::<String, _>("name")).collect();
        assert!(
            names.contains("project_dir"),
            "project_dir column missing from schema"
        );
        assert!(
            names.contains("harness"),
            "harness column missing from schema"
        );
    }

    #[tokio::test]
    async fn connect_stale_db_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("events.db");

        // Build an old-schema DB (no project_dir / harness columns).
        {
            let opts = SqliteConnectOptions::new()
                .filename(&db_path)
                .create_if_missing(true);
            let old_pool = sqlx::pool::PoolOptions::new()
                .max_connections(1)
                .connect_with(opts)
                .await
                .unwrap();
            sqlx::query(
                "CREATE TABLE events (
                    seq         INTEGER PRIMARY KEY AUTOINCREMENT,
                    received_at TEXT    NOT NULL,
                    event_type  TEXT    NOT NULL,
                    session_id  TEXT    NOT NULL,
                    turn_id     TEXT,
                    payload     TEXT    NOT NULL
                )",
            )
            .execute(&old_pool)
            .await
            .unwrap();
            old_pool.close().await;
        }

        // connect() must detect the stale file and return Err.
        let result = connect(&db_path).await;
        assert!(
            result.is_err(),
            "connect() should fail on a stale DB without project_dir/harness"
        );
    }

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
            harness: "opencode".into(),
            session_title: Some("hack".into()),
        });
        let raw = r#"{"type":"session_idle","session_id":"s1","project_dir":"/home/pl/code","harness":"opencode","session_title":"hack"}"#;

        let seq = insert_event(&pool, &ev, test_ts(), raw).await.unwrap();
        let (event_type, session_id, turn_id, payload, received_at) = fetch_row(&pool, seq).await;

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
            project_dir: "/home/pl/code".into(),
            harness: "opencode".into(),
            input_tokens: Some(120),
            output_tokens: Some(340),
        });
        let raw = r#"{"type":"agent_turn_completed","session_id":"s1","turn_id":"t1","project_dir":"/home/pl/code","harness":"opencode","input_tokens":120,"output_tokens":340}"#;

        let seq = insert_event(&pool, &ev, test_ts(), raw).await.unwrap();
        let (event_type, session_id, turn_id, _payload, _received_at) = fetch_row(&pool, seq).await;

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
            project_dir: "/home/pl/code".into(),
            harness: "opencode".into(),
            tool_call_id: "c1".into(),
            tool_name: "read".into(),
            args: json!({"path": "/x", "limit": 50}),
        });
        let raw = r#"{"type":"tool_call_requested","session_id":"s1","turn_id":"t1","project_dir":"/home/pl/code","harness":"opencode","tool_call_id":"c1","tool_name":"read","args":{"path":"/x","limit":50}}"#;

        let seq = insert_event(&pool, &ev, test_ts(), raw).await.unwrap();
        let (event_type, session_id, turn_id, payload, _received_at) = fetch_row(&pool, seq).await;

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
            project_dir: "/home/pl/code".into(),
            harness: "opencode".into(),
            tool_call_id: "c1".into(),
            tool_name: "bash".into(),
            is_error: true,
            error_text: Some("command not found: fd".into()),
        });
        let raw = r#"{"type":"tool_call_completed","session_id":"s1","turn_id":"t1","project_dir":"/home/pl/code","harness":"opencode","tool_call_id":"c1","tool_name":"bash","is_error":true,"error_text":"command not found: fd"}"#;

        let seq = insert_event(&pool, &ev, test_ts(), raw).await.unwrap();
        let (event_type, _session_id, turn_id, payload, _received_at) = fetch_row(&pool, seq).await;

        assert_eq!(event_type, "tool_call_completed");
        assert_eq!(turn_id.as_deref(), Some("t1"));
        // `is_error` must be preserved verbatim in the payload column
        assert!(payload.contains(r#""is_error":true"#));
        assert_eq!(payload, raw);
    }
}
