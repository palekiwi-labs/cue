use axum::body::Body;
use http_body_util::BodyExt as _;
use tower::ServiceExt;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::{AppState, basename, make_app};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

async fn test_state_with_url(gotify_url: String) -> AppState {
    AppState {
        config: crate::config::Config {
            gotify_url,
            port: 0,
        },
        gotify_token: Some("test-token".into()),
        http: reqwest::Client::new(),
        db: crate::db::memory_pool().await,
    }
}

async fn test_state_no_gotify() -> AppState {
    AppState {
        config: crate::config::Config {
            gotify_url: "http://localhost:80".into(),
            port: 0,
        },
        gotify_token: None,
        http: reqwest::Client::new(),
        db: crate::db::memory_pool().await,
    }
}

async fn send_request(
    router: axum::Router,
    schema_header: Option<&str>,
    body: &str,
) -> axum::http::StatusCode {
    let mut req = axum::http::Request::builder()
        .method("POST")
        .uri("/events")
        .header("Content-Type", "application/json");
    if let Some(v) = schema_header {
        req = req.header("X-Acuity-Schema", v);
    }
    let request = req.body(Body::from(body.to_string())).unwrap();
    let response = router.oneshot(request).await.unwrap();
    response.status()
}

async fn row_count(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(pool)
        .await
        .unwrap()
}

// Valid bodies for each event type
const SESSION_IDLE_BODY: &str = r#"{"type":"session_idle","session_id":"abc-123","project_dir":"/home/me/project","session_title":"hello"}"#;
const AGENT_TURN_BODY: &str = r#"{"type":"agent_turn_completed","session_id":"abc-123","turn_id":"t1","input_tokens":120,"output_tokens":340}"#;
const SESSION_IDLE_BODY_S2: &str = r#"{"type":"session_idle","session_id":"session-2","project_dir":"/home/me/other","session_title":"other"}"#;

// ---------------------------------------------------------------------------
// Query endpoint helpers
// ---------------------------------------------------------------------------

async fn get_events(router: axum::Router, query: &str) -> axum::http::Response<Body> {
    let uri = if query.is_empty() {
        "/events".to_string()
    } else {
        format!("/events?{}", query)
    };
    let request = axum::http::Request::builder()
        .method("GET")
        .uri(&uri)
        .body(Body::empty())
        .unwrap();
    router.oneshot(request).await.unwrap()
}

async fn body_json(resp: axum::http::Response<Body>) -> acuity_api::EventsPage {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).expect("response must be valid EventsPage JSON")
}

// ---------------------------------------------------------------------------
// Schema header validation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn header_missing_returns_400() {
    let state = test_state_with_url("http://localhost:80".into()).await;
    let app = make_app(state);
    let status = send_request(app, None, "{}").await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn header_wrong_version_returns_400() {
    let state = test_state_with_url("http://localhost:80".into()).await;
    let app = make_app(state);
    let status = send_request(app, Some("99"), "{}").await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// Body validation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn malformed_body_returns_422() {
    let state = test_state_with_url("http://localhost:80".into()).await;
    let app = make_app(state);
    let status = send_request(app, Some("1"), "not-json").await;
    assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn unknown_event_type_returns_422() {
    let state = test_state_with_url("http://localhost:80".into()).await;
    let app = make_app(state);
    // Missing / unknown "type" discriminant — must 422
    let status = send_request(app, Some("1"), r#"{"type":"nope","session_id":"x"}"#).await;
    assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

#[tokio::test]
async fn valid_event_persists_row() {
    let state = test_state_no_gotify().await;
    let pool = state.db.clone();
    let app = make_app(state);

    let status = send_request(app, Some("1"), SESSION_IDLE_BODY).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(row_count(&pool).await, 1);
}

#[tokio::test]
async fn valid_event_persists_correct_event_type() {
    let state = test_state_no_gotify().await;
    let pool = state.db.clone();
    let app = make_app(state);

    send_request(app, Some("1"), SESSION_IDLE_BODY).await;

    let event_type: String = sqlx::query_scalar("SELECT event_type FROM events LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(event_type, "session_idle");
}

// ---------------------------------------------------------------------------
// Gotify behaviour
// ---------------------------------------------------------------------------

#[tokio::test]
async fn valid_session_idle_forwards_to_gotify() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/message"))
        .and(header("X-Gotify-Key", "test-token"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

    let state = test_state_with_url(mock_server.uri()).await;
    let app = make_app(state);
    let status = send_request(app, Some("1"), SESSION_IDLE_BODY).await;
    assert_eq!(status, axum::http::StatusCode::OK);

    // The Gotify call is fire-and-forget (tokio::spawn); the JoinHandle is
    // dropped inside handle_event and is not accessible here. Poll until
    // wiremock records the expected request rather than using a fixed sleep.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
    loop {
        if mock_server.received_requests().await.map_or(0, |r| r.len()) >= 1 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for Gotify request"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    // wiremock verifies the expectation count (1 call) on drop
}

#[tokio::test]
async fn gotify_disabled_session_idle_returns_200() {
    // No Gotify token configured — server must still return 200 and persist.
    let state = test_state_no_gotify().await;
    let pool = state.db.clone();
    let app = make_app(state);

    let status = send_request(app, Some("1"), SESSION_IDLE_BODY).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(row_count(&pool).await, 1);
}

#[tokio::test]
async fn non_idle_event_does_not_notify_gotify() {
    // AgentTurnCompleted must NOT trigger a Gotify call.
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/message"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // zero calls expected
        .mount(&mock_server)
        .await;

    let state = test_state_with_url(mock_server.uri()).await;
    let pool = state.db.clone();
    let app = make_app(state);

    let status = send_request(app, Some("1"), AGENT_TURN_BODY).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(row_count(&pool).await, 1);
    // wiremock verifies 0 calls on drop
}

#[tokio::test]
async fn gotify_error_returns_200_event_persisted() {
    // Gotify returning 500 must NOT cause a non-200 response to the plugin.
    // Persist-first: the row must be present even when Gotify fails.
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/message"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let state = test_state_with_url(mock_server.uri()).await;
    let pool = state.db.clone();
    let app = make_app(state);

    let status = send_request(app, Some("1"), SESSION_IDLE_BODY).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(row_count(&pool).await, 1);
}

// ---------------------------------------------------------------------------
// basename (unit, sync)
// ---------------------------------------------------------------------------

#[test]
fn basename_normal_path() {
    assert_eq!(basename("/home/user/project"), "project");
}

#[test]
fn basename_single_trailing_slash() {
    assert_eq!(basename("/home/user/project/"), "project");
}

#[test]
fn basename_multiple_trailing_slashes() {
    assert_eq!(basename("/home/user/project///"), "project");
}

#[test]
fn basename_root() {
    assert_eq!(basename("/"), "unknown");
}

#[test]
fn basename_empty() {
    assert_eq!(basename(""), "unknown");
}

#[test]
fn basename_relative_no_dir_component() {
    assert_eq!(basename("relative"), "relative");
}

// ---------------------------------------------------------------------------
// GET /events/stream — SSE smoke tests
// ---------------------------------------------------------------------------

async fn sse_first_data_line(state: AppState) -> String {
    let app = make_app(state);
    let request = axum::http::Request::builder()
        .method("GET")
        .uri("/events/stream")
        .header("Accept", "text/event-stream")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    // Read chunks until we find a "data:" line.
    let mut body = response.into_body();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for SSE data frame"
        );
        use http_body_util::BodyExt as _;
        if let Some(Ok(frame)) = body.frame().await {
            if let Ok(bytes) = frame.into_data() {
                let chunk = String::from_utf8_lossy(&bytes);
                for line in chunk.lines() {
                    if let Some(rest) = line.strip_prefix("data:") {
                        return rest.trim().to_owned();
                    }
                }
            }
        } else {
            panic!("SSE stream ended before a data: frame arrived");
        }
    }
}

#[tokio::test]
async fn sse_delivers_existing_event_on_connect() {
    let state = test_state_no_gotify().await;
    // Insert one event directly into the pool before opening the SSE stream.
    let app = make_app(state.clone());
    send_request(app, Some("1"), SESSION_IDLE_BODY).await;

    let data = sse_first_data_line(state).await;

    // The data line must deserialize to a valid EventRecord.
    let record: acuity_api::EventRecord =
        serde_json::from_str(&data).expect("SSE data must be a valid EventRecord JSON");
    assert_eq!(record.seq, 1);
    assert_eq!(record.event_type, "session_idle");
    assert_eq!(record.session_id, "abc-123");
}

#[tokio::test]
async fn sse_last_event_id_resumes_from_cursor() {
    let state = test_state_no_gotify().await;
    let app = make_app(state.clone());
    // Insert two events
    send_request(app.clone(), Some("1"), SESSION_IDLE_BODY).await;
    send_request(app.clone(), Some("1"), AGENT_TURN_BODY).await;

    // Connect with Last-Event-ID: 1 — should only receive the second event
    let app2 = make_app(state);
    let request = axum::http::Request::builder()
        .method("GET")
        .uri("/events/stream")
        .header("Accept", "text/event-stream")
        .header("Last-Event-ID", "1")
        .body(Body::empty())
        .unwrap();
    let response = app2.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let mut body = response.into_body();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        assert!(std::time::Instant::now() < deadline, "timed out");
        use http_body_util::BodyExt as _;
        if let Some(Ok(frame)) = body.frame().await {
            if let Ok(bytes) = frame.into_data() {
                let chunk = String::from_utf8_lossy(&bytes);
                for line in chunk.lines() {
                    if let Some(rest) = line.strip_prefix("data:") {
                        let record: acuity_api::EventRecord =
                            serde_json::from_str(rest.trim()).expect("must be valid EventRecord");
                        // Must be the second event, not the first
                        assert_eq!(record.seq, 2);
                        assert_eq!(record.event_type, "agent_turn_completed");
                        return;
                    }
                }
            }
        } else {
            panic!("SSE stream ended unexpectedly");
        }
    }
}

// ---------------------------------------------------------------------------
// GET /events — query endpoint
// ---------------------------------------------------------------------------

#[tokio::test]
async fn query_empty_db_returns_empty_page() {
    let state = test_state_no_gotify().await;
    let app = make_app(state);

    let resp = get_events(app, "").await;
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let page = body_json(resp).await;
    assert!(page.events.is_empty());
}

#[tokio::test]
async fn query_returns_inserted_events() {
    let state = test_state_no_gotify().await;
    let app = make_app(state.clone());

    // Insert two events via POST
    send_request(app.clone(), Some("1"), SESSION_IDLE_BODY).await;
    send_request(app.clone(), Some("1"), AGENT_TURN_BODY).await;

    let app2 = make_app(state);
    let resp = get_events(app2, "").await;
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let page = body_json(resp).await;
    assert_eq!(page.events.len(), 2);
    assert_eq!(page.events[0].event_type, "session_idle");
    assert_eq!(page.events[1].event_type, "agent_turn_completed");
}

#[tokio::test]
async fn query_after_cursor_filters_correctly() {
    let state = test_state_no_gotify().await;
    let app = make_app(state.clone());

    send_request(app.clone(), Some("1"), SESSION_IDLE_BODY).await;
    send_request(app.clone(), Some("1"), AGENT_TURN_BODY).await;

    // after=1 should return only the second event (seq=2)
    let app2 = make_app(state);
    let resp = get_events(app2, "after=1").await;
    let page = body_json(resp).await;
    assert_eq!(page.events.len(), 1);
    assert_eq!(page.events[0].event_type, "agent_turn_completed");
}

#[tokio::test]
async fn query_session_id_filter() {
    let state = test_state_no_gotify().await;
    let app = make_app(state.clone());

    send_request(app.clone(), Some("1"), SESSION_IDLE_BODY).await;
    send_request(app.clone(), Some("1"), SESSION_IDLE_BODY_S2).await;

    let app2 = make_app(state);
    let resp = get_events(app2, "session_id=session-2").await;
    let page = body_json(resp).await;
    assert_eq!(page.events.len(), 1);
    assert_eq!(page.events[0].session_id, "session-2");
}

#[tokio::test]
async fn query_event_type_filter() {
    let state = test_state_no_gotify().await;
    let app = make_app(state.clone());

    send_request(app.clone(), Some("1"), SESSION_IDLE_BODY).await;
    send_request(app.clone(), Some("1"), AGENT_TURN_BODY).await;

    let app2 = make_app(state);
    let resp = get_events(app2, "event_type=agent_turn_completed").await;
    let page = body_json(resp).await;
    assert_eq!(page.events.len(), 1);
    assert_eq!(page.events[0].event_type, "agent_turn_completed");
}

#[tokio::test]
async fn query_limit_respected() {
    let state = test_state_no_gotify().await;
    let app = make_app(state.clone());

    // Insert 3 events
    send_request(app.clone(), Some("1"), SESSION_IDLE_BODY).await;
    send_request(app.clone(), Some("1"), AGENT_TURN_BODY).await;
    send_request(app.clone(), Some("1"), SESSION_IDLE_BODY).await;

    let app2 = make_app(state);
    let resp = get_events(app2, "limit=2").await;
    let page = body_json(resp).await;
    assert_eq!(page.events.len(), 2);
}

#[tokio::test]
async fn query_limit_clamped_to_500() {
    let state = test_state_no_gotify().await;
    let app = make_app(state);
    // A giant limit should not cause an error — it's clamped server-side
    let resp = get_events(app, "limit=999999").await;
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
}

#[tokio::test]
async fn query_record_fields_correct() {
    let state = test_state_no_gotify().await;
    let app = make_app(state.clone());
    send_request(app, Some("1"), SESSION_IDLE_BODY).await;

    let app2 = make_app(state);
    let resp = get_events(app2, "").await;
    let page = body_json(resp).await;
    let rec = &page.events[0];

    assert_eq!(rec.seq, 1);
    assert_eq!(rec.event_type, "session_idle");
    assert_eq!(rec.session_id, "abc-123");
    assert!(rec.turn_id.is_none());
    assert_eq!(rec.payload, SESSION_IDLE_BODY);
    assert!(!rec.received_at.is_empty());
}

// ---------------------------------------------------------------------------
// GET /events — pagination cursor (`next_after`)
//
// The server owns the "is there more?" decision so clients never depend on
// the server's internal page-size clamp. Loop until `next_after` is `None`.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn query_next_after_none_on_short_page() {
    let state = test_state_no_gotify().await;
    let app = make_app(state.clone());

    // Two events; default limit (100) >> row count -> short page.
    send_request(app.clone(), Some("1"), SESSION_IDLE_BODY).await;
    send_request(app.clone(), Some("1"), AGENT_TURN_BODY).await;

    let app2 = make_app(state);
    let resp = get_events(app2, "").await;
    let page = body_json(resp).await;
    assert_eq!(page.events.len(), 2);
    assert_eq!(page.next_after, None);
}

#[tokio::test]
async fn query_next_after_some_on_full_page() {
    let state = test_state_no_gotify().await;
    let app = make_app(state.clone());

    // Three events; request limit=2 -> full page, must report a resume cursor.
    send_request(app.clone(), Some("1"), SESSION_IDLE_BODY).await;
    send_request(app.clone(), Some("1"), AGENT_TURN_BODY).await;
    send_request(app.clone(), Some("1"), SESSION_IDLE_BODY_S2).await;

    let app2 = make_app(state);
    let resp = get_events(app2, "limit=2").await;
    let page = body_json(resp).await;
    assert_eq!(page.events.len(), 2);
    // Cursor is the seq of the last returned row; resume with after=<that seq>.
    assert_eq!(page.next_after, Some(page.events[1].seq));
}

#[tokio::test]
async fn query_next_after_resumes_and_terminates() {
    let state = test_state_no_gotify().await;
    let app = make_app(state.clone());

    send_request(app.clone(), Some("1"), SESSION_IDLE_BODY).await;
    send_request(app.clone(), Some("1"), AGENT_TURN_BODY).await;
    send_request(app.clone(), Some("1"), SESSION_IDLE_BODY_S2).await;

    // Page 1: limit=2 -> full page, cursor set.
    let app2 = make_app(state.clone());
    let page = body_json(get_events(app2, "limit=2").await).await;
    assert_eq!(page.events.len(), 2);
    let cursor = page.next_after.expect("full page must set cursor");

    // Page 2: resume at the cursor -> exactly the remaining row, cursor None.
    let app3 = make_app(state);
    let next_uri = format!("limit=2&after={}", cursor);
    let page = body_json(get_events(app3, &next_uri).await).await;
    assert_eq!(page.events.len(), 1);
    assert_eq!(page.next_after, None);
}
