// acuity: observability server for the cue ecosystem.
// Phase 1 stateless MVP: receives session.idle events, forwards to Gotify.

mod config;

use std::path::Path;
use std::sync::Arc;

use acuity_schema::{SCHEMA_VERSION, SessionIdle};
use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use serde_json::json;
use tracing::{error, info};

#[derive(Clone)]
struct AppState {
    config: config::Config,
    gotify_token: String,
    http: reqwest::Client,
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

    let gotify_token = std::env::var("ACUITY_GOTIFY_TOKEN").unwrap_or_else(|_| {
        eprintln!(
            "error: ACUITY_GOTIFY_TOKEN environment variable is required but not set"
        );
        std::process::exit(1);
    });

    let state = Arc::new(AppState {
        config: cfg.clone(),
        gotify_token,
        http: reqwest::Client::new(),
    });

    let app = Router::new()
        .route("/events", post(handle_event))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", cfg.port);
    info!("acuity listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn handle_event(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> StatusCode {
    // 1. Validate schema version header
    let schema_header = headers
        .get("x-acuity-schema")
        .and_then(|v| v.to_str().ok());

    let expected = SCHEMA_VERSION.to_string();
    match schema_header {
        Some(v) if v == expected => {}
        Some(v) => {
            error!(
                "rejected event: X-Acuity-Schema {} != expected {}",
                v, expected
            );
            return StatusCode::BAD_REQUEST;
        }
        None => {
            error!("rejected event: missing X-Acuity-Schema header");
            return StatusCode::BAD_REQUEST;
        }
    }

    // 2. Deserialize body as SessionIdle
    let event: SessionIdle = match serde_json::from_slice(&body) {
        Ok(e) => e,
        Err(err) => {
            error!("rejected event: malformed body: {}", err);
            return StatusCode::UNPROCESSABLE_ENTITY;
        }
    };

    // 3. Compose Gotify payload
    let basename = Path::new(&event.project_dir)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&event.project_dir);

    let message = format!(
        "Idle: {}",
        event
            .session_title
            .as_deref()
            .unwrap_or(&event.session_id)
    );

    let payload = json!({
        "title": basename,
        "message": message,
        "priority": 5,
    });

    // 4. Forward to Gotify
    let url = format!(
        "http://{}/message?token={}",
        state.config.gotify_host, state.gotify_token
    );

    match state.http.post(&url).json(&payload).send().await {
        Ok(resp) if resp.status().is_success() => {
            info!(
                "forwarded session.idle for session={} project={}",
                event.session_id, event.project_dir
            );
            StatusCode::OK
        }
        Ok(resp) => {
            error!(
                "gotify returned unexpected status: {}",
                resp.status()
            );
            StatusCode::BAD_GATEWAY
        }
        Err(err) => {
            error!("failed to reach gotify: {}", err);
            StatusCode::BAD_GATEWAY
        }
    }
}
