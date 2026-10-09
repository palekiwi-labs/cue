# cue-agent

`cue-agent` runs named coding agents as child harness processes and reports
what they produced. One invocation is one batch: a JSON run specification
lists up to four tasks, each naming an agent, and they run concurrently, each
in its own process group. The command prints a single receipt, as a human
summary or as JSON, when the last of them finishes.

It is a standalone binary. Nothing calls it automatically yet: a Pi delegation
extension is a separate piece of work.

## The engine, in one paragraph

Each child's stdout and stderr go to files, never pipes, and one synchronous
loop polls every live child while watching the abort flag and each deadline.
A pipe would oblige the parent to keep draining it and would signal completion
only at EOF, which arrives only once every process holding the write end has
exited. Coding agents run arbitrary bash, so a grandchild inheriting stdout is
ordinary, and with a pipe it hangs the parent forever. With files, child exit
and output completion are independent questions, answered by `try_wait` and by
the file respectively. There is no async runtime, no reader thread and no
channel anywhere in the crate.

## Defining agents

Agents live in a JSON manifest, read from two layers and merged with the local
layer overriding field by field:

1. `$XDG_CONFIG_HOME/cue/cue-agent.json` (falling back to
   `~/.config/cue/cue-agent.json`)
2. `cue-agent.json`, the nearest one at or above the working directory,
   stopping at the repository root

`agents` is an object keyed by agent name, never an array: a merge replaces an
array wholesale, so an array would let a project file wipe every global agent
instead of overriding one field of one agent.

```json
{
  "timeout": 900,
  "agents": {
    "explore": {
      "description": "Surveys unfamiliar code and reports what is where",
      "model": "anthropic/claude-haiku-4",
      "system_prompt": "You explore code and answer with findings only.",
      "tools": ["read", "bash"]
    },
    "consultant-opus": {
      "description": "Consults on hard design questions",
      "model": "anthropic/claude-opus-4",
      "system_prompt": { "file": "prompts/consultant.md" }
    },
    "diff-reviewer-flash": {
      "description": "Reviews a diff and lists defects",
      "model": "google/gemini-flash",
      "system_prompt": "You review diffs. List defects, most severe first."
    }
  }
}
```

The root accepts only `timeout`, `worktree_root`, and `agents`:

- `timeout` — non-negative integer seconds, default `0` (unlimited).
- `worktree_root` — optional parent directory for generated worktree
  checkouts (see "Worktrees" below). Absolute paths are kept; relative paths
  resolve against each execution target's directory, not the manifest
  directory or the Git root.
- `agents` — named reusable definitions, with the fields below.

Agent fields, all optional except the agent's own key:

- `description` — what the agent is for; this is what a calling model reads.
- `model` — passed as `--model`; omit to inherit the harness default.
- `system_prompt` — supplementary instructions passed via
  `--append-system-prompt`, preserving Pi's base prompt. Omission means no
  supplementary instructions.
- `tools` — omit or set to `null` for Pi defaults; `[]` passes `--no-tools`,
  disabling all tools including extension tools. A nonempty array passes a
  comma-separated `--tools` selection.
- `thinking` — passed as `--thinking`.

Every string value also accepts a single-key `{ "file": "path" }` object,
including individual elements of `tools`, but not the whole array. Locate the
file relative to the declaring manifest; use its contents verbatim without
trimming, JSON parsing, or recursive expansion. For file-sourced
`worktree_root`, locate the file this way, then interpret its contents as a
path relative to the execution target.

Fields and arrays replace inherited values atomically. `null` clears optional
values; clearing `system_prompt` removes supplementary instructions. Root
`timeout`, `agents`, and whole agent definitions cannot be null. Unknown keys,
legacy `system_prompt_file`/`timeout_secs`, and task defaults such as `prompt`,
`env`, or `cwd` in agent definitions are rejected. Each layer is type-checked
even for overridden fields; only winning file references are read.

A project file overrides one field without restating the agent:

```json
{ "agents": { "explore": { "model": "anthropic/claude-sonnet-4" } } }
```

Inspect the result:

```
cue-agent agents list
cue-agent agents list --json
```

JSON discovery returns an object with `agents`, `global_manifest`, and
`project_manifest`. Each resolved agent includes its name and reusable fields,
`source` (the last layer mentioning the agent), and `field_sources` (the layer
supplying each non-cleared field). Layers are named `user` and `project`.
Unspecified or cleared tools are `null`, distinct from explicit `[]`.

## Running agents

`cue-agent run` takes one JSON run specification from exactly one source:

```
cue-agent run '<JSON>'          # literal JSON as the positional argument
cue-agent run - < spec.json     # "-" reads the JSON from standard input
cue-agent run --spec spec.json  # read the JSON from a file
```

A positional argument is always JSON (or `-`), never guessed to be a path;
use `--spec` for files. Giving both a positional argument and `--spec`, or
neither, is a usage error.

The smallest request runs one named agent on one prompt:

```
cue-agent run '{"tasks": [{"agent": "explore", "prompt": "Where is session admission decided?"}]}'
```

### Options

- `--spec <PATH>` — read the specification from a file. Mutually exclusive
  with the positional argument. A relative path is taken from the invocation
  directory.
- `--json` — print the batch receipt as one JSON object. Without it, a
  human-readable summary is printed instead (see "The receipt").
- `--timeout <SECS>` — per-run deadline, overriding the manifest root
  `timeout` for this invocation. `0` means no deadline.
- `--help` — usage.

Prompts, working directories, environment, capture context and agent field
overrides belong in the specification; there are no CLI flags for them.
There is no harness override flag or variable and no dry-run mode.

### The envelope

The root is an object with three fields, and unknown fields are rejected:

- `tasks` — required, a nonempty array of task objects, at most four.
- `defaults` — optional object of shared task values.
- `label` — optional batch label (a string or file reference).

Each task requests one independent execution, in the order given. Every task
must name its agent explicitly with `agent`; `defaults` cannot select one.
Repeating an agent name requests separate, independent executions:

```json
{
  "label": "Review the branch diff",
  "defaults": {
    "prompt": { "file": "review-prompt.md" },
    "context": "acme/widgets/cue-agent-runtime-mvp"
  },
  "tasks": [
    { "agent": "diff-reviewer-flash" },
    { "agent": "diff-reviewer-flash", "model": "google/gemini-pro",
      "label": "Second opinion" },
    { "agent": "consultant-opus",
      "prompt": "Is the teardown order correct under an interrupt during the grace window?" }
  ]
}
```

A worked example lives in `docs/examples/cue-agent/spec.json`, with a
matching manifest in `docs/examples/cue-agent/agents.json`.

Fields accepted on a task (and, except `agent`, in `defaults`):

- `agent` — required on every task; a name defined in the merged manifest.
- `prompt` — required after defaults are applied.
- `label` — optional per-task label; it falls back to the batch label.
- `cwd` — optional absolute working directory.
- `env` — optional environment overlay object.
- `context` — optional canonical capture context address.
- `worktree` — optional request to run in a new worktree (see below).
- `description`, `model`, `system_prompt`, `tools`, `thinking` — overrides
  of the named agent's manifest fields, with the same meaning and types as in
  the manifest.

There is no timeout field anywhere in the specification: `timeout` and the
legacy `timeout_secs` are rejected as unknown fields. The bare-array form,
the `runs` key, and the old `--batch`, `--prompt`, `--prompt-file`,
`--context`, `--label`, `--cwd` and `--harness` flags no longer exist.

### Defaults and overrides

Each field resolves in this order, highest first: the task's own value, then
`defaults`, then the named agent's manifest definition. Manifest agents supply
only reusable agent fields; prompt, label, cwd, env, context and worktree come
from the specification alone.

- Omitting a field inherits it.
- An explicit `null` clears an inherited optional value: `"label": null`,
  `"context": null`, `"model": null` and so on. Clearing `system_prompt`
  removes supplementary instructions; clearing `tools` returns to Pi defaults.
  `null` never satisfies a required field: a task with `"prompt": null` is
  rejected even if `defaults` has a prompt.
- Scalars replace. Arrays such as `tools` replace whole, never concatenate.
- `worktree` replaces as a whole object.
- `env` merges by variable name (see "Environment").

### File-sourced values

Wherever the specification accepts a string, it also accepts a single-key
`{ "file": "path" }` object, including `agent`, `prompt`, labels, `cwd`, env
values, individual `tools` elements and worktree strings. The file's contents
are used verbatim as the string: not trimmed, not parsed as JSON, and never
expanded recursively, so Markdown and text that looks like a reference stay
literal.

Relative reference paths resolve against:

- the specification file's directory, for `--spec PATH` (the directory of the
  path as named, without resolving symlinks);
- the invocation directory, for positional JSON and standard input.

A task's `cwd` never relocates the reference base. Manifest references keep
resolving against their declaring manifest. Only winning values are read: a
default overridden by every task is never opened. A missing or unreadable
winning file rejects the whole request.

### Prompts

The effective prompt must not be empty or whitespace-only, and it is passed to
Pi and recorded byte for byte, surrounding whitespace included. It must not
begin with `-` or `@`: Pi's current prompt transport parses those prefixes as
options or file references, and Pi offers no `--` separator. Prepend ordinary
instruction text when passing such content. Strings that reach argv, the
environment or paths must not contain NUL bytes.

### Working directory

`cwd` must be an absolute path; a relative value rejects the request. Without
one, the task runs in the invocation directory. Each task has its own
effective cwd, so one batch can work across several repositories or unrelated
checkouts.

The manifest is always discovered from the invocation directory, never from a
task's cwd, so the agent names available are the same for every task.

### Environment

Each child inherits `cue-agent`'s own process environment, overlaid by
`defaults.env` and then by the task's `env`, merged by variable name:

- a string value sets the variable;
- a `null` value removes it, including a variable inherited from the
  supervisor;
- `"env": null` on a task drops the whole `defaults.env` overlay, leaving
  that task with the unchanged inherited environment; a `null`
  `defaults.env` is simply no overlay.

Variable names must be nonempty and must not contain `=` or NUL.

```json
{
  "defaults": { "env": { "RUST_LOG": "info", "HTTP_PROXY": null } },
  "tasks": [
    { "agent": "explore", "prompt": "Map the supervision loop.",
      "env": { "RUST_LOG": "debug" } },
    { "agent": "explore", "prompt": "Map the trace writer.", "env": null }
  ]
}
```

The inherited environment and explicit overlay are not dumped into run
manifests, receipts, index lines or traces. Overlays remain in memory for
launch and version probing. This is not output redaction: a child that prints
an environment value can still expose it in captured output or metadata.

### Finding the harness

The harness is always `pi`, found by normal executable lookup on each task's
effective environment and working directory. There is no override flag or
variable.

- The effective `PATH` is the inherited value unless the task's overlay sets
  or removes it.
- Entries are taken as the child sees them after changing into its cwd: a
  relative entry resolves against the task cwd, and an empty entry (as in
  `a::b`, or an empty `PATH`) means the task cwd itself.
- The first candidate that is a regular file the current user may execute
  wins; the resolved path is recorded as `harness_path` in the run manifest.
- When the effective `PATH` is unset (the supervisor has none, or the task
  removes it with `"PATH": null`), lookup searches the platform's default
  search path (`confstr(_CS_PATH)`), as normal Unix executable lookup does.
  The child still runs without `PATH`; if the platform defines no default,
  lookup fails for that task.

A task whose `pi` cannot be found is not an admission error: it becomes a
failed run with an `error` explaining why, and the other tasks still run.

The version probe runs the same resolved executable with `--version`, in the
same cwd and environment, with a two-second timeout and bounded output
reading. Tasks share a probe only when executable, cwd and environment are
all identical. Probe failure is benign: the run simply records no version.

### Capture context

`context` selects where a task's trace is written. It can be set in
`defaults` and overridden or cleared per task, so tasks in one invocation may
capture into different contexts or none.

Every effective non-null context must be a canonical cue context address,
`<org>/<repo>/<context>`, naming a context that already exists in the cue
store (`$CUE_STORE`, else `~/cue`). Slugs, filesystem paths and artifact addresses are
rejected, as are nonexistent contexts, before anything is launched. A
cleared or omitted context disables trace capture only; local run records are
always written.

Capture context and `CUE_CONTEXT` are independent. `cue-agent` never sets or
clears `CUE_CONTEXT` on its own: the child inherits it from the supervisor
like any other variable, and a task controls it through `env`, for example
`"env": { "CUE_CONTEXT": "cue-agent-runtime-mvp" }` or
`"env": { "CUE_CONTEXT": null }`. Context selection is also independent of
the task's cwd.

The trace is written by running `cue -C <execution directory> add` with the
canonical destination address, so the destination may lie in any scope,
independently of the repository the task ran in. Before writing, and before
an ephemeral checkout is removed, `cue-agent` reads the execution directory's
repository scope (from its `origin`) and short `HEAD` revision and passes them
explicitly as `repo_id` and `commit_hash`. cue requires both for a trace
written into a scope other than the execution directory's own; when either
cannot be read (no repository, no `origin`, no commit), a cross-scope write is
refused and reported as `trace_error` rather than stamped with an invented
revision. In the same scope, cue stamps whatever was not supplied.

### Worktrees

A `worktree` object runs the task in a newly created checkout instead of in
its cwd. The task's effective cwd (or the invocation directory) is the target
that selects the source repository.

- `base` (required) — the revision the new branch starts from.
- `ephemeral` (required) — `true` for disposable work, `false` to retain it.
- `path` (optional) — the exact checkout destination; a relative path
  resolves against the target directory. Without it, a unique child of the
  manifest's `worktree_root` is generated. With neither, the request is
  rejected at admission.
- `branch` (optional) — the new branch name; without it a unique
  `cue-agent/<run-id>` name is generated.

Only new resources are created. A destination that already exists (or
appears while the task is being prepared), a destination already registered
as a worktree even if its directory is missing, or a branch name already
taken, fails that task without touching the existing resource; existing
worktrees are never adopted. So does a destination that would contain a
registered worktree, or that equals, contains or lies inside a checkout
created for an earlier task in the same batch (symlinked aliases included),
since removing one checkout would delete the other. Omit `worktree` (or clear an
inherited one with `"worktree": null`) and use `cwd` to run in an existing
checkout. The harness starts at the root of the new checkout, and `pi` is
looked up from there.

Tasks are prepared one at a time in specification order. Preparation failures
(not a Git repository, an unknown base, a collision, a failing `git worktree
add` or checkout hook) are per-task: that task fails with an `error`, whatever
it had already created (its branch, its checkout, directories made for the
destination) is removed, and the other tasks still run.

After the run and its trace capture, an ephemeral checkout and its branch are
deleted, including uncommitted and untracked changes. A persistent checkout
and branch are kept; the receipt's `worktree` object then gives whichever of
`path` and `branch` were generated, so the work can be found. A checkout whose
harness never started (interrupted before launch, `pi` not found, spawn
failure) holds no work and is removed even when persistent. Cleanup only ever
removes resources created for that task.

Cleanup failures are reported in `cleanup_errors`, each naming the surviving
checkout or branch and its repository, independently of the run's outcome,
and make the batch exit `1`. Local records stay outside checkouts: the run
manifest records the worktree request, generated identifiers and base commit
before launch, and both the run manifest (`worktree.head`) and the index line
record the checkout's final `HEAD` before removal. A failure to write either
is reported in `persistence_errors` and does not skip cleanup.

The branch is created with a create-only ref update whose reflog message is
unique to the run. Git commits a new ref before it runs the
`reference-transaction` `committed` hook, so a creation that fails or is
stopped after an interrupt may still have made the branch: it is removed when
its reflog begins with the run's message at the base commit, left alone when
it begins otherwise (another actor's branch), and reported in
`cleanup_errors` when it exists but cannot be attributed. Ref lock files left
by interrupted creation or deletion are reported by path and left untouched
because their ownership cannot be proved.

Git commands run in their own process groups with file-backed output, so an
interrupt reaching cue-agent's foreground job does not kill a checkout half
way through creation or removal. After an interrupt, a pending preparation
command gets the grace window before it is killed. Inspection and cleanup
commands are capped at 60 seconds, or at the grace window once an interrupt
has been observed, so a hook that never exits cannot hold finalization: the
command is killed and whatever it was removing is reported as surviving
(unless it is in fact gone), with the run's result kept. A hard crash (for
example SIGKILL) can leave worktrees and
branches behind; there is no recovery service. Directories that `git` itself
or a hook creates elsewhere are not tracked. A partial checkout left in the
task's own new directory by a failed or killed `git worktree add` is removed
when Git has no registration there; one registered on anything other than
the task's branch is kept and reported in `cleanup_errors`. Repository-local
Git variables inherited by `cue-agent` (`GIT_DIR`, `GIT_WORK_TREE`,
`GIT_INDEX_FILE`, `GIT_COMMON_DIR` and the rest of
`git rev-parse --local-env-vars`) are cleared for these commands, so only the
task's target selects the repository.

The existing trace-writing helper is not yet bounded. A stalled `cue add`
can still delay reaching worktree cleanup; bounding that helper is a separate
follow-up before interruption-safe finalization is complete end to end.

### Timeouts

The per-run deadline comes from the root `timeout` of the merged manifest
(built-in `0`, then global, then project), overridden by `--timeout`. `0`
means no deadline, recorded as `null`. The deadline applies independently to
each run, counted from its own harness launch; it is not a batch deadline,
and a run timing out does not cancel the others.

### Admission

The whole request is validated before any run state is created or any
harness is launched: JSON syntax, the envelope and every field, the four-task
cap, agent names, required prompts, file references, cwd, env names, capture
contexts and worktree requests. Any failure rejects the entire request with
exit `2`.

### The cap

Four tasks per invocation. The cap is per call, not per session: overlapping
calls may run more than four in total, which is accepted for this phase. A
request over the cap fails before anything is spawned, with a clear error.
There is no queue, so nothing is silently split, truncated or deferred.

### Exit codes

- `0` — every run completed, and every required trace, local record write
  and worktree cleanup succeeded.
- `1` — the batch ran but at least one run failed, timed out or was aborted
  (including a task whose `pi` could not be found or whose worktree could not
  be prepared), or a trace, receipt or index write or a worktree cleanup
  failed. A local run record (the batch or run directory, `prompt.md`,
  `system-prompt.md` or `manifest.json`) that cannot be written before launch
  fails only its own task, which is not launched and reports the reason in
  `error`; anything it created is cleaned up and the other tasks still run.
  When the state directory itself is unavailable, every task fails this way.
  The receipt is still printed. Execution outcome and
  storage outcome stay separate: a completed run whose trace failed keeps
  `outcome: "completed"` and reports `trace_error`; storage failures appear in
  `persistence_errors`. A failed receipt write can only be reported in
  stdout; an index error is also recorded in the receipt file when writable.
- `2` — the request was rejected before any run launched: bad arguments, an
  invalid specification, an unknown agent, an oversized batch, an unreadable
  manifest, an invalid capture context, an unresolvable worktree request. A
  diagnostic prefixed `cue-agent:` goes to stderr and nothing is printed on stdout, with or
  without `--json`.

### Interrupts and deadlines

The handler is installed once admission succeeds, before any run state or
worktree is created. An interrupt during preparation stops further
preparation and launches nothing; tasks skipped before launch are reported
`aborted`, while a preparation command stopped by the interrupt reports a
preparation failure. Their owned resources are cleaned up. SIGINT or SIGTERM
to `cue-agent` tears the whole
batch down: SIGTERM to each
child's process group, a five-second grace window, then SIGKILL. A run past its
deadline is torn down the same way. Both record how the child actually died
alongside the reason teardown started, because they are distinct facts.
An interrupt during an existing timeout teardown does not restart the grace
window or replace the timeout reason. Descendants that detach using
`setsid`/`setpgid` escape the group and may outlive the run; process groups are
not a containment or sandbox boundary.

## Where runs are recorded

Two planes. The trace artifact is curation; the run directory is preservation,
so a missing trace is never data loss.

### Plane 1: the trace artifact

Written only when the task has an effective capture context and the agent
produced a final message. Its body is that message verbatim, with nothing
added, so the trace and the response extracted from `events.jsonl` are
byte-identical. All text blocks of the final assistant message are
concatenated in order, without adding separators or including thinking/tool
blocks.

```
<org>/<repo>/<context>/trace/agent/<batch-label-slug>-<batch-id>/<position>-<agent>.md
```

Every task of a batch shares one directory in whichever context it captures
to. Its prefix is the slugged batch label, or `batch` when there is no batch
label or the label has no letters or digits; task labels never choose the
directory. `<position>` is the task's one-based position in the original
`tasks` array, zero-padded to three digits, so repeated agents stay apart and
names follow specification order rather than completion order. For example,
the second task of an unlabelled batch running `explore` writes
`agent/batch-20260918-120301-3f9a2b/002-explore.md`.

Frontmatter carries `kind: agent-run`, the agent, model, harness and harness
version, `description` (the task's effective label: its own, else the batch
label), `batch_label` when the batch has one, the outcome, exit code and
duration, usage (`turns`, `tokens_input`, `tokens_output`, `cost_usd`) and
`run_id`, `batch_id` and `run_path` as forward pointers into plane 2.
`repo_id` and `commit_hash` describe the repository and revision the run
executed, as read from its execution directory (see "Capture context"). The
prompt is deliberately not frontmatter: prompts are long and multi-line, and
`prompt.md` in the run directory holds it verbatim.

### Plane 2: the run directory

Machine-local operational exhaust, unsynced and unbacked-up, written whether or
not a context exists:

```
$XDG_STATE_HOME/cue/agent/runs/<YYYY-MM-DD>/<batch-id>/<agent>-<n>/
  manifest.json      the request: agent, model, harness path and version,
                     argv, cwd, capture context, labels, repo, commit,
                      store root, timeout, launch error (no environment dump);
                      a launched worktree's final HEAD is added before removal
  prompt.md          the prompt verbatim, as passed to the harness
  system-prompt.md   the agent's effective system prompt at run time
  events.jsonl       raw harness stdout
  stderr.log         raw harness stderr
  receipt.json       outcome, exit disposition, usage, timings
```

`<n>` is the task's one-based position in the specification, so repeated
agents get distinct directories. `manifest.json`, `prompt.md` and
`system-prompt.md` are written before the harness starts, so an instant crash
still leaves a record. Agent definitions are edited over time and may be
overridden per task, which is why the effective system prompt is captured per
run: a past run cannot be interpreted without it.

The agent state root is restricted to mode `0700`; run files and the index
use `0600`. Existing cue-owned target permissions are tightened when opened.
This does not make prompt transport private: prompts also appear in child
command-line arguments and can be inspected where OS process permissions allow.

The event parsing cap is a read-side limit, **not a disk quota**. Output files
may grow throughout execution, including from escaped descendants. Live output
quotas and stronger process containment are deferred; use appropriate host
disk limits and deadlines for unattended workloads.

One line per run is appended to `$XDG_STATE_HOME/cue/agent/index.jsonl`
(agent, model, capture context, cwd, outcome, exit code, duration, cost and
run path), so "what has run" is one pass over one file. It is rebuildable from
the manifests.

## The receipt

`cue-agent run` prints one result when the batch finishes. There is no
streaming protocol in this phase; the run files exist from day one, so
tailing them or emitting a merged event stream both stay open as later
additions.

With `--json` it is one JSON object. Runs appear in specification order, one
per task, whatever order they finished in. There is no batch-level context:
tasks may capture into different contexts, and each run's `trace` address
names its own destination.

```json
{
  "batch_id": "20260918-120301-3f9a2b",
  "batch_path": "/home/you/.local/state/cue/agent/runs/2026-09-18/20260918-120301-3f9a2b",
  "cap": 4,
  "harness": "pi",
  "harness_version": "pi 1.2.3",
  "runs": [
    {
      "run_id": "20260918-120301-3f9a2b-explore-1",
      "batch_id": "20260918-120301-3f9a2b",
      "agent": "explore",
      "model": "anthropic/claude-haiku-4",
      "outcome": "completed",
      "exit_code": 0,
      "signal": null,
      "duration_ms": 8412,
      "turns": 3,
      "tokens_input": 18422,
      "tokens_output": 1204,
      "cost_usd": 0.0412,
      "response": "...the agent's final message...",
      "error": null,
      "stderr_excerpt": null,
      "events_malformed": 0,
      "events_oversized": 0,
      "events_truncated": false,
      "run_path": ".../20260918-120301-3f9a2b/explore-1",
      "trace": "acme/widgets/cue-agent-runtime-mvp/trace/agent/review-the-branch-diff-20260918-120301-3f9a2b/001-explore.md",
      "trace_error": null,
      "persistence_errors": [],
      "cleanup_errors": []
    }
  ]
}
```

The batch-level `harness_version` is set only when every probed task reported
the same version; otherwise it is `null`, and each run's own version is in its
manifest and trace.

Without `--json`, the same information is printed as a summary: a
`batch <id>` header, then for each run in specification order a line such as
`[1] explore (completed, 8412 ms)` (with exit status or signal when relevant),
any `error`, stderr excerpt, `trace`, `trace error`, `persistence error`,
`cleanup error`, `retained worktree` and `retained branch` lines, the `record`
path, and finally the response.

A run with a retained worktree whose path or branch was generated also has a
`worktree` object holding only the generated `path` and/or `branch`; it is
absent otherwise. Failures are never reduced
to the outcome word alone.

`outcome` is one of `completed`, `failed`, `timeout`, `aborted`. Failures are
isolated: one failing run never stops or taints the others, and a harness that
cannot be found or will not start is a failed run with a receipt, not a usage
error.

## Environment

These variables configure `cue-agent` itself. Unless a task's `env` overlay
says otherwise, children also inherit them along with the rest of the
supervisor's environment.

- `PATH` — where `pi` is found, per task, as described in "Finding the
  harness".
- `CUE_AGENT_CUE_BIN` — the `cue` binary used to write traces; defaults to
  `cue` on `PATH`.
- `CUE_STORE` — the cue store capture contexts must exist in; also recorded
  in each run manifest, so state outside the store knows which store it
  describes.
- `XDG_STATE_HOME` — root of the run record; defaults to `~/.local/state`.
- `XDG_CONFIG_HOME` — root of the global manifest; defaults to `~/.config`.
- `CUE_AGENT_GRACE_MS` — SIGTERM-to-SIGKILL grace window in milliseconds,
  default 5000. Mainly for tests.

`CUE_CONTEXT` has no special meaning to `cue-agent`: it is neither read as a
capture context nor set for children; it is inherited, or set or removed
through `env`, like any other variable.

## Deliberately absent

No `Harness` trait, no streaming protocol, no queue, no session-wide admission
tracking, no dry-run, no harness session persistence (`--no-session` is
always passed) and no cue-review integration. Each is recorded in the design
notes, not forgotten.
