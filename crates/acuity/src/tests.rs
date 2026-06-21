use axum::body::Body;
use tower::ServiceExt;

use crate::{AppState, make_app};

fn test_state() -> AppState {
    AppState {
        config: crate::config::Config {
            gotify_url: "http://localhost:80".into(),
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

#[tokio::test]
async fn header_missing_returns_400() {
    let app = make_app(test_state());
    let status = send_request(app, None, "{}").await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn header_wrong_version_returns_400() {
    let app = make_app(test_state());
    let status = send_request(app, Some("99"), "{}").await;
    assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn malformed_body_returns_422() {
    let app = make_app(test_state());
    let status = send_request(app, Some("1"), "not-json").await;
    assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
}
