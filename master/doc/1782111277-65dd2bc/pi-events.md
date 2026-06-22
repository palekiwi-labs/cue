---
title: Pi Events
---
# Pi Extension Lifecycle Event Hooks — Complete Catalog & Schemas

**Research question:** What lifecycle event hooks does Pi expose that extensions can match on, and what is the data schema accessible inside each hook?

**Ground truth source:** All definitions live in a single file:
`/home/pl/.pi/ref/pi/packages/coding-agent/src/core/extensions/types.ts`
Documentation cross-reference: `/home/pl/.pi/ref/pi/packages/coding-agent/docs/extensions.md`
Worked examples: `/home/pl/.pi/ref/pi/packages/coding-agent/examples/extensions/plan-mode/index.ts`

---

## 1. The Matching Contract

Extensions match on events by calling `pi.on(eventName, handler)` inside their default-export factory. The handler signature is defined once and reused for every event:

```ts
// packages/coding-agent/src/core/extensions/types.ts:1088
export type ExtensionHandler<E, R = undefined> = (event: E, ctx: ExtensionContext) => Promise<R | void> | R | void;
```

The second type parameter `R` determines whether the handler is **observe-only** (`R = undefined` -> return ignored) or **controlling** (`R = SomeResult` -> return value can mutate, replace, or cancel). Every event is registered through a dedicated overload of `ExtensionAPI.on` (verbatim at `types.ts:1098-1135`), so the match-string and the payload type are type-checked.

There are **29 lifecycle event hooks** in total, split into two behavioral categories:

| Category | Count | Handler return | Effect |
| :--- | :---: | :--- | :--- |
| **Observe-only** | 16 | `void` | Receive the event for logging/telemetry/UI; cannot change behavior. |
| **Controlling** | 13 | A `*Result` object | Can cancel, replace, transform, or block the operation. |

All 29 events are members of the `ExtensionEvent` union (`types.ts:959-981`).

---

## 2. Full Catalog

> In every schema below, the `type` field is a string-literal constant used as the discriminant. Source file for every interface is `/home/pl/.pi/ref/pi/packages/coding-agent/src/core/extensions/types.ts`.

### A. Resource Discovery

#### `resources_discover` — CONTROLLING
Fires after `session_start` so an extension can contribute additional resource paths. Schema `types.ts:502-506`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"resources_discover"` | no | constant |
| `cwd` | `string` | no | current working directory |
| `reason` | `"startup" \| "reload"` | no | why discovery ran |

Return — `ResourcesDiscoverResult` (near `types.ts:508-512`; explorer's line range was garbled, content verified):
```ts
export interface ResourcesDiscoverResult {
	skillPaths?: string[];
	promptPaths?: string[];
	themePaths?: string[];
}
```

---

### B. Session Lifecycle

#### `session_start` — OBSERVE
Fired when a session is started, loaded, or reloaded. Schema `types.ts:520-526`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"session_start"` | no | constant |
| `reason` | `"startup" \| "reload" \| "new" \| "resume" \| "fork"` | no | triggering cause |
| `previousSessionFile` | `string` | yes | path to prior session if applicable |

#### `session_before_switch` — CONTROLLING (cancellable)
Fired before switching to another session (`/new`, `/resume`). Schema `types.ts:529-533`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"session_before_switch"` | no | constant |
| `reason` | `"new" \| "resume"` | no | triggering cause |
| `targetSessionFile` | `string` | yes | target session file path |

Return — `SessionBeforeSwitchResult` (`types.ts:1024-1026`):
```ts
export interface SessionBeforeSwitchResult { cancel?: boolean; }
```

#### `session_before_fork` — CONTROLLING (cancellable)
Fired before `/fork` or `/clone`. Schema `types.ts:536-540`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"session_before_fork"` | no | constant |
| `entryId` | `string` | no | entry ID to fork from |
| `position` | `"before" \| "at"` | no | fork point relative to entry |

Return — `SessionBeforeForkResult` (`types.ts:1028-1031`):
```ts
export interface SessionBeforeForkResult {
	cancel?: boolean;
	skipConversationRestore?: boolean;
}
```

#### `session_before_compact` — CONTROLLING (cancellable / customizable)
Fired before context compaction. Schema `types.ts:543-549`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"session_before_compact"` | no | constant |
| `preparation` | `CompactionPreparation` | no | internal compaction state |
| `branchEntries` | `SessionEntry[]` | no | entries being compacted |
| `customInstructions` | `string` | yes | custom prompt for the summarizer |
| `signal` | `AbortSignal` | no | to detect cancellation |

Return — `SessionBeforeCompactResult` (`types.ts:1033-1036`):
```ts
export interface SessionBeforeCompactResult {
	cancel?: boolean;
	compaction?: CompactionResult;
}
```

#### `session_compact` — OBSERVE
Fired after context compaction. Schema `types.ts:552-556`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"session_compact"` | no | constant |
| `compactionEntry` | `CompactionEntry` | no | the resulting summary entry |
| `fromExtension` | `boolean` | no | whether an extension triggered it |

#### `session_shutdown` — OBSERVE
Fired before the extension runtime is torn down (quit / reload / session replacement). Schema `types.ts:559-564`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"session_shutdown"` | no | constant |
| `reason` | `"quit" \| "reload" \| "new" \| "resume" \| "fork"` | no | reason for teardown |
| `targetSessionFile` | `string` | yes | path to new session if applicable |

#### `session_before_tree` — CONTROLLING (cancellable / customizable)
Fired before navigating in the session tree (`/tree`). Schema `types.ts:582-586`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"session_before_tree"` | no | constant |
| `preparation` | `TreePreparation` | no | target/source/summary info |
| `signal` | `AbortSignal` | no | abort detection |

Return — `SessionBeforeTreeResult` (`types.ts:1038-1050`):
```ts
export interface SessionBeforeTreeResult {
	cancel?: boolean;
	summary?: { summary: string; details?: unknown; };
	/** Override custom instructions for summarization */
	customInstructions?: string;
	/** Override whether customInstructions replaces the default prompt */
	replaceInstructions?: boolean;
	/** Override label to attach to the branch summary entry */
	label?: string;
}
```

#### `session_tree` — OBSERVE
Fired after navigating in the session tree. Schema `types.ts:589-595`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"session_tree"` | no | constant |
| `newLeafId` | `string \| null` | no | destination leaf |
| `oldLeafId` | `string \| null` | no | previous leaf |
| `summaryEntry` | `BranchSummaryEntry` | yes | resulting summary if one was created |
| `fromExtension` | `boolean` | yes | if triggered by an extension |

---

### C. Provider Request / Context (the LLM call)

#### `context` — CONTROLLING (can replace messages)
Fired before each LLM call. Schema `types.ts:612-615`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"context"` | no | constant |
| `messages` | `AgentMessage[]` | no | current conversation history |

Return — `ContextEventResult` (`types.ts:987-989`):
```ts
export interface ContextEventResult { messages?: AgentMessage[]; }
```

#### `before_provider_request` — CONTROLLING (can replace payload)
Fired before the provider request is sent. Schema `types.ts:618-621`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"before_provider_request"` | no | constant |
| `payload` | `unknown` | no | raw API request object |

Return — `BeforeProviderRequestEventResult` (`types.ts:991`):
```ts
export type BeforeProviderRequestEventResult = unknown;
```
(The returned value replaces the payload sent to the provider.)

#### `after_provider_response` — OBSERVE
Fired after a provider response is received, before the stream is consumed. Schema `types.ts:624-628`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"after_provider_response"` | no | constant |
| `status` | `number` | no | HTTP status code |
| `headers` | `Record<string, string>` | no | response headers |

---

### D. Agent Loop / Turn / Message

#### `before_agent_start` — CONTROLLING (inject message / replace system prompt)
Fired after the user submits a prompt, before the agent loop. Schema `types.ts:631-641`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"before_agent_start"` | no | constant |
| `prompt` | `string` | no | user text after expansion |
| `images` | `ImageContent[]` | yes | attached images |
| `systemPrompt` | `string` | no | final assembled system prompt |
| `systemPromptOptions` | `BuildSystemPromptOptions` | no | config used to build the prompt |

Return — `BeforeAgentStartEventResult` (`types.ts:1018-1022`). If multiple extensions return `systemPrompt`, the values are **chained**:
```ts
export interface BeforeAgentStartEventResult {
	message?: Pick<CustomMessage, "customType" | "content" | "display" | "details">;
	/** Replace the system prompt for this turn. If multiple extensions return this, they are chained. */
	systemPrompt?: string;
}
```

#### `agent_start` — OBSERVE
Fired when an agent loop starts. Schema `types.ts:644-646`: `{ type: "agent_start" }`.

#### `agent_end` — OBSERVE
Fired when an agent loop ends. Schema `types.ts:649-652`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"agent_end"` | no | constant |
| `messages` | `AgentMessage[]` | no | messages generated in the loop |

#### `turn_start` — OBSERVE
Fired at the start of each turn. Schema `types.ts:655-659`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"turn_start"` | no | constant |
| `turnIndex` | `number` | no | turn count |
| `timestamp` | `number` | no | start time |

#### `turn_end` — OBSERVE
Fired at the end of each turn. Schema `types.ts:662-667`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"turn_end"` | no | constant |
| `turnIndex` | `number` | no | turn count |
| `message` | `AgentMessage` | no | final message of the turn |
| `toolResults` | `ToolResultMessage[]` | no | results of tools run in the turn |

#### `message_start` — OBSERVE
Fired when any message (user / assistant / toolResult) starts. Schema `types.ts:670-673`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"message_start"` | no | constant |
| `message` | `AgentMessage` | no | the message object |

#### `message_update` — OBSERVE
Fired during assistant streaming with token-by-token updates. Schema `types.ts:676-680`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"message_update"` | no | constant |
| `message` | `AgentMessage` | no | updated message object |
| `assistantMessageEvent` | `AssistantMessageEvent` | no | the raw delta/token event |

#### `message_end` — CONTROLLING (can replace finalized message)
Fired when a message ends. Schema `types.ts:683-686`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"message_end"` | no | constant |
| `message` | `AgentMessage` | no | finalized message |

Return — `MessageEndEventResult` (`types.ts:1013-1016`); replacement must keep the original role:
```ts
export interface MessageEndEventResult {
	/** Replace the finalized message. The replacement must keep the original message role. */
	message?: AgentMessage;
}
```

---

### E. Tool Execution (observation — for TUI/telemetry)

These three fire around low-level tool execution and are observe-only.

#### `tool_execution_start` — OBSERVE  (`types.ts:689-694`)
| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"tool_execution_start"` | no | constant |
| `toolCallId` | `string` | no | unique call ID |
| `toolName` | `string` | no | tool name |
| `args` | `any` | no | tool arguments |

#### `tool_execution_update` — OBSERVE  (`types.ts:697-703`)
| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"tool_execution_update"` | no | constant |
| `toolCallId` | `string` | no | unique call ID |
| `toolName` | `string` | no | tool name |
| `args` | `any` | no | tool arguments |
| `partialResult` | `any` | no | current partial output |

#### `tool_execution_end` — OBSERVE  (`types.ts:706-712`)
| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"tool_execution_end"` | no | constant |
| `toolCallId` | `string` | no | unique call ID |
| `toolName` | `string` | no | tool name |
| `result` | `any` | no | final tool result |
| `isError` | `boolean` | no | success/failure flag |

> **Note:** `tool_execution_*` are for observing rendering/streaming. The high-level interception points for *modifying* tool behavior are `tool_call` and `tool_result` (Section F). In parallel mode, `tool_call` and `tool_result` events may interleave.

---

### F. Tool Interception (control)

`tool_call` and `tool_result` are **discriminated unions** keyed on `toolName`, with per-tool input shapes. There are 8 members each (`types.ts:831-839` and `types.ts:890-898`): `bash`, `read`, `edit`, `write`, `grep`, `find`, `ls`, and a `Custom*` catch-all.

Common base — `ToolCallEventBase` (`types.ts:780-783`):
```ts
interface ToolCallEventBase {
	type: "tool_call";
	toolCallId: string;
}
```
Each concrete member adds `toolName: "<name>"` (string literal) and `input: <Tool>ToolInput`.

#### `tool_call` — CONTROLLING (can block; can mutate `input` in place)
Verbatim JSDoc (`types.ts:826-830`):
> Fired before a tool executes. Can block.
> event.input is mutable. Mutate it in place to patch tool arguments before execution.
> Later tool_call handlers see earlier mutations. No re-validation is performed after mutation.

Return — `ToolCallEventResult` (`types.ts:993-997`):
```ts
export interface ToolCallEventResult {
	/** Block tool execution. To modify arguments, mutate `event.input` in place instead. */
	block?: boolean;
	reason?: string;
}
```

Common base — `ToolResultEventBase` (`types.ts:841-847`):
```ts
interface ToolResultEventBase {
	type: "tool_result";
	toolCallId: string;
	input: Record<string, unknown>;
	content: (TextContent | ImageContent)[];
	isError: boolean;
}
```
Each concrete member additionally carries a `details` field (e.g. `BashToolDetails`, `ReadToolDetails`, ...).

#### `tool_result` — CONTROLLING (can modify result)
Fired after a tool executes.

Return — `ToolResultEventResult` (`types.ts:1007-1011`):
```ts
export interface ToolResultEventResult {
	content?: (TextContent | ImageContent)[];
	details?: unknown;
	isError?: boolean;
}
```

---

### G. Model & Thinking Selection (observation)

#### `model_select` — OBSERVE  (`types.ts:721-726`)
| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"model_select"` | no | constant |
| `model` | `Model<any>` | no | the new model |
| `previousModel` | `Model<any> \| undefined` | no | the old model |
| `source` | `ModelSelectSource` | no | `set \| cycle \| restore` |

#### `thinking_level_select` — OBSERVE  (`types.ts:729-733`)
| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"thinking_level_select"` | no | constant |
| `level` | `ThinkingLevel` | no | the new level |
| `previousLevel` | `ThinkingLevel` | no | the old level |

---

### H. User Bash

#### `user_bash` — CONTROLLING (custom operations or full replacement)
Fired when the user runs a bash command via `!` or `!!`. Schema `types.ts:740-748`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"user_bash"` | no | constant |
| `command` | `string` | no | the shell command string |
| `excludeFromContext` | `boolean` | no | true when `!!` is used |
| `cwd` | `string` | no | current working directory |

Return — `UserBashEventResult` (`types.ts:1000-1005`):
```ts
export interface UserBashEventResult {
	/** Custom operations to use for execution */
	operations?: BashOperations;
	/** Full replacement: extension handled execution, use this result */
	result?: BashResult;
}
```

---

### I. Raw User Input

#### `input` — CONTROLLING (transform or consume)
Fired when user input is received, before agent processing. Schema `types.ts:758-768`:

| Field | Type | Opt | Description |
| :--- | :--- | :---: | :--- |
| `type` | `"input"` | no | constant |
| `text` | `string` | no | the input text |
| `images` | `ImageContent[]` | yes | attached images |
| `source` | `InputSource` | no | `interactive \| rpc \| extension` |
| `streamingBehavior` | `"steer" \| "followUp"` | yes | how it is delivered if streaming |

Return — discriminated union `InputEventResult` (`types.ts:771-774`):
```ts
export type InputEventResult =
	| { action: "continue" }
	| { action: "transform"; text: string; images?: ImageContent[] }
	| { action: "handled" };
```
(`transform` replaces the input; `handled` consumes it entirely so no agent processing occurs.)

---

## 3. Quick Reference: All 29 Match Strings

Grouped by category. `*` = controlling (return value matters); `(obs)` = observe-only.

**Session/Resource:** `resources_discover` *, `session_start` (obs), `session_before_switch` *, `session_before_fork` *, `session_before_compact` *, `session_compact` (obs), `session_shutdown` (obs), `session_before_tree` *, `session_tree` (obs)

**Provider/Context:** `context` *, `before_provider_request` *, `after_provider_response` (obs)

**Agent/Turn/Message:** `before_agent_start` *, `agent_start` (obs), `agent_end` (obs), `turn_start` (obs), `turn_end` (obs), `message_start` (obs), `message_update` (obs), `message_end` *

**Tool:** `tool_execution_start` (obs), `tool_execution_update` (obs), `tool_execution_end` (obs), `tool_call` *, `tool_result` *

**Selection:** `model_select` (obs), `thinking_level_select` (obs)

**User:** `user_bash` *, `input` *

---

## 4. Reused / External Types (not fully expanded)

These appear in payloads but are defined elsewhere; they are large nested types and were not expanded in this report:

- **`AgentMessage`** — the core message type, used by `context`, `agent_end`, `message_*`, `before_agent_start`. Imported from `@earendil-works/pi-agent-core` (`types.ts:12`).
- **`Model<T>`** — LLM model descriptor (incl. `contextWindow`, `thinkingLevelMap`). Imported from `@earendil-works/pi-ai` (`types.ts:24`).
- **`ImageContent` / `TextContent`** — message content blocks.
- **`ExtensionContext`** — second arg to every handler; exposes `sessionManager` (a `ReadonlySessionManager` from `../session-manager.ts:55`) for read-only session state.
- **`ToolDefinition`** — for *registering* tools via `pi.registerTool`, not a lifecycle hook (`types.ts:433-480`). The `subagent` example uses this rather than lifecycle hooks.
- Per-tool input/detail shapes: `BashToolInput`, `ReadToolInput`, `EditToolInput`, `WriteToolInput`, `GrepToolInput`, `FindToolInput`, `LsToolInput`, and corresponding `*ToolDetails`.

---

## 5. Confidence Notes & Gaps

- **High confidence:** All 29 match strings, their observe-vs-controlling classification, and the verbatim `*Result` return types were extracted directly from `types.ts`. The field tables were cross-checked between two independent explorer passes.
- **Line-number caveat:** The explorer's reported line range for `ResourcesDiscoverResult` was garbled; the interface content is verified correct (`skillPaths` / `promptPaths` / `themePaths`) but its exact line range should be re-confirmed if cited precisely. It sits near the `ResourcesDiscoverEvent` block (~508-512).
- **Chaining semantics:** `before_agent_start.systemPrompt` is documented as "chained" when multiple extensions return it. The exact chaining mechanism for other events (and the extension load order that determines handler execution order — global vs local vs `settings.json`) was not traced into the `Loader`; `extensions.md:114` implies an ordering but a definitive priority list requires a follow-up drill into the loader/dispatcher.
- **Parallel execution:** Both docs and source note that in parallel mode `tool_call` and `tool_result` may interleave — extension authors must not assume strict ordering.
- **`registerTool` is out of scope here:** It is the tool-authoring API, not a lifecycle hook. Flagged only to avoid confusion.
- **No single `BaseEvent`:** Events do not share a common base interface; each is standalone and unified only through the `ExtensionEvent` union (`types.ts:959-981`).
