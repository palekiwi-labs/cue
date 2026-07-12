---
status: complete
priority: high
refs:
- .cue/feat-curator-activity-item/plan/1782659497-0c2ff37/slice6b-rendering.md
- .cue/feat-curator-activity-item/spec/index.md
---
# Slice 6b Manual QA Checklist

Manual live verification of the rendering rewrite from
`plan/1782659497-0c2ff37/slice6b-rendering.md` (step 7). The automated tests
(60 curator tests, workspace green, clippy clean) pass — these steps verify
the **live pipeline** (opencode plugin -> acuity server -> curator TUI)
renders the six properties the slice promised.

## Setup

```bash
# Terminal 1 -- acuity server (with file logging for triage)
ACUITY_LOG_FILE=.cue/feat-curator-activity-item/tmp/acuity-phase-6a/acuity.log \
ACUITY_PORT=33222 \
ACUITY_DATA_DIR=.cue/feat-curator-activity-item/tmp/acuity-phase-6a \
cargo run -p acuity

# Terminal 2 -- curator TUI (connects to acuity via SSE)
ACUITY_URL=http://localhost:33222 cargo run -p curator
#   navigate to the Activity view

# Terminal 3 -- trigger an opencode session that spawns a sub-agent
opencode
#   prompt: "Use the Task tool to explore the repo structure and summarize it."
#   (a Task sub-agent gives you 2+ sessions sharing a per-run id prefix)
```

## Verification checklist

- [ ] **(a) Prefix-sharing sessions render distinct headers.**
  The main session and the sub-agent session share an id prefix (e.g.
  `ses_0f14...`), but their headers should now show **distinct id-suffixes**
  (last ~8 chars) until titles arrive. Confirm the two headers are visibly
  different, not identical. (Pins the `ui.rs:214` 8-char-front-truncation fix.)

- [ ] **(b) Header flips dim-id to bright-title within seconds.**
  Watch a freshly-created session's header. It starts dim (DarkGray id-suffix
  placeholder). Within a few seconds of the first prompt, opencode regenerates
  the title and fires `session_updated` -- the header should flip to
  bright/bold (Cyan) showing the real title. This flip is the live proof that
  the 6a title-capture round-trips to the UI.

- [ ] **(c) `session_updated` rows are gone from the feed.**
  Scroll through the activity list. There should be **no** `session_updated`
  event rows visible -- their payload is absorbed into `SessionSummary`
  before render, so they're hidden by `is_hidden_in_activity`. (Confirm
  against the log file: `session_updated` lines ARE still arriving in
  `acuity.log`, they just don't render.)

- [ ] **(d) `j`/`k` never makes the highlight vanish or jitter (the drift test).**
  With a populated feed (ideally with some hidden `session_updated` rows in
  the stream), press `j` to scroll down through the entire list and `k` back
  up. The selection highlight must stay on a visible row at all times -- it
  must **never** disappear, skip, or land on a blank line. This is the
  keystone fix (selection indexes the filtered set, clamped at
  `activity_len()-1`).

- [ ] **(e) Turn rows show the per-turn model.**
  Each `agent_turn_completed` turn row should display ` - {model}` appended
  (e.g. ` - anthropic/claude-sonnet`). If the session uses a sub-agent with a
  different model (e.g. `google/gemini-3-flash-preview`), adjacent turn rows
  should show **different** models. Confirm the separator appears cleanly with
  no dangling ` - ` when model is absent.

- [ ] **(f) Block title shows harness/project once.**
  The activity feed block title should read ` Activity Feed - opencode / cue `
  (harness + project basename). It appears **once** in the block border, not
  repeated per session header.

## Triage (if something looks wrong)

The acuity log file at
`.cue/feat-curator-activity-item/tmp/acuity-phase-6a/acuity.log` has
per-variant INFO lines + DEBUG raw-vs-parsed lines. Cross-reference what the
curator shows against what the log says arrived:

```bash
# Confirm session_updated events are arriving (even though hidden in TUI)
grep "session_updated" .cue/feat-curator-activity-item/tmp/acuity-phase-6a/acuity.log

# Confirm model is populated on turn events
grep "agent_turn_completed" .cue/feat-curator-activity-item/tmp/acuity-phase-6a/acuity.log
```

## Done criteria

All six boxes checked. Then:
- `plan/1782659497-0c2ff37/slice6b-rendering.md` step 7 -> `[x]`, plan status
  -> `complete`.
