---
title: Opencode Events
---
# OpenCode Plugin Lifecycle Hooks and Data Schemas

This report provides a comprehensive list of lifecycle event hooks and data schemas exposed by OpenCode for plugin development, as discovered in the `ref/opencode` source code.

## 1. Plugin Hook Interface

Plugins in OpenCode are TypeScript/JavaScript modules that default export a function. This function returns an object implementing the `Hooks` interface.

**Source Path:** `/home/pl/.config/opencode/ref/opencode/packages/plugin/src/index.ts:222-335`

### Functional Hooks
These hooks allow direct modification of inputs and outputs for core OpenCode operations.

| Hook Name | Description |
| :--- | :--- |
| `dispose` | Called when the plugin is being unloaded. |
| `event` | Generic listener for all system events (see Section 2). |
| `config` | Called with the merged configuration object. |
| `tool` | Register custom tools that OpenCode can invoke. |
| `auth` | Hook into authentication flows. |
| `provider` | Hook into LLM provider logic. |
| `chat.message` | Triggered when a new message is received. Allows modification of the message and its parts. |
| `chat.params` | Modify parameters (temperature, topP, etc.) sent to the LLM. |
| `chat.headers` | Add custom HTTP headers to outgoing LLM requests. |
| `permission.ask` | Intercept and auto-reply to permission requests. |
| `command.execute.before` | Intercept slash commands before execution. |
| `tool.execute.before` | Intercept tool calls (like `bash`, `read`) before they run. |
| `tool.execute.after` | Intercept and modify the output/metadata of a tool call. |
| `shell.env` | Inject environment variables into shell executions (AI tools and user terminals). |
| `tool.definition` | Modify the description and parameters of tools as seen by the LLM. |

### Experimental Hooks
These hooks are subject to change but provide deep access to internal processes.

| Hook Name | Description |
| :--- | :--- |
| `experimental.chat.messages.transform` | Transform the entire message history before it is sent to the LLM. |
| `experimental.chat.system.transform` | Modify the system prompt(s) for a session. |
| `experimental.session.compacting` | Inject custom context or replace the prompt during session compaction. |
| `experimental.compaction.autocontinue` | Enable/disable the auto-continue message after compaction. |
| `experimental.text.complete` | Intercept text completion requests. |

---

## 2. Event Schemas (Generic `event` Hook)

The `event` hook receives an `input: { event: Event }` object. The `Event` type is a discriminated union of various event types.

**Source Path:** `/home/pl/.config/opencode/ref/opencode/packages/sdk/js/src/gen/types.gen.ts`

### Message Events
*   **`message.updated`**: Triggered when a message's content or state changes.
    *   `properties: { info: Message }`
*   **`message.removed`**: Triggered when a message is deleted.
    *   `properties: { sessionID: string; messageID: string }`

### Session Events
*   **`session.created`** / **`session.updated`**:
    *   `properties: { info: Session }`
*   **`session.status`**:
    *   `properties: { sessionID: string; status: SessionStatus }`
*   **`session.deleted`**:
    *   `properties: { sessionID: string }`

### Tool & Command Events
*   **`command.executed`**: Triggered after a slash command has run.
    *   `properties: { name: string; sessionID: string; arguments: string; messageID: string }`

### File & VCS Events
*   **`file.edited`**: Triggered when a file is modified via OpenCode.
    *   `properties: { file: string }`
*   **`file.watcher.updated`**: Triggered by the filesystem watcher.
    *   `properties: { file: string; event: "add" \| "change" \| "unlink" }`
*   **`vcs.branch.updated`**: Triggered when the git branch changes.
    *   `properties: { branch?: string }`

### LSP & TUI Events
*   **`lsp.client.diagnostics`**: LSP diagnostic updates.
    *   `properties: { serverID: string; path: string }`
*   **`tui.prompt.append`**: Text added to the TUI prompt.
    *   `properties: { text: string }`
*   **`tui.toast.show`**: A toast notification was triggered.
    *   `properties: { message: string; variant: "info" \| "success" \| "warning" \| "error" }`

---

## 3. Core Data Schemas

Common interfaces passed as `input` or `output` to hooks.

### `Message`
```typescript
export type Message = {
  id: string
  sessionID: string
  role: "user" | "assistant" | "system"
  agent: string
  model: { providerID: string; modelID: string }
  timestamp: string
  // ... other fields
}
```

### `Permission`
```typescript
export type Permission = {
  id: string
  type: string
  pattern?: string | Array<string>
  sessionID: string
  messageID: string
  callID?: string
  title: string
  metadata: { [key: string]: unknown }
}
```

### `Model`
```typescript
export type Model = {
  id: string
  providerID: string
  name: string
  capabilities: { [key: string]: any }
  limit: { context: number; output: number }
  options: { [key: string]: unknown }
}
```
