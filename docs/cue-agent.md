# cue-agent

`cue-agent` runs named coding agents as child harness processes and reports
what they produced. One invocation is one batch: up to four named agents run
concurrently, each in its own process group, and the command prints a single
JSON receipt when the last of them finishes.

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

1. `$XDG_CONFIG_HOME/cue/agents.json` (falling back to
   `~/.config/cue/agents.json`)
2. `.cue-agent.json`, the nearest one at or above the working directory,
   stopping at the repository root

`agents` is an object keyed by agent name, never an array: a merge replaces an
array wholesale, so an array would let a project file wipe every global agent
instead of overriding one field of one agent.

```json
{
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
      "system_prompt_file": "prompts/consultant.md",
      "timeout_secs": 900
    },
    "diff-reviewer-flash": {
      "description": "Reviews a diff and lists defects",
      "model": "google/gemini-flash",
      "system_prompt": "You review diffs. List defects, most severe first."
    }
  }
}
```

Fields, all optional except the agent's own key:

- `description` — what the agent is for; this is what a calling model reads.
- `model` — passed as `--model`; omit to inherit the harness default.
- `system_prompt` / `system_prompt_file` — one or the other, never both. A
  file path is resolved against the manifest that declared it. A project
  prompt replaces the global prompt even when it uses the other form. Setting
  both non-null forms in one manifest layer is an error.
- `tools` — passed as a comma-separated `--tools` list.
- `thinking` — passed as `--thinking`.
- `timeout_secs` — default deadline for runs of this agent.

A project file overrides one field without restating the agent:

```json
{ "agents": { "explore": { "model": "anthropic/claude-sonnet-4" } } }
```

Inspect the result:

```
cue-agent agents list
cue-agent agents list --json
```

## Running agents

One prompt, one agent:

```
cue-agent run explore --prompt "Where is session admission decided?"
```

One prompt fanned out to several agents, which is the "Request reviews from
Flash and Opus" case:

```
cue-agent run diff-reviewer-flash consultant-opus \
  --prompt "$(git diff main...HEAD)" \
  --context cue-agent-runtime-mvp \
  --label "Review the branch diff"
```

Different prompts per agent, via a batch file (or `-` for stdin):

```json
{
  "label": "Split investigation",
  "context": "cue-agent-runtime-mvp",
  "runs": [
    { "agent": "explore", "prompt": "Map the supervision loop." },
    { "agent": "consultant-opus", "prompt": "Critique the teardown order.",
      "timeout_secs": 600 }
  ]
}
```

```
cue-agent run --batch batch.json
cue-agent run --batch - < batch.json
```

A bare JSON array of runs is accepted as shorthand for `{"runs": [...]}`.

Prompts must be nonempty and must not begin with `-` or `@`: Pi parses those
prefixes as options or file references, not verbatim text. These checks apply
to the entire batch before launch. Prepend ordinary instruction text when
passing such content. Pi does not support a `--` separator workaround.

### Options

- `--prompt <TEXT>` / `--prompt-file <PATH>` — the prompt; `-` reads stdin.
- `--batch <PATH>` — per-agent runs as JSON; `-` reads stdin.
- `--context <SLUG>` — the caller's cue context; see below.
- `--label <TEXT>` — short description, recorded on the trace and used in its
  filename.
- `--timeout <SECS>` — deadline for every run in the batch; a batch entry's
  `timeout_secs` wins, then this flag, then the agent's default. Zero disables
  the deadline; the effective unlimited setting is recorded as JSON `null`.
- `--cwd <PATH>` — working directory for the harness.
- `--harness <PATH>` — the harness executable. Relative paths containing `/`
  are resolved against the invocation directory, not `--cwd`; bare names use
  `PATH`. The version probe uses the same program and working directory, with
  a two-second timeout and bounded result reading. Probe failure is benign.

### The cap

Four runs per invocation. The cap is per call, not per session: overlapping
calls may run more than four in total, which is accepted for this phase. A
batch over the cap fails before anything is spawned, with a clear error. There
is no queue, so nothing is silently split, truncated or deferred.

### Exit codes

- `0` — every run completed and its receipt/index persistence succeeded.
- `1` — the batch ran but at least one run failed, timed out or was aborted;
  or a receipt/index write failed. The available receipt is still printed.
  Per-run `persistence_errors` records storage failures without changing the
  agent's execution outcome. A failed receipt write can only be reported in
  stdout; an index error is also recorded in the receipt file when writable.
- `2` — the request never reached the harness: bad arguments, an unknown
  agent, an oversized batch, an unreadable manifest. Nothing is printed on
  stdout.

### Interrupts and deadlines

SIGINT or SIGTERM to `cue-agent` tears the whole batch down: SIGTERM to each
child's process group, a five-second grace window, then SIGKILL. A run past its
deadline is torn down the same way. Both record how the child actually died
alongside the reason teardown started, because they are distinct facts.
An interrupt during an existing timeout teardown does not restart the grace
window or replace the timeout reason. Descendants that detach using
`setsid`/`setpgid` escape the group and may outlive the run; process groups are
not a containment or sandbox boundary.

## The context parameter

`--context` is optional and does double duty: it selects where the trace
artifact is written, and it is exported to the child as `CUE_CONTEXT` so a
subagent's own cue writes land in the caller's context. When no context is
given, `$CUE_CONTEXT` is inherited if set; when neither is present, the
variable is *removed* from the child environment and no trace is written. A
child with no context fails its cue writes loudly rather than guessing, which
is cue's stated rule working as designed.

## Where runs are recorded

Two planes. The trace artifact is curation; the run directory is preservation,
so a missing trace is never data loss.

### Plane 1: the trace artifact

Written only when a context was supplied and the agent produced a final
message. Its body is that message verbatim, with nothing added, so the trace
and the response extracted from `events.jsonl` are byte-identical.
All text blocks of the final assistant message are concatenated in order,
without adding separators or including thinking/tool blocks.

```
<context>/trace/agent/<label-slug>-<agent>-<short-run-id>.md
```

Frontmatter carries `kind: agent-run`, the agent, model, harness and harness
version, the caller's `description`, the outcome, exit code and duration, usage
(`turns`, `tokens_input`, `tokens_output`, `cost_usd`) and `run_id`, `batch_id`
and `run_path` as forward pointers into plane 2. cue stamps `repo_id` and
`commit_hash`. The prompt is deliberately not frontmatter: prompts are long and
multi-line, and `prompt.md` in the run directory holds it verbatim.

### Plane 2: the run directory

Machine-local operational exhaust, unsynced and unbacked-up, written whether or
not a context exists:

```
$XDG_STATE_HOME/cue/agent/runs/<YYYY-MM-DD>/<batch-id>/<agent>-<n>/
  manifest.json      the request: agent, model, argv, cwd, repo, commit,
                     context, store root
  prompt.md          the prompt verbatim, as passed to the harness
  system-prompt.md   the agent's system prompt as it was at run time
  events.jsonl       raw harness stdout
  stderr.log         raw harness stderr
  receipt.json       outcome, exit disposition, usage, timings
```

`manifest.json`, `prompt.md` and `system-prompt.md` are written before the
harness starts, so an instant crash still leaves a record. Agent definitions
are edited over time, which is why the system prompt is captured per run: a
past run cannot be interpreted without it.

The agent state root is restricted to mode `0700`; run files and the index
use `0600`. Existing cue-owned target permissions are tightened when opened.
This does not make prompt transport private: prompts also appear in child
command-line arguments and can be inspected where OS process permissions allow.

The event parsing cap is a read-side limit, **not a disk quota**. Output files
may grow throughout execution, including from escaped descendants. Live output
quotas and stronger process containment are deferred; use appropriate host
disk limits and deadlines for unattended workloads.

One line per run is appended to `$XDG_STATE_HOME/cue/agent/index.jsonl`, so
"what has run" is one pass over one file. It is rebuildable from the manifests.

## The receipt

`cue-agent run` prints one JSON object. There is no streaming protocol in this
phase; the run files exist from day one, so tailing them or emitting a merged
event stream both stay open as later additions.

```json
{
  "batch_id": "20260918-120301-3f9a2b",
  "batch_path": "/home/you/.local/state/cue/agent/runs/2026-09-18/20260918-120301-3f9a2b",
  "cap": 4,
  "context": "cue-agent-runtime-mvp",
  "harness": "pi",
  "harness_version": "pi 1.2.3",
  "runs": [
    {
      "run_id": "20260918-120301-3f9a2b-explore-1",
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
      "trace": "acme/widgets/cue-agent-runtime-mvp/trace/agent/review-explore-4f1a9c02.md",
      "trace_error": null
    }
  ]
}
```

`outcome` is one of `completed`, `failed`, `timeout`, `aborted`. Failures are
isolated: one failing run never stops or taints the others, and a harness that
will not start is a failed run with a receipt, not a usage error.

## Environment

- `CUE_AGENT_HARNESS` — harness executable; `--harness` wins, `pi` is the
  default.
- `CUE_AGENT_CUE_BIN` — the `cue` binary used to write traces; defaults to
  `cue` on `PATH`.
- `CUE_CONTEXT` — inherited as the context when `--context` is absent.
- `CUE_STORE` — recorded in each run manifest, so state outside the store
  knows which store it describes.
- `XDG_STATE_HOME` — root of the run record; defaults to `~/.local/state`.
- `XDG_CONFIG_HOME` — root of the global manifest; defaults to `~/.config`.
- `CUE_AGENT_GRACE_MS` — SIGTERM-to-SIGKILL grace window in milliseconds,
  default 5000. Mainly for tests.

## Deliberately absent

No `Harness` trait, no streaming protocol, no queue, no session-wide admission
tracking, no harness session persistence (`--no-session` is always passed) and
no cue-review integration. Each is deferred with a reason recorded in the
design notes, not forgotten.
