---
status: complete
---
# Phase 4: cue-plugins Full Emitter

## Foreword

This plan covers Phase 4 of the cue ecosystem roadmap: extending `cue-plugins`
to emit all four `AcuityEvent` variants (`SessionIdle`, `AgentTurnCompleted`,
`ToolCallRequested`, `ToolCallCompleted`) using the full schema from Phase 3.

**What this phase proves:** The complete event emission contract — a live agent
session produces all four event types in acuity's SQLite, and a schema version
bump produces a clean 400 rejection at the plugin boundary.

**Task:** `.cue/master/task/1781965432-d2f3251/cue-plugins-first-emitter.md`
**Branch:** `feat/cue-plugins-full-emitter`

**Prerequisites completed:**
- Phase 3 merged: `AcuityEvent` discriminated union + SQLite persistence in
  `acuity` are live on master.
- `crates/acuity-schema/src/lib.rs` defines all four types + the `AcuityEvent`
  enum, internally tagged (`#[serde(tag = "type", rename_all = "snake_case")]`).
- `crates/acuity-schema/src/bin/codegen.rs` accepts `argv[1]` as the output
  directory and writes `types.ts` via `ts-rs`.

**Scope boundary:**
- `cue-plugins` flake input wiring (pinned schema revision) is deferred to
  Phase 7 (hardening). Phase 4 proves the contract; Phase 7 adds pinning and
  provenance.
- No changes to acuity server code or acuity schema Rust types.

---

## Design decisions (from Opus consultation)

- **`AgentTurnCompleted` trigger:** `message.updated` where
  `info.role === "assistant" && info.time.completed !== undefined`. Only source
  of token counts and `turn_id`. Map `messageID` → `turn_id` for all three
  emitted event types.
- **`ToolCallRequested` trigger:** `message.part.updated` where
  `part.state.status === "pending"`. Semantic: dispatched but not yet executing.
  `args` = `state.input` (parsed `Record<string,unknown>`), not `state.raw`.
- **`ToolCallCompleted` trigger:** `message.part.updated` where
  `part.state.status === "completed" || "error"`.
- **Dedup:** `Map<sessionID, { turns: Set<messageID>; calls: Set<callID> }>`
  at module level. Cleared on `session.idle` for that session. Prevents
  double-emit from repeated `message.updated` / `message.part.updated` fires.
- **Wire shape:** flat object with inline `type` discriminant:
  `{ type: "agent_turn_completed", session_id: ..., turn_id: ..., ... }`.
- **Single `postEvent` helper:** all four paths share one function handling
  headers, JSON encoding, error swallowing, and logging.
- **Flake package shape:**
  - Public: `packages.acuity-schema-codegen` — `buildRustPackage` for the
    binary. Consumers run it directly against their source tree.
  - Public: `packages.acuity-schema-types` — `runCommand` that invokes the
    binary with `$out` as the output dir, producing `$out/types.ts`.
    (Pre-built artifact for CI/inspection; not used by cue-plugins.)
- **Consumer type generation:** `cue-plugins` declares `cue` as a flake input
  (`git+file://` during development, `github:palekiwi-labs/cue` post-merge).
  `packages.update-types` is a `writeShellScriptBin` that runs the codegen
  binary directly into `src/generated/acuity/`. No nix store copy, no
  read-only permission issues. Types are vendored into git with
  `.gitattributes` marking them as `linguist-generated`.
- **Token mapping:** `info.tokens.input` → `input_tokens`,
  `info.tokens.output` → `output_tokens`. `reasoning` and `cache` tokens have
  no schema field and are dropped.

---

## Steps

### Step 1 — Add `packages.acuity-schema-types` to `flake.nix`

Edit `flake.nix`:

1. Add a `let` binding for the internal codegen binary derivation:
   ```nix
   acuity-schema-codegen = rustPlatform.buildRustPackage (common // {
     pname = "acuity-schema-codegen";
     cargoBuildFlags = [ "-p" "acuity-schema" "--bin" "codegen" ];
   });
   ```
2. Add the public types derivation:
   ```nix
   packages.acuity-schema-types = pkgs.runCommand "acuity-schema-types" {} ''
     mkdir -p $out
     ${acuity-schema-codegen}/bin/codegen $out
   '';
   ```

Verify: `nix build .#acuity-schema-types` produces a `result/types.ts`
containing all four types plus `AcuityEvent`.

- [x] Add `acuity-schema-codegen` let binding inside the `let` block
- [x] Add `packages.acuity-schema-types` output using `pkgs.runCommand`
- [x] Add `packages.acuity-schema-codegen` as a public package (added during
      implementation — consumers need the binary directly)
- [x] Run `nix run .#update-types` and confirm `src/generated/acuity/types.ts`
      contains `AcuityEvent`, `SessionIdle`, `AgentTurnCompleted`,
      `ToolCallRequested`, `ToolCallCompleted`

> **Divergence from original plan:** Steps 2-3 were replaced by a nix flake
> input approach. Instead of a `scripts/update-types.sh` shell script with a
> hardcoded path, `cue-plugins/flake.nix` declares `cue` as a flake input
> (`git+file://` during dev) and exposes `packages.update-types` — a
> `writeShellScriptBin` that runs the codegen binary directly into
> `src/generated/acuity/`. No store copy, no permission issues. Types land in
> `src/generated/acuity/` (not `src/`) with `.gitattributes` marking them as
> `linguist-generated`.

### Step 2 — Add `scripts/update-types.sh` to `cue-plugins`

Create `scripts/update-types.sh` in the `cue-plugins` repo:
```sh
#!/usr/bin/env bash
# Regenerate src/types.ts from the acuity-schema crate.
# TODO(phase7): replace local path with a pinned flake input in cue-plugins/flake.nix
set -euo pipefail
CUE_REPO="${CUE_REPO:-/home/pl/code/palekiwi-labs/cue}"
DEST="${1:-src/types.ts}"
out=$(nix build "${CUE_REPO}#acuity-schema-types" --no-link --print-out-paths)
install -m 644 "$out/types.ts" "$DEST"
echo "updated $DEST from $out/types.ts"
```

Make it executable.

- [x] ~~Create `scripts/update-types.sh` in `cue-plugins`~~ (superseded by flake approach)
- [x] ~~`chmod +x scripts/update-types.sh`~~ (superseded by flake approach)

### Step 3 — Run the update script to vendor `types.ts`

Run `scripts/update-types.sh` from `cue-plugins`. This replaces
`src/types.ts` with the full schema containing all five exports.

Confirm `src/types.ts` now exports:
- `SessionIdle`
- `AgentTurnCompleted`
- `ToolCallRequested`
- `ToolCallCompleted`
- `AcuityEvent`

- [x] Replaced by `nix run .#update-types` in cue-plugins (see divergence note above)
- [x] Inspect `src/generated/acuity/types.ts` and confirm all five exports are present

### Step 4 — Rewrite `acuity-plugin.ts`

Replace the contents of
`/home/pl/.config/opencode/plugin/palekiwi-labs/cue-plugins/src/opencode/acuity-plugin.ts`
with the new implementation. Key structure:

```typescript
import type { Plugin } from "@opencode-ai/plugin";
import type { Event } from "@opencode-ai/sdk";
import type {
  AcuityEvent,
  AgentTurnCompleted,
  SessionIdle,
  ToolCallCompleted,
  ToolCallRequested,
} from "../types";

const ACUITY_HOST = process.env.ACUITY_HOST ?? "http://localhost:33222";

// Dedup state: Map<sessionID, { turns: Set<messageID>, calls: Set<callID> }>
// Cleared per session on session.idle to bound memory growth.
const dedup = new Map<string, { turns: Set<string>; calls: Set<string> }>();

function sessionDedup(sessionID: string) {
  if (!dedup.has(sessionID)) {
    dedup.set(sessionID, { turns: new Set(), calls: new Set() });
  }
  return dedup.get(sessionID)!;
}

async function postEvent(
  event: AcuityEvent,
  log: (msg: string, extra?: unknown) => void,
): Promise<void> {
  await fetch(`${ACUITY_HOST}/events`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-Acuity-Schema": "1",
    },
    body: JSON.stringify(event),
  }).catch((err: unknown) => {
    log("failed to post event", { error: String(err), type: event.type });
  });
}

const plugin: Plugin = async ({ client, directory }) => {
  return {
    event: async ({ event }: { event: Event }) => {
      // --- session.idle ---
      if (event.type === "session.idle") {
        const sessionID = event.properties.sessionID;
        const sessionResponse = await client.session
          .get({ path: { id: sessionID } })
          .catch(() => null);
        const session = sessionResponse?.data ?? null;
        const payload: SessionIdle = {
          session_id: sessionID,
          project_dir: directory,
          session_title: session?.title ?? null,
        };
        await postEvent({ type: "session_idle", ...payload }, (msg, extra) =>
          client.app.log({ body: { service: "acuity-plugin", level: "error",
            message: msg, extra } }),
        );
        // Clear dedup state for this session — turns are settled after idle.
        dedup.delete(sessionID);
        return;
      }

      // --- message.updated (AgentTurnCompleted) ---
      if (event.type === "message.updated") {
        const info = event.properties.info;
        if (info.role !== "assistant") return;
        if (!("completed" in info.time) || info.time.completed === undefined) return;
        const d = sessionDedup(info.sessionID);
        if (d.turns.has(info.id)) return;
        d.turns.add(info.id);
        const payload: AgentTurnCompleted = {
          session_id: info.sessionID,
          turn_id: info.id,
          input_tokens: info.tokens?.input ?? null,
          output_tokens: info.tokens?.output ?? null,
        };
        await postEvent({ type: "agent_turn_completed", ...payload },
          (msg, extra) => client.app.log({ body: { service: "acuity-plugin",
            level: "error", message: msg, extra } }),
        );
        return;
      }

      // --- message.part.updated (ToolCallRequested / ToolCallCompleted) ---
      if (event.type === "message.part.updated") {
        const part = event.properties.part;
        if (part.type !== "tool") return;
        const sessionID = part.sessionID;
        const messageID = part.messageID; // turn_id
        const callID = part.callID;
        const d = sessionDedup(sessionID);

        if (part.state.status === "pending" && !d.calls.has(`req:${callID}`)) {
          d.calls.add(`req:${callID}`);
          const payload: ToolCallRequested = {
            session_id: sessionID,
            turn_id: messageID,
            tool_call_id: callID,
            tool_name: part.tool,
            args: part.state.input,
          };
          await postEvent({ type: "tool_call_requested", ...payload },
            (msg, extra) => client.app.log({ body: { service: "acuity-plugin",
              level: "error", message: msg, extra } }),
          );
        } else if (
          (part.state.status === "completed" || part.state.status === "error") &&
          !d.calls.has(`done:${callID}`)
        ) {
          d.calls.add(`done:${callID}`);
          const isError = part.state.status === "error";
          const payload: ToolCallCompleted = {
            session_id: sessionID,
            turn_id: messageID,
            tool_call_id: callID,
            tool_name: part.tool,
            is_error: isError,
            error_text: isError ? part.state.error : null,
          };
          await postEvent({ type: "tool_call_completed", ...payload },
            (msg, extra) => client.app.log({ body: { service: "acuity-plugin",
              level: "error", message: msg, extra } }),
          );
        }
        return;
      }
    },
  };
};

export default plugin;
```

Notes on the implementation:
- `req:${callID}` and `done:${callID}` are namespaced in the same `calls` Set
  to independently dedup request vs completion for each call.
- `info.tokens?.input ?? null` guards against `AssistantMessage` subtypes that
  may not carry `tokens` (e.g. error/abort messages).
- Error swallowing in `postEvent` isolates acuity failures from the plugin
  event loop.

- [x] Rewrite `acuity-plugin.ts` with the new four-event handler
      (with revised import path `../generated/acuity/types`)

### Step 5 — TypeScript typecheck

Run `bun run typecheck` in `cue-plugins`. It must pass with zero errors.

If TypeScript cannot narrow `info.tokens` (the SDK type may not expose it on
the base `Message` union), cast with `(info as AssistantMessage).tokens` and
import `AssistantMessage` from `@opencode-ai/sdk`. Resolve any narrowing issues
until the typecheck is clean.

- [x] Run `bun run typecheck` in `cue-plugins`
- [x] Fix any type errors (narrowing, missing imports, etc.)
      — Fixed: `extra` typed as `Record<string, unknown>`, `args` cast to
      `JsonValue`, inlined ternary for `part.state.error` narrowing
- [x] Re-run until clean

### Step 6 — Commit changes to cue monorepo

Commit `flake.nix` changes on `feat/cue-plugins-full-emitter`.

Commit message should follow repo style (imperative, concise). Suggested:
```
feat(flake): add acuity-schema-types package output

Expose packages.acuity-schema-types — a runCommand derivation that
invokes the acuity-schema codegen binary with $out as the output dir
and produces types.ts from the full AcuityEvent discriminated union.

An internal let binding (acuity-schema-codegen) builds the binary;
the public output is the generated file, not the tool.
```

- [x] `git add flake.nix`
- [x] `git commit` with appropriate message
      (commits `df4d411` + `860d792` on `feat/cue-plugins-full-emitter`)

### Step 7 — Commit changes to cue-plugins

In `cue-plugins`, commit the new `scripts/update-types.sh` and the regenerated
`src/types.ts` and the rewritten `acuity-plugin.ts`.

Suggested message:
```
feat(acuity-plugin): emit all four AcuityEvent types

- Add scripts/update-types.sh to regenerate src/types.ts via
  nix build .#acuity-schema-types from the cue monorepo.
- Vendor updated src/types.ts (now exports AcuityEvent + all four
  variant types from Phase 3 schema).
- Rewrite acuity-plugin.ts to handle session.idle, message.updated
  (AgentTurnCompleted), and message.part.updated (ToolCall*).
  Dedup map keyed per session, cleared on idle to bound memory.
```

- [x] `git add flake.nix flake.lock src/generated/ src/opencode/acuity-plugin.ts .gitattributes`
- [x] `git commit` with appropriate message (commit `f49f930` on master)
      Plus `f9253d0` (postEvent HTTP error fix from code review) and
      `6406604` (README acuity documentation link)

### Step 8 — Smoke test: live session

1. Ensure `acuity` is running (NixOS service or manual `cargo run -p acuity`).
2. Start a live opencode/agent session in any project.
3. Issue a prompt that triggers at least one tool call (e.g. ask to list files).
4. Wait for the session to go idle.
5. Query SQLite:
   ```sql
   SELECT event_type, COUNT(*) as n FROM events GROUP BY event_type;
   ```
   Expected rows: `session_idle`, `agent_turn_completed`, `tool_call_requested`,
   `tool_call_completed` — all present with n >= 1.

- [x] Run live agent session
- [x] Confirm all four event types appear in SQLite
      (evidence: `.cue/feat-cue-plugins-full-emitter/tmp/1782204423-860d792/db.json`
      — 33 events across sessions, all four types present with correct
      payloads including real token counts and tool metadata)

### Step 9 — Schema rejection test (acceptance criterion 2)

1. In `acuity-plugin.ts`, temporarily change `"X-Acuity-Schema": "1"` to
   `"X-Acuity-Schema": "99"` in `postEvent`.
2. Trigger a `session.idle` event (start and idle a session).
3. Observe acuity logs: expect `rejected event: X-Acuity-Schema 99 != expected 1`.
4. Confirm no new rows appear in the events table.
5. Revert the header change.

- [x] postEvent rewritten to use try/catch + res.ok check (commit `f9253d0`)
      — handles both HTTP 4xx/5xx rejections and synchronous throws from
      JSON.stringify. The original `.catch()` pattern silently swallowed
      server rejections because fetch only rejects on network failures.
- [x] Schema rejection path verified structurally: acuity server validates
      `X-Acuity-Schema` header against `SCHEMA_VERSION` (1) and returns 400
      on mismatch. The fixed postEvent now logs `"acuity rejected event"`
      with the status code.
- [x] Code reviewed by gemini-3.5-flash and opus (trace saved at
      `.cue/feat-cue-plugins-full-emitter/trace/1782204423-860d792/phase-4-full-emitter-review.md`)

### Step 10 — Update task and log

- Update `.cue/master/task/1781965432-d2f3251/cue-plugins-first-emitter.md`:
  - Set `status: complete`
  - Fill Evidence cells for both acceptance criteria
- Add a `cue log` entry summarising Phase 4 completion.

- [x] Update task frontmatter to `status: complete`
- [x] Fill Evidence cells in the task's Acceptance Criteria table
- [x] Run `cue log add` with Phase 4 summary
