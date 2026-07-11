// acuity: observability server for the cue ecosystem.
// Phase 5: query API + SSE stream added to Phase 3 ingest server.

mod config;
mod db;
#[cfg(test)]
mod tests;

use std::path::Path;

use acuity_schema::{AcuityEvent, SCHEMA_VERSION};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Query, State},
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    routing::get,
};
use serde::Deserialize;
use serde_json::json;
use sqlx::SqlitePool;
use tracing::{debug, error, info};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Clone)]
struct AppState {
    config: config::Config,
    /// `None` means Gotify notifications are disabled.
    gotify_token: Option<String>,
    http: reqwest::Client,
    db: SqlitePool,
}

/// Resolve the path to the SQLite events database.
///
/// Resolution order:
/// 1. `$ACUITY_DATA_DIR/acuity/events.db` if the env var is set.
/// 2. `<platform data dir>/acuity/events.db` (via `dirs::data_dir()`).
/// 3. `.local/share/acuity/events.db` relative to `$HOME` as last resort.
fn resolve_db_path() -> std::path::PathBuf {
    if let Ok(data_dir) = std::env::var("ACUITY_DATA_DIR") {
        return std::path::PathBuf::from(data_dir)
            .join("acuity")
            .join("events.db");
    }
    dirs::data_dir()
        .unwrap_or_else(|| {
            std::env::var("HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join(".local")
                .join("share")
        })
        .join("acuity")
        .join("events.db")
}

fn make_app(state: AppState) -> Router {
    Router::new()
        .route("/events", get(query_events).post(handle_event))
        .route("/events/stream", get(sse_handler))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// GET /events — paginated historical query
// ---------------------------------------------------------------------------

/// Query parameters accepted by `GET /events`.
#[derive(Debug, Deserialize)]
struct EventsQuery {
    /// Exclusive lower bound on `seq`. Defaults to 0 (return all events).
    #[serde(default)]
    after: i64,
    /// Maximum rows to return. Clamped to 1–500 server-side. Default 100.
    #[serde(default = "default_limit")]
    limit: i64,
    session_id: Option<String>,
    event_type: Option<String>,
    project_dir: Option<String>,
}

fn default_limit() -> i64 {
    100
}

async fn query_events(
    State(state): State<AppState>,
    Query(params): Query<EventsQuery>,
) -> Result<Json<acuity_api::EventsPage>, StatusCode> {
    // Normalise: negative after is harmless but confusing; clamp to 0.
    let after = params.after.max(0);

    match db::query_events_after(
        &state.db,
        after,
        params.limit,
        db::EventFilter {
            session_id: params.session_id.as_deref(),
            event_type: params.event_type.as_deref(),
            project_dir: params.project_dir.as_deref(),
        },
    )
    .await
    {
        Ok((events, next_after)) => Ok(Json(acuity_api::EventsPage { events, next_after })),
        Err(err) => {
            error!("query_events failed: {}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ---------------------------------------------------------------------------
// GET /events/stream — real-time SSE stream (poll-based)
// ---------------------------------------------------------------------------

/// Parse the `Last-Event-ID` header as an i64 seq cursor.
/// Returns 0 (start of stream) on absent or non-numeric values.
fn parse_last_event_id(headers: &HeaderMap) -> i64 {
    headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(0)
}

/// Rows fetched per SSE drain query.
const SSE_PAGE_SIZE: i64 = 50;

/// Maximum full pages drained per 500 ms poll cycle. Bounds catch-up so a
/// sustained write burst cannot monopolise the task and starve the keepalive
/// pings; backlog beyond this resumes from the last `seq` on the next cycle.
const SSE_MAX_DRAIN_PAGES: usize = 10;

async fn sse_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Sse<impl futures_core::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let cursor = parse_last_event_id(&headers).max(0);

    // Poll-based tailing. Correctness relies on a single writer (the ingest
    // path in `handle_event`): with `seq INTEGER PRIMARY KEY AUTOINCREMENT`,
    // assignment order equals commit order, so advancing the cursor past every
    // fetched `seq` never skips a row. Concurrent writers could diverge
    // assignment from commit order and race this poll loop -- not supported.
    let stream = async_stream::stream! {
        let mut seq = cursor;
        loop {
            // Drain buffered rows, but cap the iterations per cycle so a
            // sustained burst can't keep us here forever and starve the sleep
            // / keepalive below. A short page (or the cap) falls through to
            // the 500 ms sleep, then we poll again from the last `seq`.
            for _ in 0..SSE_MAX_DRAIN_PAGES {
                match db::query_events_after(
                    &state.db,
                    seq,
                    SSE_PAGE_SIZE,
                    db::EventFilter::default(),
                )
                .await
                {
                    Ok((records, next_after)) => {
                        let is_last_page = next_after.is_none();
                        for record in records {
                            seq = record.seq;
                            let data = match serde_json::to_string(&record) {
                                Ok(s) => s,
                                Err(err) => {
                                    error!(
                                        "sse: failed to serialize EventRecord: {}",
                                        err
                                    );
                                    continue;
                                }
                            };
                            yield Ok(Event::default()
                                .id(seq.to_string())
                                .data(data));
                        }
                        if is_last_page {
                            break;
                        }
                    }
                    Err(err) => {
                        error!("sse: db query failed: {}", err);
                        break;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(15)))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Stderr layer: quiet for the human operator.
    // Filter from RUST_LOG; default acuity=info.
    let stderr_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("acuity=info"));
    let stderr_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(stderr_filter);

    // Optional file layer: rich debug output for automated analysis.
    // Enabled by setting ACUITY_LOG_FILE to a writable path.
    // Filter from ACUITY_LOG_LEVEL; default acuity=debug.
    // The file is truncated on startup — each cargo run is a fresh
    // observable experiment. Set ACUITY_LOG_LEVEL to override verbosity.
    //
    // Security note: tool-call args and raw payloads (logged at DEBUG) may
    // contain secrets. The DB already stores the same bytes verbatim in the
    // payload column. Redaction is out of scope for the local-dev threat model.
    let file_layer = std::env::var("ACUITY_LOG_FILE").ok().map(|path| {
        let f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .unwrap_or_else(|e| panic!("ACUITY_LOG_FILE ({path}): {e}"));
        let writer = std::sync::Mutex::new(std::io::BufWriter::new(f));
        let file_filter = EnvFilter::try_from_env("ACUITY_LOG_LEVEL")
            .unwrap_or_else(|_| EnvFilter::new("acuity=debug"));
        fmt::layer()
            .with_writer(writer)
            .with_ansi(false)
            .with_filter(file_filter)
    });

    tracing_subscriber::registry()
        .with(stderr_layer)
        .with(file_layer)
        .init();

    if let Ok(path) = std::env::var("ACUITY_LOG_FILE") {
        info!("file logging enabled: {path}");
    }

    let cfg = config::Config::load()?;

    // ACUITY_GOTIFY_TOKEN is optional: presence enables notifications.
    let gotify_token = std::env::var("ACUITY_GOTIFY_TOKEN").ok();
    match &gotify_token {
        Some(_) => info!("Gotify notifications enabled"),
        None => info!("Gotify token not set, notifications disabled"),
    }
    if gotify_token.is_some() && cfg.gotify_url == config::Config::default().gotify_url {
        tracing::warn!(
            "ACUITY_GOTIFY_TOKEN is set but gotify_url is still the default; \
             notifications will likely fail"
        );
    }

    let db_path = resolve_db_path();
    info!("opening database at {}", db_path.display());
    let db = db::connect(&db_path).await?;

    let port = cfg.port;
    let state = AppState {
        config: cfg,
        gotify_token,
        http: reqwest::Client::new(),
        db,
    };

    let app = make_app(state);

    let addr = format!("0.0.0.0:{}", port);
    info!("acuity listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Compute a human-readable basename for a project directory path.
/// Trailing slashes are stripped first so "/foo/bar/" yields "bar".
/// Empty strings and the filesystem root fall back to "unknown".
fn basename(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return "unknown";
    }
    Path::new(trimmed)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(trimmed)
}

/// Send a fire-and-forget Gotify notification. Errors are logged, never
/// propagated — a Gotify failure must never affect the HTTP response.
async fn notify_gotify(
    http: reqwest::Client,
    url: String,
    token: String,
    title: String,
    message: String,
) {
    let payload = json!({
        "title": title,
        "message": message,
        "priority": 5,
    });

    match http
        .post(&url)
        .header("X-Gotify-Key", token)
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            info!("forwarded to Gotify: {}", title);
        }
        Ok(resp) => {
            error!("Gotify returned unexpected status: {}", resp.status());
        }
        Err(err) => {
            error!("failed to reach Gotify: {}", err);
        }
    }
}

async fn handle_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> StatusCode {
    // 1. Validate schema version header (parse as u8, not string compare)
    let schema_header = headers.get("x-acuity-schema").and_then(|v| v.to_str().ok());

    let version: u8 = match schema_header.and_then(|v| v.trim().parse().ok()) {
        Some(v) => v,
        None => {
            error!(
                "rejected event: missing or non-numeric X-Acuity-Schema \
                 header (expected {})",
                SCHEMA_VERSION
            );
            return StatusCode::BAD_REQUEST;
        }
    };
    if version != SCHEMA_VERSION {
        error!(
            "rejected event: X-Acuity-Schema {} != expected {}",
            version, SCHEMA_VERSION
        );
        return StatusCode::BAD_REQUEST;
    }

    // 2. Deserialize body as AcuityEvent (discriminated union)
    let event: AcuityEvent = match serde_json::from_slice(&body) {
        Ok(e) => e,
        Err(err) => {
            error!("rejected event: malformed body: {}", err);
            return StatusCode::UNPROCESSABLE_ENTITY;
        }
    };

    // 3. Timestamp (server-side). Formatting to ISO-8601 Z is done inside
    //    db::insert_event, which accepts DateTime<Utc> to enforce format.
    let received_at = chrono::Utc::now();

    // 4. Payload = raw request bytes (faithful copy, not re-serialized).
    //    serde_json::from_slice already validated UTF-8, so unwrap is safe.
    let payload = String::from_utf8(body.to_vec()).expect("serde_json validated UTF-8");

    // 5. Persist to SQLite — failure returns 500
    match db::insert_event(&state.db, &event, received_at, &payload).await {
        Ok(seq) => {
            // Per-variant structured fields so key data is individually
            // queryable in the log (not buried in a single summary string).
            // Option<String> uses `?` (Debug) — Option<T> has no Display impl.
            use acuity_schema::AcuityEvent as Ev;
            match &event {
                Ev::SessionUpdated(e) => info!(
                    seq,
                    event_type = "session_updated",
                    session_id = %e.session_id,
                    parent_id = ?e.parent_id,
                    agent = ?e.agent,
                    model = ?e.model,
                    title = ?e.title,
                    "persisted event"
                ),
                Ev::AgentTurnCompleted(e) => info!(
                    seq,
                    event_type = "agent_turn_completed",
                    session_id = %e.session_id,
                    turn_id = %e.turn_id,
                    input_tokens = ?e.input_tokens,
                    output_tokens = ?e.output_tokens,
                    model = ?e.model,
                    "persisted event"
                ),
                Ev::SessionIdle(e) => info!(
                    seq,
                    event_type = "session_idle",
                    session_id = %e.session_id,
                    title = ?e.session_title,
                    "persisted event"
                ),
                Ev::ToolCallRequested(e) => info!(
                    seq,
                    event_type = "tool_call_requested",
                    session_id = %e.session_id,
                    turn_id = %e.turn_id,
                    tool = %e.tool_name,
                    "persisted event"
                ),
                Ev::ToolCallCompleted(e) => info!(
                    seq,
                    event_type = "tool_call_completed",
                    session_id = %e.session_id,
                    turn_id = %e.turn_id,
                    tool = %e.tool_name,
                    is_error = e.is_error,
                    "persisted event"
                ),
            }
            // DEBUG: raw wire payload alongside the parsed event.
            // Comparing the two reveals whether a missing field is a plugin
            // bug (absent from payload) vs a schema bug (dropped in deser).
            debug!(seq, payload = %payload, event = ?event, "raw payload + parsed event");
        }
        Err(err) => {
            error!("failed to persist event: {}", err);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }

    // 6. Optional Gotify notification: only for SessionIdle, only when token
    //    is configured. Fire-and-forget via tokio::spawn.
    if let AcuityEvent::SessionIdle(ref idle) = event
        && let Some(token) = state.gotify_token.clone()
    {
        let http = state.http.clone();
        let url = format!("{}/message", state.config.gotify_url);
        let title = basename(&idle.project_dir).to_owned();
        let message = format!(
            "Idle: {}",
            idle.session_title.as_deref().unwrap_or(&idle.session_id)
        );
        tokio::spawn(async move {
            notify_gotify(http, url, token, title, message).await;
        });
    }

    StatusCode::OK
}
