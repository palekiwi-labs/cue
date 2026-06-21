// acuity: observability server for the cue ecosystem.
// Phase 1 stateless MVP: receives session.idle events, forwards to Gotify.

mod config;
#[cfg(test)]
mod tests;

use std::path::Path;

use acuity_schema::{SCHEMA_VERSION, SessionIdle};
use axum::{
    Router,
    extract::{DefaultBodyLimit, State},
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

fn make_app(state: AppState) -> Router {
    Router::new()
        .route("/events", post(handle_event))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .with_state(state)
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

    // ACUITY_GOTIFY_TOKEN is intentionally read here, not via figment Config.
    // See config.rs for the rationale (kept out of Config to avoid a silent
    // duplicate read from the ACUITY_ env layer).
    let gotify_token = std::env::var("ACUITY_GOTIFY_TOKEN").map_err(|_| {
        anyhow::anyhow!(
            "ACUITY_GOTIFY_TOKEN environment variable is required but not set"
        )
    })?;

    let port = cfg.port;
    let state = AppState {
        config: cfg,
        gotify_token,
        http: reqwest::Client::new(),
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
                "rejected event: missing or non-numeric X-Acuity-Schema header (expected {})",
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

    // 2. Deserialize body as SessionIdle
    let event: SessionIdle = match serde_json::from_slice(&body) {
        Ok(e) => e,
        Err(err) => {
            error!("rejected event: malformed body: {}", err);
            return StatusCode::UNPROCESSABLE_ENTITY;
        }
    };

    // 3. Compose Gotify payload
    let title = basename(&event.project_dir);

    let message = format!(
        "Idle: {}",
        event
            .session_title
            .as_deref()
            .unwrap_or(&event.session_id)
    );

    let payload = json!({
        "title": title,
        "message": message,
        "priority": 5,
    });

    // 4. Forward to Gotify (token as X-Gotify-Key header, never in URL)
    let url = format!("{}/message", state.config.gotify_url);

    match state
        .http
        .post(&url)
        .header("X-Gotify-Key", &state.gotify_token)
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            info!(
                session_id = %event.session_id,
                project_dir = %event.project_dir,
                "forwarded session.idle"
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
