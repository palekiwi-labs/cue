---
title: Opencode Model Capture
---
# Capturing Session Model/Agent from an opencode Plugin — Research Report

**Research question**: Why does a plugin see `agent` but NOT `model` on `session.created`/`session.updated` event payloads (with `model` always `undefined`), while the SDK `Session` type declares neither? Where is the model actually defined, how is it resolved, and what is the reliable way for a plugin to capture the model a session actually used?

**Source base**: All paths below are relative to `ref/opencode/` (absolute: `/home/pl/.config/opencode/ref/opencode/`). Findings reflect the current git state. No implementation suggestions beyond stating API-surface facts.

---

## TL;DR — The Reconciliation

The observed behavior has **two independent causes**, and fixing one does not fix the other:

1. **SDK type lag (the type gap).** The user's installed SDK (`node_modules/@opencode-ai/sdk/dist/gen/types.gen.d.ts`, ~line 465) is an **older build**. The *current* source SDK declares `agent` and `model` on `Session`. The `dist/gen/` layout vs the current `src/v2/gen/` layout confirms a version mismatch. Upgrading the SDK makes the fields type-visible (no cast needed).

2. **Runtime value is genuinely `undefined` (the data gap — NOT fixed by upgrading).** The session event's `info.model` is set from `input.model` at construction (`packages/opencode/src/session/session.ts:566`). Sessions created without an explicit model — **notably subagent sessions spawned by the Task tool** (`packages/opencode/src/tool/task.ts:129-145`, which calls `sessions.create({ parentID, title, agent, permission })` with **no model**) — have `model: undefined`. The model is resolved **per turn** at run time and is only reliably visible via turn-level events, not the session record.

**Therefore**: to capture the model a session *actually used*, subscribe to `session.next.step.started` (best) or read `modelID`/`providerID` from `message.updated` assistant messages. Reading `info.model` from session events / `session.get` will miss subagent sessions.

---

## Q1 — Exact runtime shape of `session.created` / `session.updated` event `info`

**Answer**: The event's `properties.info` is a locally-constructed `Session.Info` object (alias of `SessionV1.SessionInfo`), NOT the raw DB row, but a projection that carries the same field set. It DOES include both `agent` and `model` keys. `model` is `undefined` whenever the session was created without an explicit model.

Publish sites:

```ts
// packages/opencode/src/session/session.ts:577
yield* events.publish(SessionV1.Event.Created, { sessionID: result.id, info: result })

// packages/opencode/src/session/session.ts:788
yield* events.publish(SessionV1.Event.Updated, { sessionID, info: next })
```

The `info` object is built in `createNext` (and re-derived in `fromRow`). The construction shows `model` is sourced directly from the create input — so it is `undefined` when no model was passed:

```ts
// packages/opencode/src/session/session.ts:554-574
const result: Info = {
  id: SessionID.descending(input.id),
  slug: Slug.create(),
  version: InstallationVersion,
  projectID: ctx.project.id,
  directory: input.directory,
  path: input.path,
  workspaceID: input.workspaceID,
  parentID: input.parentID,
  title: input.title ?? (input.parentID ? childTitlePrefix : parentTitlePrefix) + new Date().toISOString(),
  agent: input.agent,
  model: input.model,            // <-- undefined when caller omits model
  metadata: input.metadata,
  permission: input.permission ? [...input.permission] : undefined,
  cost: 0,
  tokens: EmptyTokens,
  time: { created: Date.now(), updated: Date.now() },
}
```

Full field set delivered in `info`: `id, slug, version, projectID, directory, path?, workspaceID?, parentID?, title, agent?, model?, metadata?, permission?, cost, tokens, time, summary?, share?, revert?`. The `model` key (when present) is structured as `{ id, providerID, variant? }` — there is **no** alternate key name (`modelID`/`provider`/nested under `config`/`metadata`) on the session object. When `model` is undefined it is simply absent.

---

## Q2 — Where `model` is defined internally + the SDK codegen gap

**Answer**: `model` is defined in three places internally, and the SDK codegen gap is a **version lag**, not a deliberate strip.

Internal definitions:

```ts
// packages/core/src/session/schema.ts:25-49  (V2 domain type)
export class Info extends Schema.Class<Info>("SessionV2.Info")({
  id: ID,
  agent: AgentV2.ID.pipe(Schema.optional),
  model: ModelV2.Ref.pipe(Schema.optional),   // <-- present, optional
  ...
})
```

The DB column is a JSON object (`packages/core/src/session/sql.ts:21-65`):

```ts
model: text({ mode: "json" }).$type<{
  id: string
  providerID: string
  variant?: string
}>(),
```

The SDK in the **current source** includes both fields (this is the authoritative reference — the user's installed copy is behind):

```ts
// packages/sdk/js/src/v2/gen/types.gen.ts:168-219
export type Session = {
  id: string
  slug: string
  projectID: string
  ...
  title: string
  agent?: string
  model?: {
    id: string
    providerID: string
    variant?: string
  }
  version: string
  ...
}
```

**Codegen-gap explanation**: The SDK `Session` type is generated from the route response schema `Session.Info` (the `GET /session/{id}` and event payloads use the same schema). The current generator produces a type that matches the internal `Info`. The user's installed SDK omits `agent`/`model` because it is an older build (their path `dist/gen/types.gen.d.ts` differs from the current `src/v2/gen/types.gen.ts`). Upgrading `@opencode-ai/sdk` to match the running opencode reconciles the type.

---

## Q3 — How opencode resolves a model when `session.model` is null

**Answer**: Model resolution happens at the start of every provider turn via `SessionRunnerModel.resolve`. The chain consults the session column, then the catalog default, then any available supported model — it does **not** inherit the parent session's model.

```ts
// packages/core/src/session/runner/model.ts:128-138
resolve: Effect.fn("SessionRunnerModel.resolve")(function* (session) {
  // Location plugins populate and filter the catalog asynchronously during layer startup.
  yield* boot.wait()
  const preferred = yield* catalog.model.default()
  const selected = session.model
    ? yield* catalog.model.get(session.model.providerID, session.model.id)
    : (Option.getOrUndefined(preferred.pipe(Option.filter(supported))) ??
      (yield* catalog.model.available()).find(supported))
  if (!selected) return yield* new ModelNotSelectedError({ sessionID: session.id })
  return yield* resolve(session, selected, yield* catalog.provider.get(selected.providerID))
}),
```

**Resolution chain (verified):**
1. `session.model` DB column → if set, use it.
2. `catalog.model.default()` → the user's global preferred model (the top-level `model` field in `opencode.json`).
3. `catalog.model.available().find(supported)` → first supported model in the catalog.

**Why a subagent session still runs on a specific model**: a Task-tool subagent session satisfies `session.model == null`, so it falls to step 2/3 (the global default / first supported model). It does **not** inherit the parent's model through this code path. (Note: the parent session's model is also typically `undefined` at creation and resolved the same way, so "inheritance" would be moot in most cases.)

The resolution result is what populates the model in turn-level events:

```ts
// packages/core/src/session/runner/llm.ts:147-152
const agent = yield* agents.select(session.agent)
const model = yield* models.resolve(session)

// packages/core/src/session/runner/llm.ts:178-196
const publisher = createLLMEventPublisher(events, {
  sessionID: session.id,
  agent: agent.id,
  model: {
    id: ModelV2.ID.make(model.id),
    providerID: ProviderV2.ID.make(model.provider),
    ...(session.model?.variant === undefined ? {} : { variant: session.model.variant }),
  },
})
```

> **Confidence note (gap)**: The verified `models.resolve` chain (model.ts:128-138) consults only `session.model` + catalog. The agent-config `model` field (see Q5) exists on the `Agent.Info` type, but whether/how it is written back into `session.model` before resolution was not fully traced. For native agents ("general"/"plan") the agent config has no model, so this distinction does not affect them. Treat the turn-level events as the source of truth for the actually-used model regardless.

---

## Q4 — Does `client.session.get({ path: { id } })` return a richer object?

**Answer**: **No.** It returns the same `Session.Info` projection produced by `fromRow`, identical in shape to the event payload. It carries `model` only when the DB column is populated — so for subagent sessions it remains `undefined`.

Route + handler:

```ts
// packages/opencode/src/server/routes/instance/httpapi/groups/session.ts:132-143
HttpApiEndpoint.get("get", SessionPaths.get, {
  params: { sessionID: SessionID },
  query: WorkspaceRoutingQuery,
  success: described(Session.Info, "Get session"),
  error: [HttpApiError.BadRequest, ApiNotFoundError],
})

// packages/opencode/src/server/routes/instance/httpapi/handlers/session.ts:83-85
const get = Effect.fn("SessionHttpApi.get")(function* (ctx: { params: { sessionID: SessionID } }) {
  return yield* requireSession(ctx.params.sessionID)
})
```

Conclusion: `session.get` cannot be relied upon to recover the model for sessions where the event payload lacks it. Do not repurpose the existing `session.get` call to obtain the model.

---

## Q5 — Agent config and the agent→model relationship

**Answer**: Agents "general" and "plan" are **native** built-in agents and define **no model** — they rely entirely on the runtime resolution chain. User-defined agents may specify a `model` string in `opencode.json`, parsed into `{ providerID, modelID }`. There is an HTTP `GET /agent` route (so `client.agent.list()` is available), but it returns the configured model only — native agents come back with `model: undefined`.

Native agent definitions (no `model` field):

```ts
// packages/opencode/src/agent/agent.ts:143-179
plan: {
  name: "plan",
  description: "Plan mode. Disallows all edit tools.",
  options: {},
  permission: Permission.merge(defaults, Permission.fromConfig({ /* ... */ }), user),
  mode: "primary",
  native: true,
},
general: {
  name: "general",
  description: `General-purpose agent for researching complex questions and executing multi-step tasks. Use this agent to execute multiple units of work in parallel.`,
  permission: Permission.merge(defaults, Permission.fromConfig({ todowrite: "deny" }), user),
  options: {},
  mode: "subagent",
  native: true,
},
```

Agent `Info` type (the `model` field is optional and structured):

```ts
// packages/opencode/src/agent/agent.ts:30-51
export const Info = Schema.Struct({
  name: Schema.String,
  // ...
  model: Schema.optional(
    Schema.Struct({
      modelID: ModelV2.ID,
      providerID: ProviderV2.ID,
    }),
  ),
  // ...
}).annotate({ identifier: "Agent" })
```

User-config agents take a string model, parsed at load:

```ts
// packages/core/src/v1/config/agent.ts:12-14
const AgentSchema = Schema.StructWithRest(
  Schema.Struct({
    model: Schema.optional(Schema.String),
// ...

// packages/opencode/src/agent/agent.ts:265
if (value.model) item.model = Provider.parseModel(value.model)
```

HTTP route for listing agents:

```ts
// packages/opencode/src/server/routes/instance/httpapi/groups/instance.ts:149-158
HttpApiEndpoint.get("agent", InstancePaths.agent, {
  query: WorkspaceRoutingQuery,
  success: described(Schema.Array(Agent.Info), "List of agents"),
}).annotateMerge(
  OpenApi.annotations({
    identifier: "app.agents",
    summary: "List agents",
    description: "Get a list of all available AI agents in the OpenCode system.",
  }),
)
```

**Implication**: mapping `agent` name → model via `GET /agent` only works for explicitly-configured (user) agents. For native agents (which is what "plan"/"general" are), the API returns no model — the effective model is the runtime-resolved one (Q3), observable only via turn-level events (Q6).

---

## Q6 — Better events for capturing the resolved model

**Answer**: **Yes.** The reliable source is `session.next.step.started`, which fires at the start of every provider turn carrying the **resolved** model. `message.updated` is a secondary source (the assistant message carries `modelID`/`providerID`).

`session.next.step.started` carries the resolved model at `properties.model`:

```ts
// packages/sdk/js/src/v2/gen/types.gen.ts:914-927
| {
    type: "session.next.step.started"
    properties: {
      sessionID: string
      assistantMessageID: string
      agent: string
      model: {
        id: string
        providerID: string
        variant?: string
      }
    }
  }
```

This `model` is the output of the `models.resolve` chain (Q3) wired through the publisher (`llm.ts:178-196`), so it reflects the model actually used for that turn — including the catalog-fallback case for subagent sessions.

`message.updated` carries the model on the message object (shape differs by role). The assistant message schema:

```ts
// packages/core/src/v1/session.ts:451-463
export const Assistant = Schema.Struct({
  // ...
  modelID: ModelV2.ID,
  providerID: ProviderV2.ID,
  agent: Schema.String,
  // ...
})
```

- User message: `info.model.modelID` / `info.model.providerID` / `info.model.variant?`
- Assistant message: `info.modelID` / `info.providerID` (flat fields) plus `info.agent`.

> **SDK-version caveat**: whether these events are exposed in the user's *installed* SDK depends on its version. They are present in the current source SDK; if absent in the installed build, upgrading `@opencode-ai/sdk` exposes them.

---

## Plugin Access Cheat-Sheet

Access expressions a plugin author would use (facts about the API surface):

| Goal | Source | Access expression | Reliable for subagent sessions? |
|---|---|---|---|
| Resolved model per turn (BEST) | `session.next.step.started` event | `event.properties.model` (`{ id, providerID, variant? }`) | Yes |
| Model from assistant message | `message.updated` event (role=assistant) | `event.properties.info.modelID` + `event.properties.info.providerID` | Yes |
| Model from user message | `message.updated` event (role=user) | `event.properties.info.model.modelID` | Yes |
| Agent name | `session.created`/`updated` or `session.next.step.started` | `event.properties.info.agent` / `event.properties.agent` | Yes |
| Session-row model (unreliable) | `session.created`/`updated` event | `event.properties.info.model` | **No** — undefined for subagents |
| Session-row model via fetch | `client.session.get(...)` | `(await client.session.get({ path: { id } })).model` | **No** — same undefined limitation |
| Agent-configured model (user agents only) | `client.agent.list()` then match by name | `agents.find(a => a.name === agentName)?.model` | **No** — undefined for native "plan"/"general" |

For sessions where `session.next.step.started` is not observed (e.g. a session that never produced a turn), the model is genuinely unresolvable from session/agent records — it would only have existed as a runtime catalog fallback.

---

## Confidence and Gaps

- **High confidence**: Session event `info` shape and construction (`session.ts:554-574, 577, 788`); current SDK `Session` type includes `agent`/`model` (`types.gen.ts:168-219`); `session.get` returns the same projection (`session.ts` route/handler); `models.resolve` chain (`model.ts:128-138`); native "plan"/"general" have no model (`agent.ts:143-179`); `session.next.step.started` carries resolved model (`types.gen.ts:914-927`, wired via `llm.ts:178-196`).
- **The user's installed SDK version** was inferred (not directly read from their `node_modules`) to be older than the in-repo source, based on the `dist/gen/` vs `src/v2/gen/` layout difference and the field-set discrepancy. Confirm by checking the installed `@opencode-ai/sdk` version against the running opencode version.
- **Gap**: Whether/where a user-configured agent's `model` field (parsed at `agent.ts:265`) is written back into `session.model` before `models.resolve` runs was not fully traced. The verified resolution chain (`model.ts:128-138`) does not reference agent config. For native agents this is moot; for user-configured agents with an explicit model, the persistence path is an open question.
- **Gap**: `catalog.model.default()` internal logic (what populates the global preferred model beyond the `opencode.json` top-level `model` field) was not traced.
- **Gap**: Exact SDK event-subscription method names (`client.event.*` / `client.subscribe.*`) and whether `session.next.step.started` is reachable in the user's installed SDK version — verify after upgrading.

