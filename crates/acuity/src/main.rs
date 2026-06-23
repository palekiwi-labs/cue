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
use tracing::{error, info};

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
}

fn default_limit() -> i64 {
    100
}

async fn query_events(
    State(state): State<AppState>,
    Query(params): Query<EventsQuery>,
) -> Json<acuity_api::EventsPage> {
    // Normalise: negative after is harmless but confusing; clamp to 0.
    let after = params.after.max(0);

    match db::query_events_after(
        &state.db,
        after,
        params.limit,
        params.session_id.as_deref(),
        params.event_type.as_deref(),
    )
    .await
    {
        Ok(events) => Json(acuity_api::EventsPage { events }),
        Err(err) => {
            error!("query_events failed: {}", err);
            Json(acuity_api::EventsPage { events: vec![] })
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

async fn sse_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Sse<impl futures_core::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let cursor = parse_last_event_id(&headers).max(0);

    let stream = async_stream::stream! {
        let mut seq = cursor;
        loop {
            // Drain: keep fetching until a short page, then sleep.
            loop {
                match db::query_events_after(&state.db, seq, 50, None, None).await {
                    Ok(records) => {
                        let is_last_page = records.len() < 50;
                        for record in records {
                            seq = record.seq;
                            let data = match serde_json::to_string(&record) {
                                Ok(s) => s,
                                Err(err) => {
                                    error!("sse: failed to serialize EventRecord: {}", err);
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

    Sse::new(stream).keep_alive(
        KeepAlive::new().interval(std::time::Duration::from_secs(15)),
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "acuity=info".into()),
        )
        .init();

    let cfg = config::Config::load()?;

    // ACUITY_GOTIFY_TOKEN is optional: presence enables notifications.
    let gotify_token = std::env::var("ACUITY_GOTIFY_TOKEN").ok();
    match &gotify_token {
        Some(_) => info!("Gotify notifications enabled"),
        None => info!("Gotify token not set, notifications disabled"),
    }
    if gotify_token.is_some()
        && cfg.gotify_url == config::Config::default().gotify_url
    {
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
    let schema_header = headers
        .get("x-acuity-schema")
        .and_then(|v| v.to_str().ok());

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
    let payload =
        String::from_utf8(body.to_vec()).expect("serde_json validated UTF-8");

    // 5. Persist to SQLite — failure returns 500
    match db::insert_event(&state.db, &event, received_at, &payload).await {
        Ok(seq) => {
            info!(
                seq,
                event_type = event.event_type(),
                session_id = event.session_id(),
                "persisted event"
            );
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
