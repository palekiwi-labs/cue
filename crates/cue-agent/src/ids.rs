//! Run identity.
//!
//! cue-agent assigns identity rather than adopting one the harness generates:
//! pi accepts `--session-id`, so the run directory, the trace filename and the
//! harness's own record all carry the same id with no mapping table. Ids must
//! therefore satisfy pi's validation (non-empty, alphanumeric start and end,
//! only `-`, `_` and `.` inside); cue-agent additionally emits no dots and no
//! slashes so an id is always one safe path segment.

use std::time::{SystemTime, UNIX_EPOCH};

/// Validate an id the way pi does (`assertValidSessionId`,
/// packages/coding-agent/src/core/session-manager.ts:212-218).
pub fn is_valid(id: &str) -> bool {
    let bytes = id.as_bytes();
    let alnum = |b: u8| b.is_ascii_alphanumeric();
    let inner = |b: u8| alnum(b) || b == b'-' || b == b'_' || b == b'.';
    match bytes {
        [] => false,
        [only] => alnum(*only),
        [first, middle @ .., last] => {
            alnum(*first) && alnum(*last) && middle.iter().copied().all(inner)
        }
    }
}

/// `<yyyymmdd>-<hhmmss>-<entropy>`, e.g. `20260918-120301-3f9a2b`.
pub fn batch_id(now: SystemTime) -> String {
    let secs = now
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let (y, m, d, hh, mm, ss) = crate::state::civil_from_unix(secs as i64);
    format!(
        "{y:04}{m:02}{d:02}-{hh:02}{mm:02}{ss:02}-{:06x}",
        entropy() & 0xff_ffff
    )
}

/// `<batch-id>-<agent>-<n>`: unique by construction within a batch and stable
/// for a given batch, which is what makes retroactive trace materialisation
/// idempotent.
pub fn run_id(batch_id: &str, agent: &str, index: usize) -> String {
    format!("{batch_id}-{}-{index}", sanitize(agent))
}

/// The short form used in the trace filename, derived deterministically from
/// the run id so the same run always names the same trace.
pub fn short(run_id: &str) -> String {
    format!("{:08x}", fnv1a64(run_id.as_bytes()) as u32)
}

/// Reduce an arbitrary name to an id-safe segment.
pub fn sanitize(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "agent".to_string()
    } else {
        trimmed
    }
}

/// Entropy without a dependency: the clock at nanosecond resolution mixed with
/// the process id. Ids only need to be unique among concurrent batches on one
/// machine, not unguessable.
fn entropy() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 ^ d.as_secs())
        .unwrap_or_default();
    let pid = std::process::id() as u64;
    fnv1a64(&(nanos ^ (pid << 32)).to_le_bytes())
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_satisfy_the_harness_validation() {
        let batch = batch_id(SystemTime::now());
        assert!(is_valid(&batch), "{batch}");
        for agent in ["explore", "consultant-opus", "weird name!", "-lead-"] {
            let run = run_id(&batch, agent, 1);
            assert!(is_valid(&run), "{run}");
            assert!(!run.contains('/') && !run.contains('.'), "{run}");
        }
    }

    #[test]
    fn validation_rejects_what_the_harness_rejects() {
        for id in ["", "-lead", "trail-", "has/slash", "has space"] {
            assert!(!is_valid(id), "accepted {id:?}");
        }
        assert!(is_valid("a"));
        assert!(is_valid("a.b_c-d1"));
    }

    #[test]
    fn the_short_form_is_derived_from_the_run_id() {
        let short = short("20260918-120301-3f9a2b-explore-1");
        assert_eq!(short.len(), 8);
        assert_eq!(short, super::short("20260918-120301-3f9a2b-explore-1"));
        assert_ne!(short, super::short("20260918-120301-3f9a2b-explore-2"));
    }
}
