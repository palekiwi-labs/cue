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

TypeScript harness plugins POST lifecycle event payloads to `acuity`. The wire
format is defined by the `acuity-schema` crate and generated to TypeScript via
`ts-rs`.

All POSTs must include a `X-Acuity-Schema: N` version header. `acuity` rejects
requests with a mismatched schema version.

The event envelope adds `seq` (monotonic sequence number) and `received_at`
(server timestamp) at ingest time.

### Lifecycle Events

- Agent session idle
- Agent tool call requested
- Agent tool call completed

## Outbound: SSE and Query API

`curator` and other consumers connect to `acuity` via:

- SSE for real-time event streams
- A query API for historical data

The response/read types are defined in the `acuity-api` crate.

## Crate Dependencies

- `acuity-schema`: ingest wire types
- `acuity-api`: read/response types

## Deferred

- Auth/trust boundary (API tokens, shared secret).
- Detailed endpoint design.
- Retention and pruning policy.
- Multi-host / remote deployment configuration.
