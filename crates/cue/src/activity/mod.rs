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
        session_id: session_id.filter(|s| !s.is_empty()),
        harness: harness.filter(|s| !s.is_empty()),
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

const MAX_TIMESTAMP: u64 = (1 << 53) - 1;

fn filename_identity(name: &str) -> Option<(u64, Uuid)> {
    let stem = name.strip_suffix(".json")?;
    if stem.len() != 57 || !stem.is_ascii() || stem.as_bytes()[20] != b'-' {
        return None;
    }
    if !stem[..20].bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let timestamp = stem[..20].parse().ok()?;
    let id = Uuid::parse_str(&stem[21..]).ok()?;
    (id.to_string() == stem[21..]).then_some((timestamp, id))
}

pub fn list(context_dir: &Path, since: Option<u64>, limit: Option<usize>) -> Result<Vec<Event>> {
    let dir = context_dir.join("activity");
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => return Err(error.into()),
    };
    let mut events = Vec::new();
    for entry in entries {
        let entry = entry?;
        let Some((timestamp, id)) = filename_identity(&entry.file_name().to_string_lossy()) else {
            continue;
        };
        if !entry.file_type()?.is_file() {
            continue;
        }
        let bytes = fs::read(entry.path())?;
        let event = match serde_json::from_slice::<Event>(&bytes) {
            Ok(event)
                if event.timestamp == timestamp && event.id == id && timestamp <= MAX_TIMESTAMP =>
            {
                event
            }
            _ => {
                eprintln!(
                    "Warning: skipping malformed activity record {}",
                    entry.path().display()
                );
                continue;
            }
        };
        if since.is_none_or(|since| timestamp >= since) {
            events.push(event);
        }
    }
    events.sort_by_key(|e| (e.timestamp, e.id));
    if let Some(limit) = limit {
        events.drain(..events.len().saturating_sub(limit));
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn publication_never_replaces_an_existing_event_and_cleans_up_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let mut event = Event {
            id: Uuid::new_v4(),
            timestamp: 1,
            kind: Kind::SessionAssociated,
            host: None,
            harness: None,
            session_id: None,
            details: Map::new(),
        };
        let path = publish(dir.path(), &event).unwrap();
        let original = fs::read(&path).unwrap();
        event.details.insert("changed".into(), true.into());
        assert!(publish(dir.path(), &event).is_err());
        assert_eq!(fs::read(path).unwrap(), original);
        assert_eq!(
            fs::read_dir(dir.path().join("activity")).unwrap().count(),
            1
        );
    }

    #[test]
    fn concurrent_writers_publish_only_complete_unique_events() {
        let dir = tempfile::tempdir().unwrap();
        let done = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    for _ in 0..25 {
                        record(
                            dir.path(),
                            Kind::GitCommitAttempt,
                            Map::new(),
                            None,
                            None,
                            None,
                        )
                        .unwrap();
                    }
                    done.fetch_add(1, Ordering::Release);
                });
            }
            while done.load(Ordering::Acquire) < 8 {
                let entries = list(dir.path(), None, None).unwrap();
                assert!(entries.iter().all(|e| e.id.get_version_num() == 4));
                // Parse every final file independently: list's warning-and-skip
                // policy must not hide a transient partially published payload.
                if let Ok(paths) = fs::read_dir(dir.path().join("activity")) {
                    for path in paths {
                        let path = path.unwrap().path();
                        if filename_identity(path.file_name().unwrap().to_str().unwrap()).is_some()
                        {
                            serde_json::from_slice::<Event>(&fs::read(path).unwrap()).unwrap();
                        }
                    }
                }
                std::thread::yield_now();
            }
        });
        let entries = list(dir.path(), None, None).unwrap();
        assert_eq!(entries.len(), 200);
        assert_eq!(
            entries.iter().map(|e| e.id).collect::<HashSet<_>>().len(),
            200
        );
        assert_eq!(
            fs::read_dir(dir.path().join("activity")).unwrap().count(),
            200
        );
    }
}
