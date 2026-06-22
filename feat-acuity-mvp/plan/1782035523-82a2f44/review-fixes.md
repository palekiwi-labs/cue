---
status: complete
---

## Foreword

This plan addresses the blockers, concerns, and missing tests identified in the
code review of `feat/acuity-mvp` (trace: `trace/1782035523-82a2f44/review-acuity-mvp.md`).

Revised after consulting claude-opus on sequencing and technical details
(see log entry: "Opus consultation on review-fixes plan").

Key changes to the original plan from the consultation:

- Resequenced: deterministic handler tests come FIRST (Part A) so the schema
  refactor (formerly A5) is guarded by tests when it lands
- Added wiremock happy-path test asserting X-Gotify-Key header so A2/A4 are
  not shipped blind
- A4 default corrected: `"http://localhost"` not `"http://localhost:80"`;
  trailing-slash normalization added to `Config::load()`
- A5 must also delete the dead `expected: String` binding and rewrite the
  log branches (would cause warning breaking A13)
- A7 must explicitly remove `use std::sync::Arc` import (same issue)
- A12 (narrow tokio features) dropped: out of scope for a security-fix PR,
  cargo feature unification means the saving is marginal and it breaks silently
  on future additions
- A6/A7/A9 collapsed into one coherent main() rewrite step
- B1 dev-deps: serde_json removed (already in dependencies)

All changes are in `crates/acuity/`.
Work order: deterministic tests first, then code fixes, then happy-path test.

---

## Steps

### Part A -- Deterministic handler tests (test-first, before the fixes)

- [x] **A1** Add `[dev-dependencies]` to `crates/acuity/Cargo.toml`:

  ```toml
  tower          = { version = "0.5", features = ["util"] }
  http-body-util = "0.1"
  wiremock       = "0.6"
  ```

  (`axum`, `tokio`, `serde_json` are already in `[dependencies]`, available to tests.)

- [x] **A2** Create `crates/acuity/src/tests.rs` with a `#[cfg(test)]` module.
      Add a helper that constructs the router via `make_app(state: AppState) -> Router`,
      extracted from `main()` so tests can call it directly.
      Refactor `main.rs` to call `make_app(state)` instead of building the router inline.
      Use `tower::ServiceExt::oneshot` to drive requests in tests.

- [x] **A3** Add `mod tests;` to `main.rs`.

- [x] **A4** Add test: `header_missing_returns_400` -- POST `/events` with no
      `X-Acuity-Schema` header; assert `400`.

- [x] **A5** Add test: `header_wrong_version_returns_400` -- POST `/events` with
      `X-Acuity-Schema: 99`; assert `400`.

- [x] **A6** Add test: `malformed_body_returns_422` -- POST `/events` with correct
      schema header but body `"not-json"`; assert `422`.

- [x] **A7** Run `cargo test -p acuity` -- all three tests green (they already pass
      against the current implementation).

- [x] **A8** Commit: `test: add deterministic handler tests for acuity`

---

### Part B -- Code fixes

- [x] **B1 (review-B1)** Add `DefaultBodyLimit::max(16 * 1024)` layer in `make_app()`.
      Add `use axum::extract::DefaultBodyLimit;` import. The default axum limit is 2 MB;
      this tightens it for a payload that is always < 1 KB.

- [x] **B2 (review-B2)** Move Gotify token to `X-Gotify-Key` header:

  ```rust
  let url = format!("{}/message", state.config.gotify_url);
  state.http
      .post(&url)
      .header("X-Gotify-Key", &state.gotify_token)
      .json(&payload)
      .send()
      .await
  ```

- [x] **B3 (review-C1)** Add comments at both sites (`config.rs` near `Env::prefixed`
      and `main.rs` near `env::var("ACUITY_GOTIFY_TOKEN")`) explaining that
      `ACUITY_GOTIFY_TOKEN` is intentionally read manually and NOT part of `Config`,
      so the figment env layer silently ignores it (no `gotify_token` field). Write
      this comment AFTER B4 is done so the field names are final.

- [x] **B4 (review-C2)** In `config.rs`:

  - Rename field `gotify_host: String` -> `gotify_url: String`
  - Update default to `"http://localhost"` (no port, no path, no trailing slash)
  - After `extract()`, normalize: strip any trailing slash from `config.gotify_url`
    so `"http://localhost/"` and `"http://localhost"` both work

- [x] **B5 (review-C3)** In `handle_event`, replace the string-compare schema check
      with a typed `u8` parse. Full replacement:

  - Delete `let expected = SCHEMA_VERSION.to_string();`
  - Parse header as `u8` via `.trim().parse::<u8>()`
  - Compare parsed value to `SCHEMA_VERSION`
  - Rewrite the log branches to reference `SCHEMA_VERSION` directly (not `expected`)

- [x] **B6 (review-C4+C5+N2) -- single coherent main() rewrite**:

  - Replace `process::exit(1)` with `?` via `anyhow::anyhow!(...)`
  - Remove `Arc<AppState>`: drop `Arc::new(...)` wrap, change handler signature
    from `State<Arc<AppState>>` to `State<AppState>`
  - Remove `use std::sync::Arc;` import
  - Reorder startup: read `cfg.port` before moving `cfg` into `AppState`,
    eliminating `cfg.clone()`

- [x] **B7 (review-N1)** Remove `acuity-api` from `[dependencies]` in `Cargo.toml`.

- [x] **B8 (review-N3)** In `handle_event`, extract a `basename(path: &str) -> &str`
      free function. Implementation:

  - Trim trailing slashes: `let trimmed = path.trim_end_matches('/');`
  - If `trimmed` is empty, return `"unknown"`
  - `Path::new(trimmed).file_name()` -> fallback to `trimmed` on `None`
  - This handles: normal path, trailing slash, root `/`, empty string

- [x] **B9 (review-N4)** Change the `info!(...)` call in the success branch to use
      structured tracing fields:

  ```rust
  info!(session_id = %event.session_id, project_dir = %event.project_dir,
        "forwarded session.idle");
  ```

- [x] **B10** Run `cargo build -p acuity` -- zero errors, zero warnings.

- [x] **B11** Run `cargo test -p acuity` -- Part A tests still green.

- [x] **B12** Commit: `fix: address code review findings in acuity`

---

### Part C -- Happy-path and edge-case tests

- [x] **C1** Add `wiremock`-based happy-path test: `valid_event_forwards_to_gotify`:

  - Spin up a `wiremock::MockServer`
  - Set `AppState.config.gotify_url` to the mock server's URI
  - POST `/events` with correct schema header and a valid `SessionIdle` JSON body
  - Assert: handler returns `200`
  - Assert: mock server received exactly one request to `/message`
    with header `X-Gotify-Key: <token>` and JSON body containing
    `title`, `message`, `priority` fields

- [x] **C2** Add `wiremock`-based `502` test: `gotify_error_returns_502`:

  - Mock server returns `500`
  - POST valid event
  - Assert handler returns `502`

- [x] **C3** Add unit tests for the `basename` function (pure, no HTTP):

  - `/home/user/project` -> `"project"`
  - `/home/user/project/` -> `"project"`
  - `/home/user/project///` -> `"project"` (multiple trailing slashes)
  - `/` -> `"unknown"`
  - `""` -> `"unknown"`
  - `"relative"` -> `"relative"` (no directory component)

- [x] **C4** Run `cargo test -p acuity` -- all tests green.

- [x] **C5** Commit: `test: add happy-path, 502, and basename tests for acuity`

---

### Notes

- **A12 dropped**: narrowing tokio features (`"full"` -> minimal set) is out of
  scope for this PR. Cargo feature unification means the saving is marginal (axum,
  reqwest, hyper declare what they need regardless), and removing features breaks
  silently when future code uses e.g. `tokio::time`. Track as a future cleanup.
- **Verify X-Gotify-Key**: before merge, confirm the deployed Gotify version
  accepts token auth via `X-Gotify-Key` header (not just `?token=` query param).
  The `wiremock` test in C1 will catch a header-name mismatch in integration.
