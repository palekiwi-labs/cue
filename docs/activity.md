# Context activity

Activity is an automatic history alongside deliberate milestone logs. It does
not measure progress, infer focus, or provide an audit-grade record.

## Commands

```sh
cue activity add --context acme/project/work \
  --kind git_commit_attempt \
  --details '{"command":"git commit -m example","cwd":"/workspace/project"}' \
  --harness pi --session-id session-123

cue activity list --context acme/project/work --limit 20 --json
cue activity list --context work --since 1790000000000

cue add findings 'Research findings' --type note --context work \
  --harness pi --session-id session-123
```

`activity add` accepts:

- `--kind`: `artifact_created`, `git_commit_attempt`, or `session_associated`.
- `--details`: a required JSON object; fields are producer-defined.
- `--session-id` and `--harness`: optional provenance. Harness names are strings,
  not a fixed list. No harness is inferred when absent.
- `--host`: optional, defaulting to `CAST_HOST_NAME`. An explicit empty host
  suppresses that default. Empty provenance strings are omitted.

`--context` accepts a slug or canonical `<org>/<repo>/<context>` address.
Canonical addresses work outside the addressed repository. Context resolution
uses the explicit flag, `CUE_CONTEXT`, then branch configuration, as usual.
`--store` and `-C` work on activity commands as on other cue commands.

With no selected context, `activity add` succeeds silently without writing.
A selected but missing or invalid context is an error, including a stale
selection from the environment or branch configuration. `activity list`
requires a selected, existing context.

Successful `activity add` prints the event file path. Direct recording errors
return nonzero. Producers must catch them and warn without changing the original
action's success. Missing context is an intentional skip, not a warning.

`activity list --since` uses inclusive Unix milliseconds. `--limit` selects the
newest N valid matching events, then renders them chronologically, breaking
timestamp ties by ID. No limit means all events; zero means none. JSON output is
an array of event objects. Human output has one timestamp, ID, kind, and details
object per line. Neither format adds a context field: the command reads exactly
one context, identified by the selector.

## Event contract and ownership

```json
{
  "id": "8123a111-fb74-4111-8af6-dc175c8dbaf1",
  "timestamp": 1790000000000,
  "kind": "artifact_created",
  "host": "desktop",
  "session_id": "session-123",
  "harness": "pi",
  "details": {
    "address": "acme/project/work/note/findings.md",
    "type": "note"
  }
}
```

IDs are random UUID v4; timestamps are Unix milliseconds. Context is implicit in
storage. Provenance fields are optional. Readers tolerate unknown envelope
fields, but do not return them; unsupported kinds are malformed records.

Cue records `artifact_created` exactly once after each successful `cue add`,
including `--force` replacement. This denotes successful artifact publication,
not necessarily first creation of that address. Failed artifact writes emit no
event. Recording failures only warn on stderr; the created artifact still
succeeds. The actual destination determines attribution, including cross-scope
addresses and the generated timestamp/revision directory of `tmp` artifacts.

Harness integrations own `git_commit_attempt` and `session_associated` events.
For git attempts, details should contain the observed `command` and execution
`cwd`; do not claim a verified commit hash. Session association records belong
to the newly selected context, with session and harness in the envelope.
Integrations must not duplicate cue's native artifact events. Creating a context
does not record an event in this initial implementation.

## Storage and reliability

Events live at:

```text
<store>/<org>/<repo>/<context>/activity/<20-digit-unix-ms>-<uuid>.json
```

Each write creates a unique adjacent `.activity-staging-*` file, finishes and
flushes the payload, then publishes with `tempfile::persist_noclobber`. Existing
events are never replaced. A collision returns an error. Published files are
never subsequently modified by cue.

On Linux, tempfile uses no-replace rename where supported, with a hard-link and
unlink fallback. Cleanup is best-effort: crashes or fallback unlink failures
can leave staging residue. There is no automatic residue cleanup. Temporary
files retain tempfile's private permissions (0600 on Unix); synchronized stores
must be accessible to the user running cue and Syncthing.

Readers recognize only canonical final filenames and regular files, not
symlinks. The filename timestamp and ID must match the envelope, and timestamps
must fit a JavaScript-safe integer. Staging files, conflict filenames, and other
non-event filenames are ignored. Malformed payloads warn on stderr and are
skipped without consuming the limit. Real directory or record I/O failures are
errors, even with `--limit 0`. A missing activity directory means an empty stream.
JSON stdout remains machine-readable when warnings occur.

Application staging files can synchronize through Syncthing. Reader correctness
does not depend on ignore rules. An isolated Linux/Syncthing 2.1.2 experiment
with paused peers and concurrent writers verified exact convergence of 80 events
and showed a deliberately incomplete staging sample replicating but remaining
invisible to cue readers.

Flush is not fsync: power-loss durability is not guaranteed. The experiment did
not test crash injection, forced hard-link fallback, all filesystems, or Windows.
The store is trusted, not hardened against hostile directory mutation.
Chronology follows producer clocks, not causal order. Clock skew and late
synchronization mean `--since` is not a reliable synchronization cursor.
