use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
pub enum Kind {
    ArtifactCreated,
    GitCommitAttempt,
    SessionAssociated,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Event {
    pub id: Uuid,
    pub timestamp: u64,
    pub kind: Kind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    pub details: Map<String, Value>,
}

pub fn context_dir(
    cwd: &Path,
    context: Option<&str>,
    store: Option<&Path>,
) -> Result<Option<PathBuf>> {
    let Some(resolved) = cuelib::head::resolve_active_context(cwd, context)? else {
        return Ok(None);
    };
    let dir = resolved.context_dir(cwd, store)?;
    if !dir.join("context.md").is_file() {
        bail!("Context does not exist: {}", resolved.address(cwd)?);
    }
    Ok(Some(dir))
}

pub fn record(
    context_dir: &Path,
    kind: Kind,
    details: Map<String, Value>,
    session_id: Option<String>,
    harness: Option<String>,
    host: Option<String>,
) -> Result<PathBuf> {
    let event = Event {
        id: Uuid::new_v4(),
        timestamp: u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?,
        kind,
        details,
        session_id,
        harness,
        host: host
            .or_else(|| std::env::var("CAST_HOST_NAME").ok())
            .filter(|h| !h.is_empty()),
    };
    publish(context_dir, &event)
}

fn publish(context_dir: &Path, event: &Event) -> Result<PathBuf> {
    let dir = context_dir.join("activity");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{:020}-{}.json", event.timestamp, event.id));
    let mut staged = tempfile::Builder::new()
        .prefix(".activity-staging-")
        .tempfile_in(&dir)?;
    serde_json::to_writer(&mut staged, event)?;
    staged.write_all(b"\n")?;
    staged.flush()?;
    staged
        .persist_noclobber(&path)
        .with_context(|| format!("Failed to publish {}", path.display()))?;
    Ok(path)
}

pub fn list(context_dir: &Path) -> Result<Vec<Event>> {
    let dir = context_dir.join("activity");
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => return Err(error.into()),
    };
    let mut events = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.path().extension().is_some_and(|ext| ext == "json") {
            events.push(serde_json::from_slice::<Event>(&fs::read(entry.path())?)?);
        }
    }
    events.sort_by_key(|e| (e.timestamp, e.id));
    Ok(events)
}
