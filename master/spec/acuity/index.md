# Acuity Specification

## Purpose

`acuity` is an observability server for the cue ecosystem. It collects agent
lifecycle events from TypeScript harness plugins, stores them in SQLite, and
serves them to consumers (primarily `curator`) via SSE and a query API.

`acuity` is a standalone network service. It has no dependency on the `.cue/`
filesystem layout and does not depend on `cuelib`.

## Storage

SQLite. Embedded, single-file, suitable for single-host and small-team use.

## Inbound: Event Ingestion

TypeScript harness plugins POST JSON lifecycle event payloads to `acuity`. The
wire format is defined by the `acuity-schema` crate and generated to TypeScript
via `ts-rs`.

### Ingest endpoint

`POST /events`

### Schema versioning

All POSTs must include an `X-Acuity-Schema: N` header where `N` is a
monotonic integer. The authoritative value of `N` is a constant defined in
`acuity-schema` and is the single source of truth for both the server and the
generated TypeScript types. `acuity` rejects requests with a missing or
mismatched schema version with `HTTP 400`.

### Ingest envelope

The event envelope adds `seq` (monotonic sequence number) and `received_at`
(server timestamp) at ingest time.

### Lifecycle Events

- `SessionIdle`
- `ToolCallRequested`
- `ToolCallCompleted`

## Outbound: SSE and Query API

`curator` and other consumers connect to `acuity` via:

- SSE for real-time event streams
- A query API for historical data

The response/read types are defined in the `acuity-api` crate.

## Configuration

`acuity` uses a layered configuration model:

1. Compiled-in defaults
2. JSON config file (`~/.config/acuity/acuity.json`, or the path in
   `$ACUITY_CONFIG_DIR/acuity.json` if that env var is set)
3. Environment variables prefixed `ACUITY_` (with `__` for nesting)

Secrets (e.g. downstream API tokens) are passed via environment variables only
and are never stored in the config file.

## Crate Dependencies

- `acuity-schema`: ingest wire types
- `acuity-api`: read/response types

## Deferred

- Auth/trust boundary (API tokens, shared secret).
- Outbound endpoint design (SSE stream, query API).
- Retention and pruning policy.
- Multi-host / remote deployment configuration.
