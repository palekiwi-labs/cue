//! Plane 2: machine-local operational exhaust under `$XDG_STATE_HOME/cue`.
//!
//! Every run writes here whether or not a cue context was supplied, so the
//! trace artifact in the caller's context is curation rather than preservation:
//! a missing trace is never data loss.

use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// Restrict only the requested cue-owned directory, not its existing parents.
pub fn private_dir(path: &Path) -> std::io::Result<()> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(std::io::Error::other(
            "state directory must not be a symlink",
        ));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

pub fn private_file(path: &Path) -> std::io::Result<fs::File> {
    let file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.set_len(0)?;
    Ok(file)
}

pub fn write_private(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> std::io::Result<()> {
    private_file(path.as_ref())?.write_all(bytes.as_ref())
}

/// `$XDG_STATE_HOME/cue/agent`, the root of the run record.
pub fn agent_root() -> Result<PathBuf> {
    let state =
        if let Some(value) = std::env::var_os("XDG_STATE_HOME").filter(|value| !value.is_empty()) {
            PathBuf::from(value)
        } else {
            dirs::home_dir()
                .context("Could not determine the home directory for the cue state directory")?
                .join(".local")
                .join("state")
        };
    Ok(state.join("cue").join("agent"))
}

/// `runs/<YYYY-MM-DD>/<batch-id>` under the agent state root.
pub fn batch_dir(agent_root: &Path, now_secs: i64, batch_id: &str) -> PathBuf {
    let (y, m, d, _, _, _) = civil_from_unix(now_secs);
    agent_root
        .join("runs")
        .join(format!("{y:04}-{m:02}-{d:02}"))
        .join(batch_id)
}

/// Append one line to `$XDG_STATE_HOME/cue/agent/index.jsonl`.
///
/// One pass over one file answers "what has run", and the file is rebuildable
/// from the per-run manifests if it is ever lost.
pub fn append_index(agent_root: &Path, entry: &serde_json::Value) -> Result<()> {
    private_dir(agent_root)?;
    let path = agent_root.join("index.jsonl");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)
        .with_context(|| format!("Could not open {}", path.display()))?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    let mut line = serde_json::to_string(entry)?;
    line.push('\n');
    file.write_all(line.as_bytes())
        .with_context(|| format!("Could not append to {}", path.display()))?;
    Ok(())
}

/// Civil date and time from a Unix timestamp, in UTC.
///
/// Howard Hinnant's `civil_from_days`, inlined rather than taken as a
/// dependency: a run directory name is the only date this binary formats.
pub fn civil_from_unix(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    (y, m as u32, d as u32, hh as u32, mm as u32, ss as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch_is_the_first_of_january_1970() {
        assert_eq!(civil_from_unix(0), (1970, 1, 1, 0, 0, 0));
    }

    #[test]
    fn a_known_timestamp_round_trips() {
        // 2026-09-18T12:03:01Z
        assert_eq!(civil_from_unix(1_789_732_981), (2026, 9, 18, 12, 3, 1));
    }

    #[test]
    fn a_leap_day_is_not_skipped() {
        // 2024-02-29T00:00:00Z
        assert_eq!(civil_from_unix(1_709_164_800), (2024, 2, 29, 0, 0, 0));
    }
}
