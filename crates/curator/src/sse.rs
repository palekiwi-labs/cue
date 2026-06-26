use std::sync::mpsc::{SyncSender, TrySendError};

use anyhow::{Result, anyhow};
use futures_util::StreamExt;

use acuity_api::EventRecord;

use crate::msg::{Msg, SseStatus};

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/// Exponential backoff step capped at 5 000 ms.
///
/// Extracted as a pure function so Slice 8 can unit-test the cap without
/// any async machinery.
pub(crate) fn next_backoff(current_ms: u64) -> u64 {
    (current_ms * 2).min(5_000)
}

// ---------------------------------------------------------------------------
// LineBuffer — synchronous SSE parser
// ---------------------------------------------------------------------------

/// Incremental SSE line buffer.
///
/// All parse logic lives here so it can be unit-tested without a server.
/// The async `connect_and_stream` function only needs to call
/// `lb.feed(&chunk)` for each incoming byte chunk.
///
/// # Correctness notes
///
/// - Lines may be split across chunk boundaries — the `buf` field retains
///   the partial tail between `feed()` calls.
/// - Lines starting with `:` are keep-alive comments and are silently skipped.
///   Skipping them prevents reconnect storms when acuity emits a `:keep-alive`
///   comment every ~15 s.
/// - Exactly one leading space is stripped from `data:` and `id:` field values
///   per the SSE spec (e.g. `data: {…}` → `{…}`).
/// - A blank line (the event terminator) dispatches the accumulated event only
///   if `pending_data` is non-empty.
/// - Malformed JSON in `pending_data` is logged and skipped; the parser does
///   not reconnect on decode errors.
pub(crate) struct LineBuffer {
    /// Bytes received but not yet formed into a complete line.
    buf: Vec<u8>,
    /// The `id:` field of the event being accumulated.
    pending_id: Option<i64>,
    /// The `data:` field of the event being accumulated.
    pending_data: String,
    /// Seq of the last successfully parsed event. Initialised to the cursor
    /// supplied at construction (i.e. `Last-Event-ID` on the connection).
    cursor: i64,
}

impl LineBuffer {
    pub(crate) fn new(initial_cursor: i64) -> Self {
        Self {
            buf: Vec::new(),
            pending_id: None,
            pending_data: String::new(),
            cursor: initial_cursor,
        }
    }

    /// Feed one raw byte chunk from the network.
    ///
    /// Returns every `EventRecord` completed by this chunk. May return zero,
    /// one, or many records depending on how many blank-line terminators the
    /// chunk contains.
    pub(crate) fn feed(&mut self, chunk: &[u8]) -> Vec<EventRecord> {
        self.buf.extend_from_slice(chunk);
        let mut records = Vec::new();

        while let Some(nl) = self.buf.iter().position(|&b| b == b'\n') {

            // Extract the line bytes (excluding the \n).
            let line_bytes = self.buf[..nl].to_vec();
            self.buf.drain(..=nl);

            // Strip optional trailing \r (CRLF line endings).
            let line = match line_bytes.last() {
                Some(&b'\r') => &line_bytes[..line_bytes.len() - 1],
                _ => &line_bytes[..],
            };

            let line = match std::str::from_utf8(line) {
                Ok(s) => s,
                Err(_) => continue, // skip non-UTF-8 lines silently
            };

            // Keep-alive comment — skip without touching accumulator state.
            if line.starts_with(':') {
                continue;
            }

            // Blank line = event terminator.
            if line.is_empty() {
                if !self.pending_data.is_empty() {
                    match serde_json::from_str::<EventRecord>(&self.pending_data) {
                        Ok(record) => {
                            if let Some(id) = self.pending_id {
                                self.cursor = id;
                            }
                            records.push(record);
                        }
                        Err(_) => {
                            // Silently skip — zero records returned to caller
                            // signals the drop; eprintln! would corrupt TUI.
                        }
                    }
                }
                self.pending_id = None;
                self.pending_data.clear();
                continue;
            }

            // Field dispatch.
            if let Some(rest) = line.strip_prefix("id:") {
                let val = rest.strip_prefix(' ').unwrap_or(rest);
                if let Ok(id) = val.parse::<i64>() {
                    self.pending_id = Some(id);
                }
            } else if let Some(rest) = line.strip_prefix("data:") {
                let val = rest.strip_prefix(' ').unwrap_or(rest);
                if !self.pending_data.is_empty() {
                    // SSE spec: multiple data: lines in one event are joined
                    // with U+000A (LF). serde_json is tolerant of interior
                    // whitespace so multi-line JSON objects parse correctly.
                    self.pending_data.push('\n');
                }
                self.pending_data.push_str(val);
            }
            // Other field names (event:, retry:) are intentionally ignored.
        }

        records
    }

    /// The seq of the last successfully parsed event (or the initial cursor
    /// if no events have been parsed yet).
    pub(crate) fn cursor(&self) -> i64 {
        self.cursor
    }
}

// ---------------------------------------------------------------------------
// SSE thread
// ---------------------------------------------------------------------------

/// Spawn a background thread that connects to `{url}/events/stream` and posts
/// [`Msg::Sse`] and [`Msg::SseStatus`] to `tx`.
///
/// The thread owns a single-threaded tokio runtime. On runtime-build failure
/// a [`SseStatus::Reconnecting`] is sent and the thread exits; the main loop
/// will surface the degraded state in the UI rather than silently freezing.
pub fn spawn(url: String, tx: SyncSender<Msg>) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(_) => {
                // SseStatus::Reconnecting surfaces the failure to the UI;
                // eprintln! would corrupt the TUI display.
                let _ = tx.send(Msg::SseStatus(SseStatus::Reconnecting { attempt: 1 }));
                return;
            }
        };
        rt.block_on(run_loop(url, tx));
    });
}

async fn run_loop(url: String, tx: SyncSender<Msg>) {
    // Build the client once; it is Arc'd internally and designed to be
    // reused across reconnects (shares the TLS stack and connection pool).
    let client = reqwest::Client::new();
    let mut cursor: i64 = 0;
    let mut backoff_ms: u64 = 500;
    let mut attempt: u32 = 0;

    loop {
        attempt += 1;
        let _ = tx.send(Msg::SseStatus(SseStatus::Reconnecting { attempt }));

        match connect_and_stream(&client, &url, cursor, &tx).await {
            Ok(last_cursor) => {
                cursor = last_cursor;
                backoff_ms = 500;
                attempt = 0;
                // Connected status is sent inside connect_and_stream as soon
                // as the 200 response arrives, before consuming the stream.
            }
            Err(_) => {
                // SseStatus::Reconnecting (sent above) already surfaces the
                // degraded state to the UI; eprintln! would corrupt the TUI.
                tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                backoff_ms = next_backoff(backoff_ms);
            }
        }
    }
}

async fn connect_and_stream(
    client: &reqwest::Client,
    url: &str,
    cursor: i64,
    tx: &SyncSender<Msg>,
) -> Result<i64> {
    let response = client
        .get(format!("{url}/events/stream"))
        .header("Last-Event-ID", cursor.to_string())
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow!("sse: server returned {}", response.status()));
    }

    // Signal connected immediately after a successful HTTP response — before
    // consuming the stream so the UI shows "connected" during active streaming,
    // not just after the stream ends.
    let _ = tx.send(Msg::SseStatus(SseStatus::Connected));

    let mut stream = response.bytes_stream();
    let mut lb = LineBuffer::new(cursor);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        for record in lb.feed(&chunk) {
            match tx.try_send(Msg::Sse(record)) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    // Channel is saturated (reconnect burst or live overload).
                    // Drop this event — telemetry is inherently lossy and the
                    // ring buffer (EVENT_CAP) already bounds what the UI shows.
                    // Using try_send ensures the input thread (and Quit) are
                    // never blocked by SSE bursts.
                }
                Err(TrySendError::Disconnected(_)) => {
                    // Receiver dropped — main thread exited cleanly.
                    return Ok(lb.cursor());
                }
            }
        }
    }

    // Stream exhausted (server closed connection); caller will reconnect.
    Ok(lb.cursor())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Helpers ---

    /// Build a minimal valid JSON string for an `EventRecord`.
    fn record_json(seq: i64, session_id: &str) -> String {
        format!(
            r#"{{"seq":{seq},"received_at":"2026-01-01T00:00:{seq:02}Z","event_type":"session_idle","session_id":"{session_id}","turn_id":null,"payload":"{{}}"}}"#
        )
    }

    /// Build a complete SSE event frame (two trailing newlines = terminator).
    fn event_frame(seq: i64, session_id: &str) -> String {
        format!("id: {seq}\ndata: {}\n\n", record_json(seq, session_id))
    }

    // --- next_backoff ---

    #[test]
    fn next_backoff_doubles_then_caps_at_5000() {
        assert_eq!(next_backoff(500), 1000);
        assert_eq!(next_backoff(1000), 2000);
        assert_eq!(next_backoff(2000), 4000);
        assert_eq!(next_backoff(4000), 5000); // cap
        assert_eq!(next_backoff(5000), 5000); // stays at cap
    }

    // --- LineBuffer: single complete event ---

    #[test]
    fn single_complete_event_returned() {
        let mut lb = LineBuffer::new(0);
        let records = lb.feed(event_frame(1, "s1").as_bytes());
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].seq, 1);
        assert_eq!(records[0].session_id, "s1");
    }

    // --- LineBuffer: chunk boundary ---

    #[test]
    fn chunk_boundary_split_completes_on_second_chunk() {
        let mut lb = LineBuffer::new(0);
        let frame = event_frame(1, "s1");
        let bytes = frame.as_bytes();
        let mid = bytes.len() / 2;
        let r1 = lb.feed(&bytes[..mid]);
        assert!(r1.is_empty(), "no record before second chunk");
        let r2 = lb.feed(&bytes[mid..]);
        assert_eq!(r2.len(), 1);
        assert_eq!(r2[0].seq, 1);
    }

    // --- LineBuffer: keep-alive skip ---

    #[test]
    fn keep_alive_comments_are_skipped() {
        let mut lb = LineBuffer::new(0);
        let input = format!(
            "{}:keep-alive\n{}",
            event_frame(1, "s1"),
            event_frame(2, "s1")
        );
        let records = lb.feed(input.as_bytes());
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].seq, 1);
        assert_eq!(records[1].seq, 2);
    }

    // --- LineBuffer: leading-space strip ---

    #[test]
    fn data_field_with_leading_space_parses_correctly() {
        let mut lb = LineBuffer::new(0);
        // Standard SSE: one space after the colon.
        let frame = format!("id: 1\ndata: {}\n\n", record_json(1, "s1"));
        let records = lb.feed(frame.as_bytes());
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].seq, 1);
    }

    #[test]
    fn data_field_without_leading_space_parses_correctly() {
        let mut lb = LineBuffer::new(0);
        // No space after the colon — still valid per our strip logic.
        let frame = format!("id: 1\ndata:{}\n\n", record_json(1, "s1"));
        let records = lb.feed(frame.as_bytes());
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].seq, 1);
    }

    // --- LineBuffer: malformed JSON ---

    #[test]
    fn malformed_json_produces_no_records_and_no_panic() {
        let mut lb = LineBuffer::new(0);
        let records = lb.feed(b"id: 1\ndata: not valid json at all\n\n");
        assert!(records.is_empty());
    }

    // --- LineBuffer: multiple events in one chunk ---

    #[test]
    fn multiple_events_in_single_chunk_returned_in_order() {
        let mut lb = LineBuffer::new(0);
        let combined: String = (1..=3)
            .map(|i| event_frame(i, "s1"))
            .collect();
        let records = lb.feed(combined.as_bytes());
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].seq, 1);
        assert_eq!(records[1].seq, 2);
        assert_eq!(records[2].seq, 3);
    }

    // --- LineBuffer: blank-line no-op ---

    #[test]
    fn blank_line_with_no_pending_data_emits_nothing() {
        let mut lb = LineBuffer::new(0);
        let records = lb.feed(b"\n\n\n");
        assert!(records.is_empty());
    }

    // --- LineBuffer: cursor tracking ---

    #[test]
    fn cursor_is_initial_value_before_any_events() {
        let lb = LineBuffer::new(42);
        assert_eq!(lb.cursor(), 42);
    }

    #[test]
    fn cursor_tracks_seq_of_last_parsed_event() {
        let mut lb = LineBuffer::new(0);
        for i in 1..=5_i64 {
            lb.feed(event_frame(i, "s1").as_bytes());
        }
        assert_eq!(lb.cursor(), 5);
    }

    #[test]
    fn cursor_not_updated_after_malformed_json() {
        let mut lb = LineBuffer::new(0);
        lb.feed(event_frame(1, "s1").as_bytes());
        lb.feed(b"id: 99\ndata: {bad json}\n\n");
        // Cursor should still reflect the last *successfully* parsed event.
        assert_eq!(lb.cursor(), 1);
    }

    // --- LineBuffer: multi-line data: join per SSE spec ---

    #[test]
    fn data_multiline_lines_joined_with_newline() {
        // An event whose data: is split across two lines at a JSON key boundary.
        // The LF between them must not corrupt parsing.
        let mut lb = LineBuffer::new(0);
        let line1 = r#"{"seq":7,"received_at":"2026-01-01T00:00:07Z","event_type":"session_idle","session_id":"s1","turn_id":null,"#;
        let line2 = r#""payload":"{}"}"#;
        let frame = format!("id: 7\ndata: {line1}\ndata: {line2}\n\n");
        let records = lb.feed(frame.as_bytes());
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].seq, 7);
        assert_eq!(lb.cursor(), 7);
    }
}
