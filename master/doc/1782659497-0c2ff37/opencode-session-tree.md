---
title: Opencode Session Tree
---
# opencode Session Tree — Research Report

**Research question**: How does opencode organize its session tree? What objects are part of the tree (user messages, agent messages, tool calls, etc.), how are they connected, and are there parent-child relationships linking main-agent sessions to subagent sessions?

**Source base**: All paths below are relative to `ref/opencode/` (absolute: `/home/pl/.config/opencode/ref/opencode/`). Findings reflect the current git state at research time. No implementation suggestions are included.

---

## Executive Summary

opencode models a conversation as a **Session**: a self-contained record that owns a durable event log and a set of projected read-model rows. The "session tree" has two distinct meanings that this report separates:

1. **The intra-session tree** — the message/part hierarchy *inside one session* (Session -> Message -> Part, with tool calls and results as Part variants).
2. **The inter-session tree** — a parent/child relationship between sessions, used to connect a main-agent session to its **subagent** sessions. Subagents are not nested messages; each subagent invocation is a **full Session** in its own right, linked back to the parent by a single self-referential column.

The architecture is **event-sourced**: a durable append-only log (`SessionMessage`) is the source of truth, and the more familiar `Message`/`Part` rows are **projected read-models** derived from it.

---

## 1. Object Inventory

### 1.1 Core persisted objects (Drizzle tables)

All defined in `packages/core/src/session/sql.ts` unless noted.

| Object | Table | Role | Source |
|---|---|---|---|
| **Session** | `SessionTable` | Root of a conversation; holds title, cost/tokens, model, agent, permissions, and the `parent_id` link. | `packages/core/src/session/sql.ts:21-65` |
| **SessionMessage** | `SessionMessageTable` | Durable append-only event log — the source of truth. | `packages/core/src/session/sql.ts:118-137` |
| **Message** | `MessageTable` | Projected read-model: a role-based container (user/assistant). | `packages/core/src/session/sql.ts:67-79` |
| **Part** | `PartTable` | Projected read-model: granular content of a Message (text, reasoning, tool, etc.). | `packages/core/src/session/sql.ts:81-97` |
| **SessionInput** | `SessionInputTable` | Advisory inbox holding prompts before they are promoted into the log. | `packages/core/src/session/sql.ts:139-165` |
| **SessionContextEpoch** | `SessionContextEpochTable` | Per-session context-window state. | `packages/core/src/session/sql.ts` (enumerated by schema scan) |
| **Todo** | `TodoTable` | Session-scoped task list. | `packages/core/src/session/sql.ts` (enumerated by schema scan) |

Verbatim `SessionTable` definition (the inter-session link lives here):

```ts
// packages/core/src/session/sql.ts:21-65
export const SessionTable = sqliteTable(
  "session",
  {
    id: text().$type<SessionSchema.ID>().primaryKey(),
    project_id: text().$type<ProjectV2.ID>().notNull().references(() => ProjectTable.id, { onDelete: "cascade" }),
    workspace_id: text().$type<WorkspaceV2.ID>(),
    parent_id: text().$type<SessionSchema.ID>(),
    slug: text().notNull(),
    directory: DatabasePath.directoryColumn().notNull(),
    path: DatabasePath.pathColumn(),
    title: text().notNull(),
    version: text().notNull(),
    share_url: text(),
    summary_additions: integer(),
    summary_deletions: integer(),
    summary_files: integer(),
    summary_diffs: text({ mode: "json" }).$type<Snapshot.FileDiff[]>(),
    metadata: text({ mode: "json" }).$type<Record<string, unknown>>(),
    cost: real().notNull().default(0),
    tokens_input: integer().notNull().default(0),
    tokens_output: integer().notNull().default(0),
    tokens_reasoning: integer().notNull().default(0),
    tokens_cache_read: integer().notNull().default(0),
    tokens_cache_write: integer().notNull().default(0),
    revert: text({ mode: "json" }).$type<{ messageID: MessageID; partID?: PartID; snapshot?: string; diff?: string }>(),
    permission: text({ mode: "json" }).$type<PermissionV1.Ruleset>(),
    agent: text(),
    model: text({ mode: "json" }).$type<{ id: string; providerID: string; variant?: string }>(),
    ...Timestamps,
    time_compacting: integer(),
    time_archived: integer(),
  },
  (table) => [index("session_parent_idx").on(table.parent_id)],
)
```

### 1.2 SessionMessage (log) variants

The durable log's `type` column is a discriminated union. Enumerated literal values:

> `agent-switched`, `model-switched`, `user`, `synthetic`, `system`, `shell`, `assistant`, `compaction`
> — `packages/core/src/session/message.ts:179-188`

So a single session_message row can represent a user prompt, an assistant turn, a system event, a shell interaction, a model/agent switch, a synthetic message, or a compaction marker.

### 1.3 Tool call / result storage

Inside the durable log, a tool call is represented by `Session.Message.AssistantTool`, which embeds both the invocation and its result:

```ts
// packages/core/src/session/message.ts:107-123
export class AssistantTool extends Schema.Class<AssistantTool>("Session.Message.Assistant.Tool")({
  type: Schema.Literal("tool"),
  id: Schema.String,
  name: Schema.String,
  state: ToolState,
  time: Schema.Struct({
    created: V2Schema.DateTimeUtcFromMillis,
    ran: V2Schema.DateTimeUtcFromMillis.pipe(Schema.optional),
    completed: V2Schema.DateTimeUtcFromMillis.pipe(Schema.optional),
  }),
}) {}
```

- **Tool name** → `AssistantTool.name` (`message.ts:110`)
- **Tool input/args** → `AssistantTool.state.input` — a string when pending, a `Record` once running/completed (`message.ts:74, 79, 86, 95`)
- **Tool result** → embedded in `AssistantTool.state.result` (`message.ts:90, 99`); there is no separate "tool result" object in the durable log.

In the projected read-model, the same information surfaces as a `ToolPart`:

> `ToolPart` stores `tool` (name), `callID` (unique), and a `state` union holding `input`, `output` (result string), and `result` object — `packages/core/src/v1/session.ts:306-316`

### 1.4 Projected Part variants

The V1 projected `Part` discriminated union has these literal `type` values:

> `text`, `subtask`, `reasoning`, `file`, `tool`, `step-start`, `step-finish`, `snapshot`, `patch`, `agent`, `retry`, `compaction`
> — `packages/core/src/v1/session.ts:356-369`

> Note on schema generations: there are two parallel type systems — the newer `SessionMessage` ADTs in `packages/core/src/session/message.ts` (durable log) and the V1 projected types in `packages/core/src/v1/session.ts`. Both are live. This is consistent with an in-progress migration; treat the `SessionMessage` log as authoritative and the V1 types as the current projection target.

### 1.5 Role distinction (user vs assistant)

- **`MessageTable`**: distinguished by a `role` field inside the JSON `data` column. The union is annotated as a discriminator:
  > `Schema.Union([User, Assistant]).annotate({ discriminator: "role", identifier: "Message" })` — `packages/core/src/v1/session.ts:488`
- **`SessionMessageTable`**: distinguished by the `type` column (`user` or `assistant` literals) — `message.ts:41, 145`.

---

## 2. Connection Model (how objects are linked)

```
Session (SessionTable)          <-- inter-session tree root
  |
  |  parent_id  (self-ref)      <-- links a child session to its parent
  |
  +-- session_id --> SessionMessage (log)         [uniqueIndex(session_id, seq)]
  +-- session_id --> Message (projected)          [index(session_id, time_created, id)]
  |                    |
  |                    +-- message_id --> Part (projected)   [index(message_id, id)]
  |                                      (session_id denormalized on Part for session-wide queries)
  +-- session_id --> SessionInput (inbox)
  +-- session_id --> SessionContextEpoch
  +-- session_id --> Todo
```

Verbatim link definitions and their supporting indexes:

| Link | Field | Index | Source |
|---|---|---|---|
| Session -> child Session | `parent_id` | `index("session_parent_idx").on(table.parent_id)` | `packages/core/src/session/sql.ts:30, 63` |
| Session -> SessionMessage | `session_id` (FK cascade) | `uniqueIndex("session_message_session_seq_idx").on(table.session_id, table.seq)` | `packages/core/src/session/sql.ts:122, 132` |
| Session -> Message | `session_id` (FK cascade) | `index("message_session_time_created_id_idx")` | `packages/core/src/session/sql.ts:70, 78` |
| Message -> Part | `message_id` (FK cascade) | `index("part_message_id_id_idx")` | `packages/core/src/session/sql.ts:85, 94` |
| Session -> Part | `session_id` (denormalized) | — | `packages/core/src/session/sql.ts:88` |
| Session -> SessionInput | `session_id` (FK cascade) | — | `packages/core/src/session/sql.ts:145` |

The `(session_id, seq)` unique index on `session_message` is what enforces ordering and dedup within a single session's log (`sql.ts:132`).

The projector (`packages/core/src/session/projector.ts`) is responsible for transforming the durable `SessionMessage` log into the projected `Message`/`Part` rows. `insertMessage` writes a new `SessionMessage` using `event.seq` (`projector.ts:211-227`); the `run` function delegates to `SessionMessageUpdater.update` (`projector.ts:112`), which applies immer-based patches to incrementally update an active assistant message (e.g. appending text deltas) — see `message-updater.ts:207-425`.

---

## 3. Parent-Child (Subagent) Linking — the inter-session tree

This is the heart of question 3. Subagents are spawned by the **Task tool**. Each invocation creates a brand-new Session that is structurally linked to the parent via a single column, with an advisory runtime link on the parent side.

### 3.1 Spawn flow (end to end)

When the main agent calls the Task tool, `TaskTool.execute`:

1. Resolves the subagent type and model from the tool parameters.
2. Creates a child session with the current session as parent:

```ts
// packages/opencode/src/tool/task.ts:129-145
const nextSession =
  session ??
  (yield* sessions.create({
    parentID: ctx.sessionID,
    title: params.description + ` (@${next.name} subagent)`,
    agent: next.name,
    permission: [
      ...deriveSubagentSessionPermission({
        parentSessionPermission: parent.permission ?? [],
        parentAgent,
        subagent: next,
      }),
      ...(cfg.experimental?.primary_tools?.map((item) => ({
        pattern: "*",
        action: "allow" as const,
        permission: item,
      })) ?? []),
    ],
  }))
```

3. Registers runtime metadata on the parent's current turn (this is the parent-side pointer — advisory, not a DB column):

```ts
// packages/opencode/src/tool/task.ts:158-168
const metadata = {
  parentSessionId: ctx.sessionID,
  sessionId: nextSession.id,
  model,
  ...(runInBackground ? { background: true } : {}),
}

yield* ctx.metadata({
  title: params.description,
  metadata,
})
```

4. Runs the subagent by admitting a prompt into the child session via `ops.prompt`:

```ts
// packages/opencode/src/tool/task.ts:173-192
const runTask = Effect.fn("TaskTool.runTask")(function* () {
  const parts = yield* ops.resolvePromptParts(params.prompt)
  const result = yield* ops.prompt({
    messageID: MessageID.ascending(),
    sessionID: nextSession.id,
    model: { modelID: model.modelID, providerID: model.providerID },
    variant: next.model ? undefined : variant,
    agent: next.name,
    tools: { ... },
    parts,
  })
  return result.parts.findLast((item) => item.type === "text")?.text ?? ""
})
```

5. (Foreground path) Waits for the child session to finish and returns the wrapped result:

```ts
// packages/opencode/src/tool/task.ts:301-312
const result = yield* Effect.raceFirst(
  background.wait({ id: nextSession.id }).pipe(Effect.map((waited) => waited.info)),
  background.waitForPromotion(nextSession.id),
)
// ...
return {
  title: params.description,
  metadata,
  output: renderOutput({ sessionID: nextSession.id, state: "completed", text: result?.output ?? "" }),
}
```

### 3.2 Field-by-field link table (inter-session)

| Source | Field | Target | Type | Source |
|---|---|---|---|---|
| `session` row | `parent_id` | parent `session` row | **Structural** (the only structural link) | `packages/opencode/src/session/session.ts:121`; column def `packages/core/src/session/sql.ts:30` |
| `Session.Info` | `parentID` | parent `Session.Info` | in-memory mirror of above | `packages/opencode/src/session/session.ts:220` |
| Parent Part metadata | `sessionId` | child `Session` | Advisory (UI/traversal) | `packages/opencode/src/tool/task.ts:160` |
| Parent Part metadata | `parentSessionId` | parent `Session` | Advisory (redundant) | `packages/opencode/src/tool/task.ts:159` |
| Parent tool-result text | `<task id="...">` | child `Session` | In-text reference (parseable) | `packages/opencode/src/tool/task.ts:71` |

### 3.3 Result flow back to the parent (lossy by design)

The subagent's full message/part tree is **not** transferred into the parent. Only the **final text** of the child's last text part is captured (`task.ts:191`), and it is wrapped in a structured tag before being returned as the parent tool call's output:

```ts
// packages/opencode/src/tool/task.ts:63-78
function renderOutput(input: {
  sessionID: SessionID
  state: "running" | "completed" | "error"
  summary?: string
  text: string
}) {
  const tag = input.state === "error" ? "task_error" : "task_result"
  return [
    `<task id="${input.sessionID}" state="${input.state}">`,
    ...(input.summary ? [`<summary>${input.summary}</summary>`] : []),
    `<${tag}>`,
    input.text,
    `</${tag}>`,
    "</task>",
  ].join("\n")
}
```

So from the parent's perspective, a subagent invocation looks like one tool call whose result is a text blob of the form `<task id="<childSessionID>" state="completed"><task_result>...</task_result></task>`. The result is **not** streamed token-by-token; it is delivered whole when the child session finishes.

### 3.4 Can a parent be recovered from a child? Can the spawning tool call be found?

- **Parent from child**: Yes, structurally. Read the child `session` row's `parent_id` (`packages/opencode/src/session/session.ts:121`).
- **Specific spawning tool call from child**: Not via a structural FK. The parent does not store the child session ID in a dedicated column. Recovery requires querying the parent session's `part` rows and matching on the JSON metadata where `sessionId === <child_id>` (metadata is persisted as JSON in the part's `data`). The `<task id="...">` text in the tool result is a secondary, parseable reference.

In other words, the inter-session link is **directional and structurally thin**: child → parent is a clean FK lookup; parent → specific child tool call relies on advisory metadata.

### 3.5 Concurrency

Multiple subagent sessions can be spawned from one parent. Each Task-tool invocation creates its own `session` row sharing the same `parent_id`. Foreground tasks block the parent turn (the tool result is returned when the child completes); background tasks (`background: true`) let the parent continue and trigger more tasks concurrently, coordinated by a `BackgroundJob` service (`packages/opencode/src/tool/task.ts:251`).

### 3.6 Subagent isolation

The child session's permission ruleset is derived from the parent's via `deriveSubagentSessionPermission`, which inherits parent denies plus `external_directory` rules and conditionally denies `todowrite`/`task` depending on the subagent's capabilities:

```ts
// packages/opencode/src/agent/subagent-permissions.ts:27-34
return [
  ...parentAgentDenies,
  ...input.parentSessionPermission.filter(
    (rule) => rule.permission === "external_directory" || rule.action === "deny",
  ),
  ...(canTodo ? [] : [{ permission: "todowrite" as const, pattern: "*" as const, action: "deny" as const }]),
  ...(canTask ? [] : [{ permission: "task" as const, pattern: "*" as const, action: "deny" as const }]),
]
```

The child session title follows the pattern `"<description> (@<agentName> subagent)"` (`task.ts:132`).

---

## 4. Confidence and Gaps

- **High confidence**: Schema/table definitions, message/part ADT variants, the `parent_id` link, the Task-tool spawn/result flow, and the permission derivation. All backed by verbatim snippets from the named files.
- **Medium confidence**: The `SessionContextEpoch` and `SessionInput` promotion lifecycle (admitted → promoted) was enumerated but not fully traced; the promotion logic lives in `SessionInput.projectPromoted` at `packages/core/src/session/projector.ts:421` and involves the `SessionRunner`.
- **Schema-generation note**: Two parallel type systems coexist (`SessionMessage` durable ADTs in `packages/core/src/session/message.ts` vs V1 projected types in `packages/core/src/v1/session.ts`). Both are live; treat the durable log as authoritative.
- **Not verified**: A dedicated "session tree" TUI/API view. The gap subagent noted `session-request-tree.ts` uses `parentID` for UI hierarchy reconstruction, but the exact rendering path was not traced end to end.
- **Part variant `subtask`**: the V1 projected Part union includes a `subtask` type (`v1/session.ts:356-369`) distinct from the `tool` type used by Task-tool calls. Its relationship to the subagent mechanism (if any) was not investigated; it may be unrelated (e.g. a nested-step marker). Flagging as an open question.

