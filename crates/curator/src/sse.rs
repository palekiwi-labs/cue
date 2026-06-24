use std::sync::mpsc::SyncSender;

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
                        Err(e) => {
                            eprintln!("sse: malformed event JSON, skipping: {e}");
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
            Err(e) => {
                eprintln!("sse: failed to build tokio runtime: {e}");
                let _ = tx.send(Msg::SseStatus(SseStatus::Reconnecting { attempt: 1 }));
                return;
            }
        };
        rt.block_on(run_loop(url, tx));
    });
}

async fn run_loop(url: String, tx: SyncSender<Msg>) {
    let mut cursor: i64 = 0;
    let mut backoff_ms: u64 = 500;
    let mut attempt: u32 = 0;

    loop {
        attempt += 1;
        let _ = tx.send(Msg::SseStatus(SseStatus::Reconnecting { attempt }));

        match connect_and_stream(&url, cursor, &tx).await {
            Ok(last_cursor) => {
                cursor = last_cursor;
                backoff_ms = 500;
                attempt = 0;
                // Connected status is sent inside connect_and_stream as soon
                // as the 200 response arrives, before consuming the stream.
            }
            Err(e) => {
                eprintln!("sse: stream error (attempt {attempt}): {e}");
                tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                backoff_ms = next_backoff(backoff_ms);
            }
        }
    }
}

async fn connect_and_stream(
    url: &str,
    cursor: i64,
    tx: &SyncSender<Msg>,
) -> Result<i64> {
    let client = reqwest::Client::new();
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
            if tx.send(Msg::Sse(record)).is_err() {
                // Receiver dropped — main thread exited cleanly.
                return Ok(lb.cursor());
            }
        }
    }

    // Stream exhausted (server closed connection); caller will reconnect.
    Ok(lb.cursor())
}
