# Code Review: feat/acuity-mvp

Branch: feat/acuity-mvp
Commits reviewed: 5a3ea36, 82a2f44
Reviewer role: expert Rust engineer

---

## Summary

A clean, minimal stateless MVP. The overall structure -- axum handler, figment
config, reqwest forwarding -- is sound and idiomatic for this scale of service.
The issues below are real and worth fixing before this ships, but none of them
are architectural.

---

## BLOCKER

### B1 -- No request body size limit (DoS vector)
`crates/acuity/src/main.rs:66`

`body: axum::body::Bytes` reads the entire body into memory before the handler
is called. Axum's default body-size limit in 0.8 is 2 MB. For this endpoint
that is 2 MB per unauthenticated POST -- a trivial amplification attack on a
low-resource binary. The limit should be set explicitly and much smaller (a few
kilobytes is more than enough for a SessionIdle payload).

```rust
// In the router builder:
let app = Router::new()
    .route("/events", post(handle_event))
    .layer(axum::extract::DefaultBodyLimit::max(16 * 1024)) // 16 KiB
    .with_state(state);
```

### B2 -- Gotify token logged in error traces via URL construction
`crates/acuity/src/main.rs:119-122`

The Gotify token is embedded in the URL query string:

```rust
let url = format!(
    "http://{}/message?token={}",
    state.config.gotify_host, state.gotify_token
);
```

If the `reqwest::Error` path is hit (line 140) and the error is ever surfaced
to a logging backend that captures the full error chain (which includes the
request URL), the token will be written to logs in plaintext. The Gotify API
accepts the token as an `X-Gotify-Key` header instead. Use that.

```rust
let url = format!("http://{}/message", state.config.gotify_host);

state.http
    .post(&url)
    .header("X-Gotify-Key", &state.gotify_token)
    .json(&payload)
    .send()
    .await
```

---

## CONCERN

### C1 -- `ACUITY_GOTIFY_TOKEN` is silently shadowed / collides with figment env layer
`crates/acuity/src/config.rs:35`, `crates/acuity/src/main.rs:37`

Figment is configured with `Env::prefixed("ACUITY_").split("__")` (config.rs:35).
`ACUITY_GOTIFY_TOKEN` matches this prefix. Figment will attempt to deserialize
`ACUITY_GOTIFY_TOKEN` as `Config::gotify_token`, but `Config` has no such field,
so the extract silently ignores it. This is harmless today, but it creates a
confusing mental model: the token appears to be configured via env at the
`Config::load()` call site, yet it is actually read separately in `main`.

If someone adds `gotify_token: String` to `Config` later (a natural refactor),
there will be two reads of the same env var with no obvious link between them.

Consider either:
- Adding `gotify_token` to `Config` and removing the standalone `env::var` call,
  or
- Adding a comment at both read sites explaining the intentional split.

### C2 -- `gotify_host` default uses HTTP, no HTTPS path exists
`crates/acuity/src/config.rs:16`, `crates/acuity/src/main.rs:119-120`

The default host is `localhost:80` and the URL is unconditionally constructed
with the `http://` scheme (main.rs:119). Even for a local-only MVP, making HTTPS
impossible without a code change is a footgun. Consider accepting a full base URL
(`http://localhost:80`) in the config, or splitting `scheme` and `host`. This
makes a future TLS deployment a config change rather than a code change.

### C3 -- Schema version comparison is string equality, not numeric
`crates/acuity/src/main.rs:73-87`

`SCHEMA_VERSION` is `u8`. It is converted to a `String` (line 73) and then
compared as a string to the header value. This works for single-digit versions
but would misorder multi-digit versions (e.g. `"9" > "10"` lexicographically).
More importantly the intent -- "header value must equal the exact supported
version" -- is correct, but parsing the header value as a `u8` first and
comparing as integers would be more explicit about the type contract:

```rust
let schema_version: u8 = match schema_header.and_then(|v| v.parse().ok()) {
    Some(v) => v,
    None => {
        error!("rejected event: missing or non-numeric X-Acuity-Schema header");
        return StatusCode::BAD_REQUEST;
    }
};
if schema_version != SCHEMA_VERSION { ... }
```

### C4 -- `process::exit(1)` bypasses cleanup in `main`
`crates/acuity/src/main.rs:41`

Using `std::process::exit(1)` prevents any Drop impls from running. For an MVP
binary this is acceptable, but it is inconsistent with the `anyhow::Result`
return type of `main`. Propagating the error with `?` (or returning
`Err(anyhow::anyhow!(...))`) is cleaner and consistent with the rest of main's
error handling:

```rust
let gotify_token = std::env::var("ACUITY_GOTIFY_TOKEN")
    .map_err(|_| anyhow::anyhow!("ACUITY_GOTIFY_TOKEN is required but not set"))?;
```

### C5 -- `Arc<AppState>` is redundant; `State<AppState>` is sufficient
`crates/acuity/src/main.rs:44,64`

`AppState` derives `Clone`, which is the only requirement for axum's `State`
extractor. Wrapping it in `Arc` is harmless but adds unnecessary indirection.
`reqwest::Client` is already `Clone + Arc`-backed internally; `config::Config`
is `Clone`. Drop the `Arc` and pass `State(state): State<AppState>` directly.

### C6 -- `dirs` v6 depends on `dirs-sys` which may pull in libc/winapi; check alternatives
`crates/acuity/Cargo.toml:18`

`dirs = "6"` is used only for `dirs::home_dir()` in config.rs:29. On the one
hand this is a common, well-maintained crate. On the other hand for a server
binary `$HOME` is a perfectly reasonable fallback: `std::env::var("HOME")`. If
the team wants to minimize the dependency surface this is a straight swap. Not a
blocker, but worth a deliberate decision.

---

## NIT

### N1 -- Unused `acuity-api` dependency
`crates/acuity/Cargo.toml:8`

`acuity-api` is listed as a dependency but its `lib.rs` is a two-line stub
explicitly marked "Populated in Phase 5". It is not referenced anywhere in
`main.rs` or `config.rs`. Remove it from `[dependencies]` until Phase 5 is
implemented to keep the dependency graph clean.

### N2 -- `cfg.clone()` is unnecessary
`crates/acuity/src/main.rs:45`

`cfg` is moved into `AppState::config` on line 45 and then used again on
line 54 (`cfg.port`). The clone is needed because both accesses require the
value. This is fine, but the ordering could be inverted -- read `cfg.port`
first, then move `cfg` into the state -- to avoid the clone:

```rust
let addr = format!("0.0.0.0:{}", cfg.port);
let state = Arc::new(AppState {
    config: cfg,
    ...
});
```

### N3 -- `basename` fallback loses path separators
`crates/acuity/src/main.rs:99-102`

```rust
let basename = Path::new(&event.project_dir)
    .file_name()
    .and_then(|n| n.to_str())
    .unwrap_or(&event.project_dir);
```

If `project_dir` is something like `/foo/bar/` (trailing slash), `file_name()`
returns `None` and the fallback is the full path string. The Gotify
notification title would then contain the full path rather than the directory
name. `.file_name()` on a path ending in `/` returns `None` on Unix.
`.canonicalize()` is async-hostile here, but stripping a trailing slash before
calling `Path::new` covers the common case:

```rust
let trimmed = event.project_dir.trim_end_matches('/');
let basename = Path::new(trimmed)
    .file_name()
    .and_then(|n| n.to_str())
    .unwrap_or(trimmed);
```

### N4 -- Structured log fields vs positional formatting
`crates/acuity/src/main.rs:126-129`

```rust
info!(
    "forwarded session.idle for session={} project={}",
    event.session_id, event.project_dir
);
```

`tracing` supports structured key-value fields natively. Prefer:

```rust
info!(
    session_id = %event.session_id,
    project_dir = %event.project_dir,
    "forwarded session.idle"
);
```

This emits structured spans/events usable by JSON formatters without string
parsing.

### N5 -- `tokio = { features = ["full"] }` pulls in unnecessary runtime components
`crates/acuity/Cargo.toml:10`

For a simple HTTP server the minimal required features are `["rt-multi-thread",
"net", "macros"]`. `full` adds `fs`, `process`, `signal`, `sync`, `time`, etc.
Not a correctness issue, but it increases compile time and binary size.

### N6 -- No `[profile.release]` in Cargo.toml
`crates/acuity/Cargo.toml`

A server binary benefits from `opt-level = 3` and `lto = "thin"` in a release
profile. Not present. Low priority for an MVP, but worth adding to the workspace
`Cargo.toml` if not already there.

---

## PRAISE

### P1 -- Clean figment layering
`crates/acuity/src/config.rs:23-39`

The three-layer config stack (defaults -> JSON file -> env overrides) is exactly
right. Respecting `ACUITY_CONFIG_DIR` before falling back to `$HOME/.config` is
a good, XDG-aligned convention. Silent tolerance of a missing JSON file
(figment's `Json::file` behaviour) is appropriate for a first-run experience.

### P2 -- Schema version pinning
`crates/acuity/src/main.rs:73-87`

Rejecting events with a mismatched `X-Acuity-Schema` header before touching the
body is the right design. It prevents silent schema drift and gives producers a
clear, actionable error signal.

### P3 -- Graceful Gotify error surfacing
`crates/acuity/src/main.rs:132-142`

Returning `502 Bad Gateway` for upstream Gotify failures, and `200 OK` only on
confirmed success, is semantically correct and gives callers accurate feedback
about delivery.

### P4 -- `reqwest` with `rustls-tls` and `default-features = false`
`crates/acuity/Cargo.toml:11`

Disabling default features to avoid pulling in OpenSSL and pinning to rustls is
the right call for a portable server binary.

---

## Missing Tests

The following tests would materially improve confidence for this MVP:

1. **Header validation** -- handler rejects missing `X-Acuity-Schema`, wrong
   version, and accepts the correct version.
2. **Body parsing** -- handler returns `422` on malformed JSON and `200` on a
   valid `SessionIdle` payload.
3. **Gotify forwarding payload shape** -- assert the JSON sent to Gotify has
   the expected `title`, `message`, and `priority` fields.
4. **Gotify error propagation** -- mock a 500 from Gotify and assert the handler
   returns `502`.
5. **Config loading** -- unit test `Config::load()` with env vars set, with a
   JSON file, and with neither (defaults).
6. **`basename` edge cases** -- trailing slash, root path `/`, non-UTF8 path.

For axum handler tests `axum_test` or `tower::ServiceExt::oneshot` are the
standard approaches and do not require a live socket.
