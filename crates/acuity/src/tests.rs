use axum::body::Body;
use tower::ServiceExt;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::{AppState, basename, make_app};

fn test_state_with_url(gotify_url: String) -> AppState {
    AppState {
        config: crate::config::Config {
            gotify_url,
            port: 0,
        },
        gotify_token: "test-token".into(),
        http: reqwest::Client::new(),
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

const VALID_BODY: &str = r#"{"session_id":"abc-123","project_dir":"/home/me/project","session_title":"hello"}"#;

#[tokio::test]
async fn header_missing_returns_400() {
    let app = make_app(test_state_with_url("http://localhost:80".into()));
    let status = send_request(app, None, "{}").await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn header_wrong_version_returns_400() {
    let app = make_app(test_state_with_url("http://localhost:80".into()));
    let status = send_request(app, Some("99"), "{}").await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn malformed_body_returns_422() {
    let app = make_app(test_state_with_url("http://localhost:80".into()));
    let status = send_request(app, Some("1"), "not-json").await;
    assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn valid_event_forwards_to_gotify() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/message"))
        .and(header("X-Gotify-Key", "test-token"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

    let app = make_app(test_state_with_url(mock_server.uri()));
    let status = send_request(app, Some("1"), VALID_BODY).await;
    assert_eq!(status, axum::http::StatusCode::OK);
}

#[tokio::test]
async fn gotify_error_returns_502() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/message"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&mock_server)
        .await;

    let app = make_app(test_state_with_url(mock_server.uri()));
    let status = send_request(app, Some("1"), VALID_BODY).await;
    assert_eq!(status, axum::http::StatusCode::BAD_GATEWAY);
}

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
