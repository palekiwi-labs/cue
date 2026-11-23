# cue

`cue` is the file-based memory CLI at the core of the ecosystem. It manages
structured, branch-isolated artifacts (specs, plans, todos, tasks, traces,
logs) under a project's `.cue/` directory so an agent retains intent and
history across sessions.

## Install

```
nix run github:palekiwi-labs/cue
nix profile add github:palekiwi-labs/cue
```

## Usage

Create a context for the current project, then add artifacts to it:

```
cue context create <slug>
cue add <filename> "<content>" --context <slug>
```

The central store root is selected by `$CUE_STORE` or defaults to `~/cue`.
Repositories are partitioned by their origin-derived `<org>/<repo>` scope, so
linked worktrees and clones of the same origin share context without project-
local cue files.

Scoped commands resolve their active context in this order:

1. an explicit `--context <slug>` argument;
2. the `$CUE_CONTEXT` environment variable;
3. `branch.<current-branch>.cue-context` in local Git configuration;
4. unset.

Agents should set `$CUE_CONTEXT` for child processes and sessions so they
inherit the intended context without changing repository configuration.

### Context activity

`cue activity add` and `cue activity list` record and inspect automatic context
activity. Successful `cue add` calls emit native artifact events, with optional
`--harness` and `--session-id` provenance. See [activity](activity.md) for the
CLI, event contract, producer ownership, and synchronization guarantees.

### Lifecycle hooks

Executable hooks in `$CUE_CONFIG_DIR/hooks/all.d/` run for every event;
`hooks/<event>.d/` selects one event. The default config directory is
`~/.config/cue`. Hooks receive JSON on stdin after a successful store write.
Failures warn but never roll back the mutation; delivery is best-effort,
with no outbox or retries.

Events are `artifact_create`, `context_create`, `log_entry_add`,
`artifact_update`, `context_update`, and `artifact_delete`. Every payload
contains `v: 1`, `event`, a unique `event_id`, canonical `address`, UTC
RFC 3339 `occurred_at`, and `session_id` from `PI_SESSION_ID` (null when
absent or empty). Creation payloads retain their existing fields.

Updates emit one event per changed frontmatter field. Each includes
`mutation_id`, `field`, `old`, and `new`; absent values are JSON null and
lists remain arrays. All fields changed by one command share a mutation ID
but have distinct event IDs. Deletes include `mutation_id` without field
values. For example:

```json
{
  "v": 1,
  "event": "artifact_update",
  "event_id": "a-unique-event-id",
  "address": "acme/widgets/release/task/ship.md",
  "occurred_at": "2026-01-01T00:00:00Z",
  "session_id": null,
  "mutation_id": "a-shared-mutation-id",
  "field": "status",
  "old": "open",
  "new": "complete"
}
```

`cue context archive` and `unarchive` use the same store-wide mutation lock
and atomic writer as updates. They emit `context_update` for `archived_at`,
with its timestamp or null as the value. Repeated archive/unarchive calls,
unchanged fields, dry runs, and failed mutations emit no events.

Hooks receive `CUE_HOOK_EVENT`, `CUE_HOOK_EVENT_ID`,
`CUE_HOOK_PARENT_EVENT_ID` (only for nested events), `CUE_HOOK_ROOT_EVENT_ID`,
`CUE_HOOK_DEPTH`, and `CUE_STORE`. Dispatch results are appended to
`$XDG_STATE_HOME/cue/hooks.jsonl` (default `~/.local/state/cue/hooks.jsonl`);
every record includes the same session ID or null. No additional actor
attribution is inferred.

The NATS publisher uses `cue.evt.v1.<ctx>.<event>`, where `<ctx>` is the
base64url-encoded context address without padding. Update events have no
field suffix; consumers inspect `field` in the payload.

### Render artifacts

`cue render` emits explicitly named files as `<artifact path="...">` blocks for
session injection. Relative paths resolve inside the selected context:

```
cue render context.md spec/index.md plan/index.md --context release
```

Use `-` where newline-delimited paths from stdin should appear. This composes
`cue list` filtering with rendering while preserving the order of explicit and
piped entries:

```
cue list --context release --type task --filter 'status!=complete' \
  | cue render context.md spec/index.md plan/index.md - --context release
```

Plain `cue list` output uses absolute paths, so an all-piped render does not
need an active context:

```
cue list --context release --type task | cue render -
```

Run `cue --help` for the full command reference.

> This page is a stub. Detailed usage and the artifact format will be
> documented here.
