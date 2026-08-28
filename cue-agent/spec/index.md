---
title: cue-agent Specification — Supervised pi Launcher & Batch Engine
status: in-progress
priority: high
refs:
  - .cue/master/task/cue-agent.md
  - .cue/cue-agent/ref/pi-0.84.2-api.md
  - .cue/cue-agent/ref/pi-0.84.2-json-events.md
  - .cue/cue-agent/ref/supervisor-invariants.md
  - .cue/cue-agent/ref/cast-agent-supervisor.rs
---

# cue-agent Specification

Provenance: snapshot of the converged design from the palekiwi
coordination workspace (design task `design-cue-agent`,
`.cue/design-cue-agent/spec/index.md`). This copy is the
authoritative spec for this repo's build task; cross-workspace
paths were rewritten to the local refs above.

## 1. Intent & Scope

`cue-agent` is a process-isolated, supervised launcher and batch
execution engine for headless coding agents, embedded in the `cue`
workspace (`crates/cue-agent`). It executes JSON execution specs by
spawning `pi` child processes with supervision, timeout enforcement,
git worktree lifecycle management, and structured trace persistence
via `cuelib`.

### Key Architectural Principles

- **pi-only (MVP), API modeled after pi**: `pi` is the sole child
  harness for the MVP. The JSON spec fields adopt pi's flag
  vocabulary. When other harnesses are added later, fields map to
  their native APIs; fields a harness lacks become validated no-ops.
- **JSON Execution Spec**: All agent invocation parameters live in a
  JSON array payload — identical schema for one agent or n agents.
  `cue-agent` CLI flags describe only `cue-agent`'s own orchestrator
  behavior.
- **Session identity owned by cue-agent**: `cue-agent` mints run ids
  and session ids before spawn. The run id is the caller-facing
  handle; the session id enables conversation resume and is
  discoverable from saved artifacts.
- **Native Parallelism**: Spawns and supervises multiple agents in
  parallel with per-run failure isolation.
- **Process Isolation**: Each agent runs in its own process group
  with dedicated stream capture and teardown supervision.
- **Git Worktree Support**: Built-in ephemeral or persistent
  worktrees.
- **cue-native persistence**: All outputs are written by `cuelib` as
  artifacts into a cue context (see section 6).

### Scope & Constraints

- **Harness**: `pi` only (MVP), expected on `PATH`. Harness
  provisioning is out of scope.
- **pi baseline**: 0.84.2 — API reference with source citations:
  `.cue/cue-agent/ref/pi-0.84.2-api.md`; validated event-stream
  schema: `.cue/cue-agent/ref/pi-0.84.2-json-events.md`.
- **Consumers**: `opencode` (via the cue-plugins `delegate` tool,
  replacing the built-in `task` tool — separate repo) and `cue-review`
  (parallel multi-agent reviews, future).
- **Out of scope (MVP)**: other harnesses, background execution
  (reserved in schema), live `--stream` output, `--format text`
  convenience rendering, harness provisioning.

---

## 2. CLI Surface (orchestrator flags only)

```bash
cue-agent run [FLAGS] [JSON_SPEC]
```

Spec input modes (one of):

1. Positional JSON string: `cue-agent run '[{...}]'`
2. File: `cue-agent run --spec-file spec.json`
3. Stdin: `cat spec.json | cue-agent run -`

Flags:

- `--task <slug>`: cue context receiving artifacts. Default: active
  context from `.cue/HEAD`, else `master`. (Mirrors the `cue --task`
  convention; never mutates `.cue/HEAD`.)
- `--concurrency <N>`: max simultaneous children (default `0` =
  unbounded).
- `--timeout <secs>`: overall wall-clock cap for the whole batch.

There are no pi passthrough flags on the CLI. pi's own `--mode`
(text/json/rpc) is an internal concern: `cue-agent` always spawns
`pi -p --mode json` and consumes the JSONL event stream itself. The
caller never selects pi's output mode.

Output: `cue-agent` blocks until all runs finish, then prints the
final JSON array of per-run receipts on stdout (contract details in
section 6).

Reserved for the future (not in the MVP): `--stream` (live JSONL
event passthrough, annotated with run id), `cue-agent status
<run-id>` / `attach` — check-up on background runs (see section 7).

---

## 3. JSON Execution Spec

A JSON array; each element describes one agent run. Same schema for
1 or n agents.

```json
[
  {
    "id": "reviewer-a",
    "harness": "pi",
    "model": "google/gemini-3.6-flash",
    "system-prompt": "You are a performance auditor...",
    "append-system-prompt": ["Focus on hot paths."],
    "prompt": "Review the workspace diff and list bottlenecks...",
    "tools": ["read", "grep", "find", "ls"],
    "exclude-tools": [],
    "thinking": "medium",
    "approve": false,
    "session": { "persist": true },
    "env": { "GEMINI_API_KEY": "..." },
    "worktree": { "mode": "ephemeral", "base": "HEAD" },
    "timeout": 300,
    "background": false
  }
]
```

### Field Reference

pi-modeled fields (map onto pi flags):

- `id` _(string, optional)_: run id, unique within the batch.
  Defaults to `run-0`, `run-1`, ...
- `harness` _(string, optional)_: default `"pi"`; only `"pi"` valid
  in the MVP. Kept for forward-compatibility with other harnesses.
- `model` _(string, optional)_: pi `--model` pattern; supports
  `provider/id` and `provider/id:thinking` shorthand.
- `provider` _(string, optional)_: pi `--provider` (redundant with
  `provider/id` shorthand; provided for fidelity).
- `system-prompt` _(string | {file}, optional)_: replaces pi's
  default system prompt.
- `append-system-prompt` _(string | array of strings | {file},
  optional)_: appended to the baseline prompt; repeatable in pi,
  hence array-typed here.
- `prompt` _(string | {file}, required)_: the task prompt. Passed
  via tmpfile mechanism, never via argv (see section 8).
- `tools` _(array, optional)_: allowlist passed to `--tools`.
- `exclude-tools` _(array, optional)_: denylist passed to
  `--exclude-tools`.
- `no-tools` / `no-builtin-tools` _(booleans, optional)_: pi
  `--no-tools` / `--no-builtin-tools`.
- `thinking` _(enum, optional)_: `off | minimal | low | medium |
  high | xhigh | max` (pi 0.84.2 vocabulary).
- `approve` _(boolean, optional, default `false`)_: pi `--approve` /
  `--no-approve` — trust of project-local resources (`.pi/extensions`,
  `.pi/skills`, `.agents/skills`, `.pi/settings.json`, ...). NOT tool
  approval: headless pi auto-approves tool calls; supervision comes
  from `tools`/`exclude-tools` allowlists. Default `--no-approve`:
  subagents must not silently execute project-local extension code,
  especially in fresh ephemeral-worktree checkouts.
- `session` _(object, optional; default `{ "persist": false }`)_:
  - `persist` _(boolean)_: `false` passes `--no-session` (fully
    in-memory, ephemeral run). `true` persists the pi session.
  - `id` _(string, optional)_: session id. When `persist` is true
    and `id` is absent, `cue-agent` generates a UUIDv7. When `id`
    is present, pi resumes that session if it exists or creates it
    if missing (`--session-id` semantics).
- `background` _(boolean, optional, default `false`)_: reserved. MVP
  rejects `true` with a clear error; see section 7.

cue-agent orchestrator fields (no pi counterpart):

- `env` _(object, optional)_: environment overlay injected into the
  child. Values redacted in receipts/trace artifacts; keys
  preserved.
- `worktree` _(object, optional)_:
  - `mode`: `cwd` (default) | `ephemeral` | `named`.
  - `base`: git ref for ephemeral/named (default `HEAD`).
  - `name`: directory/branch name for `named`.
- `timeout` _(integer, optional)_: per-run wall-clock seconds.

File interpolation: `{ "file": "path" }` values are resolved
relative to the directory containing the spec file (cwd for
stdin/argv input).

---

## 4. Execution & Supervision

1. Parse and validate the spec array; mint run ids and (where
   `session.persist`) session ids before any spawn.
2. Spawn up to `--concurrency` children in parallel; per-run
   supervision with failure isolation (one run's crash/timeout never
   cancels siblings).
3. Flag translation (pi), per run:

   - always: `pi -p --mode json`
   - `prompt` -> positional message (via tmpfile-read content)
   - `model`, `provider`, `thinking`, `tools` (comma-joined),
     `exclude-tools`, `no-tools`, `no-builtin-tools`,
     `system-prompt`, `append-system-prompt` (one flag occurrence
     per array element) -> corresponding flags
   - `approve: false` -> `--no-approve`; `approve: true` ->
     `--approve`
   - `session.persist: false` -> `--no-session`
   - `session.persist: true` -> `--session-id <id>` plus a pinned
     `--session-dir` (section 5)
   - `env` -> `Command::envs` overlay
   - child cwd -> workspace dir, cwd, or managed worktree
   - child spawned in its own process group (`setpgid`)

4. Stream capture: raw JSONL events flushed live to `stream.jsonl`;
   the first event (session header) is validated against the
   expected session id as a sanity check.
5. Timeouts: per-run `timeout` and batch `--timeout`.
6. Signals: on SIGINT/SIGTERM, SIGTERM all child process groups,
   wait a 5s grace period, then SIGKILL remaining groups.
7. Receipt statuses: `completed | failed | timed_out |
   interrupted`, with exit code (0 success; 1 API/session error;
   129/143 signals).

Supervision mechanics (process groups, reader/writer tasks,
two-phase teardown, bounded joins) are specified in
`.cue/cue-agent/ref/supervisor-invariants.md`, with a proven
starting-point implementation in
`.cue/cue-agent/ref/cast-agent-supervisor.rs`.

---

## 5. Session Persistence Model

Two distinct identities:

- **Run id** — identifies the execution. Locates artifacts (receipt,
  stream, response). This is what `cue-agent` returns to the caller.
- **Session id** — identifies the pi conversation. Enables resume.
  Discoverable by the caller from the saved receipt; not required
  for ordinary consumption.

Both are minted before spawn, so a future background mode can return
the run id immediately at launch time.

### Session storage location (decided)

pi writes and owns its own session files (JSONL transcripts);
cue-agent does not manage or duplicate them — it only decides which
directory pi reads/writes via `--session-dir`. The default
(`~/.pi/agent/sessions/--<encoded-cwd>--/`) is project-scoped by the
child's cwd. Because `--session-id` lookup misses silently create an
empty session, resume under default storage only works when every
run of a conversation shares the same cwd — false for ephemeral
worktrees.

Decision: whenever `session.persist` is true, cue-agent pins
`--session-dir` to `.cue/<context>/sessions/` (the target cue
context, resolved the same way as `--task`). Effects: resume is
cwd-independent (works across ephemeral worktrees); machine sessions
stay out of interactive pi history; sessions remain inspectable
(`pi --export <file>` renders HTML; interactive pi attaches with
`--session-dir`).

### Resume

The caller re-passes `session.id` (read from the prior run's
receipt) together with a new prompt; pi replays the prior context
(`--session-id` resume semantics). Cross-harness resume is out of
scope.

---

## 6. Output Artifacts (cuelib)

No `--output-dir` flag. All artifacts are written via `cuelib` into
the target cue context (`--task`, default `.cue/HEAD`, else
`master`) as `tmp` point-in-time artifacts.

**Flat layout** — parallelism defines execution, not output layout.
A run launched alone and a run launched among nine siblings produce
the same thing: one tmp artifact directory per run.

```
.cue/<context>/tmp/<timestamp>-<run-id>/
    prompt.md
    system-prompt.md
    stream.jsonl        # raw pi JSONL events, live-flushed
    stderr.log
    response.md         # extracted final assistant text
    receipt.json

.cue/<context>/tmp/<timestamp>-batch/batch-receipt.json
```

`batch-receipt.json` is a single summary artifact (overall duration,
concurrency, per-run id/status/duration table); it references runs,
it does not contain them.

`receipt.json` per run — **self-sufficient**: a consumer can act on
a result from the receipts array alone, without reading artifacts:

- `id`, `status`, `exit_code`, `duration_ms`, `event_count`
- `response`: the final assistant text, embedded verbatim (also
  stored durably as `response.md`)
- `error`: failure reason / stderr tail; present when
  `status != "completed"`
- `artifacts`: absolute paths for drill-down — the run's artifact
  directory, and the worktree path when one was used
- `session`: `{ "persist": bool, "id": "<uuid>", "file": "<path>" }`
  (absent/ephemeral when `persist` false)
- `worktree`: mode and path used
- `env`: keys only, values redacted (`"[REDACTED]"`)
- `model`, `thinking` as actually applied

Extraction rule for `response` (validated against live pi 0.84.2
probes; full schema in `.cue/cue-agent/ref/pi-0.84.2-json-events.md`):
on `agent_end` (carries the full conversation transcript), take the
last entry with `role == "assistant"` and concatenate its
`type:"text"` content blocks (skip `thinking`/`toolCall`, drop
`textSignature`, tolerate empty text blocks). Equivalent fallback:
the last `message_end` with assistant role before `agent_settled`.
Use the final message's `stopReason` (`"stop"` expected) as a health
signal. Streams can be legitimately EMPTY (pre-flight failures like
a bad model print nothing on stdout — not even the session header —
and put an ANSI-colored error on stderr with exit 1).

### stdout & exit-code contract

- Whenever the spec parses, stdout is a valid JSON array of
  receipts — even if every run failed. Per-run truth lives inside
  the receipts, never in stdout validity.
- Exit codes: `0` all runs completed; `1` one or more runs failed /
  timed out / interrupted; `2` spec or usage error (human-readable
  message on stderr, no receipts on stdout).
- No human-friendly output mode: humans consume results via the cue
  artifacts, machines via receipts.

---

## 7. Background Execution (reserved)

Not in the MVP. The schema reserves `background: true` (MVP rejects
it with a clear error). Future design: wrap pi in `herdr`
(agent-native terminal multiplexer with CLI + socket API and pane
liveness states). Because run ids and session ids are minted
pre-spawn, `cue-agent` can return the run id immediately on launch;
check-up later via a `cue-agent status <run-id>` subcommand reading
live artifacts (and herdr liveness). Mixed batches (one background
report-writer + one synchronous research run) are the target use
case.

---

## 8. opencode Integration (`delegate`) — consumer contract

Implemented in a separate repo (palekiwi-labs/cue-plugins); recorded
here because it constrains the receipts contract.

- `cue-plugins` registers a tool named **`delegate`** — a new name,
  not shadowing the built-in `task` tool.
- Presets live in `~/.config/cue/agents.json` or `.cue/agents.json`:
  each preset is a named bundle of spec fields (tools allowlist,
  system-prompt, model default, thinking, ...).
- Tool schema: `{ agent: <preset>, description: <short summary>,
  prompt: <task> }`.
- Executor: constructs a single-element spec array and invokes
  `cue-agent run`. Prompt is written to a tmpfile (never
  argv/stdin). On completion it returns `receipts[0].response` as
  the tool's string result; on failure it surfaces `status` +
  `error` from the receipt as the tool error.
- Abort ladder: spawn detached; on abort SIGTERM the `cue-agent`
  process, let its supervisor tear down children in their own
  process groups, backstop SIGKILL after 10s.
- Reference implementation: cue-plugins worktree
  `feat/castagent-task-tool` (`src/opencode/task.ts`).
