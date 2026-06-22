use axum::body::Body;
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
    let status =
        send_request(app, Some("1"), r#"{"type":"nope","session_id":"x"}"#)
            .await;
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

    let event_type: String =
        sqlx::query_scalar("SELECT event_type FROM events LIMIT 1")
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

    // The Gotify call is fire-and-forget (tokio::spawn). Give it a brief
    // window to complete before wiremock verifies on drop.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    // wiremock verifies the expectation (1 call) on drop
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
