---
status: complete
---
## Foreword

This plan covers Phase 1 of the cue ecosystem roadmap: the `acuity` stateless
MVP. It implements task `task/1781965432-d2f3251/acuity-stateless-mvp.md`.

Scope is intentionally narrow by hard constraint:
- No SQLite. No second event type. No SSE or query surface.
- One POST endpoint. One event type. One downstream (Gotify).

The deliverable is a real `session.idle` POST from the opencode plugin, carrying
a type imported from the vendored `types.ts` with a correct `X-Acuity-Schema`
header, deserialized by `acuity` and forwarded to Gotify.

Prerequisites: Phase 0 complete (all six crates compile, codegen command
exists, `cue-plugins` repo initialized with vendored `types.ts`).

### Key decisions recorded here

- **`SessionIdle` fields:** `session_id`, `project_dir`, `session_title: Option<String>`
  (plugin calls `client.session.get()` to resolve human-readable title)
- **Endpoint:** `POST /events`
- **Config:** figment pattern (same as `cue`) -- defaults ->
  `~/.config/acuity/acuity.json` -> `ACUITY_` env vars; token via
  `ACUITY_GOTIFY_TOKEN` env var only (not stored in config file)
- **`SCHEMA_VERSION`:** `pub const SCHEMA_VERSION: u8 = 1;` in `acuity-schema`
- **Gotify wire format:** JSON (not multipart)

---

## Steps

### Area 1 -- `acuity-schema`: define the real event type

- [x] **1.1** In `crates/acuity-schema/src/lib.rs`:
  - Remove the `Placeholder` struct and its doc comment
  - Add `pub const SCHEMA_VERSION: u8 = 1;`
  - Add `SessionIdle` struct with `serde` + `ts-rs` derives:
    ```rust
    #[derive(Debug, Serialize, Deserialize, TS)]
    #[ts(export_to = "types.ts")]
    pub struct SessionIdle {
        pub session_id: String,
        pub project_dir: String,
        pub session_title: Option<String>,
    }
    ```

- [x] **1.2** In `crates/acuity-schema/src/bin/codegen.rs`:
  - Replace `use acuity_schema::Placeholder;` with `use acuity_schema::SessionIdle;`
  - Replace `Placeholder::export_all(&cfg)` with `SessionIdle::export_all(&cfg)`

- [x] **1.3** Verify: `cargo build -p acuity-schema` passes with no warnings.

---

### Area 2 -- `acuity` binary: config + HTTP server

- [x] **2.1** Add dependencies to `crates/acuity/Cargo.toml`:
  ```toml
  [dependencies]
  acuity-schema = { path = "../acuity-schema" }
  acuity-api    = { path = "../acuity-api" }
  axum          = "0.8"
  tokio         = { version = "1", features = ["full"] }
  reqwest       = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
  serde         = { version = "1", features = ["derive"] }
  serde_json    = "1"
  figment       = { version = "0.10", features = ["json", "env"] }
  anyhow        = "1"
  tracing       = "0.1"
  tracing-subscriber = { version = "0.3", features = ["env-filter"] }
  dirs          = "6"
  ```
  Note: `reqwest` must use `default-features = false` + `rustls-tls` -- the Nix
  Rust devshell does not provide OpenSSL headers so `native-tls` fails to build.

- [x] **2.2** Create `crates/acuity/src/config.rs`:
  - `Config` struct: `gotify_host: String`, `port: u16`
  - `Default` impl: `gotify_host = "localhost:80"`, `port = 33222`
    (33222 matches the port the ref plugin already posts to -- no change
    needed on the plugin side for the URL)
  - `Config::load()` using figment, same layering as `cuelib`:
    defaults -> `~/.config/acuity/acuity.json` (or `$ACUITY_CONFIG_DIR/acuity.json`)
    -> `Env::prefixed("ACUITY_").split("__")`

- [x] **2.3** Implement `crates/acuity/src/main.rs`:
  - `AppState`: holds `Config` + `gotify_token: String` (read from
    `ACUITY_GOTIFY_TOKEN` env var at startup; hard-exit with a clear
    error message if missing)
  - Single route: `POST /events`
  - Handler logic (in order):
    1. Read `X-Acuity-Schema` header; reject with `400` if absent or
       value != `SCHEMA_VERSION.to_string()`
    2. Deserialize JSON body as `SessionIdle`; reject with `422` if
       malformed
    3. Compose Gotify payload:
       - `title`: `Path::new(&event.project_dir).file_name()` (basename)
       - `message`: `format!("Idle: {}", event.session_title.as_deref().unwrap_or(&event.session_id))`
       - `priority`: 5
    4. POST to `http://{gotify_host}/message?token={token}` with JSON body
    5. Return `200 OK` on success; `502` if Gotify call fails (log the error)
  - `main()`: load config, read token, init tracing, bind on configured port,
    serve

- [x] **2.4** Verify: `cargo build -p acuity` passes with no warnings.

---

### Area 3 -- codegen run and vendoring

- [x] **3.1** Run codegen to update `dist/`:
  ```bash
  cargo run -p acuity-schema --bin codegen -- crates/acuity-schema/dist
  ```
  Inspect `crates/acuity-schema/dist/types.ts` -- should contain `SessionIdle`,
  no `Placeholder`.

- [x] **3.2** Vendor to `cue-plugins`:
  ```bash
  cargo run -p acuity-schema --bin codegen -- /home/pl/code/palekiwi-labs/cue-plugins/src
  ```
  Inspect `cue-plugins/src/types.ts` -- should match `dist/types.ts`.

---

### Area 4 -- `cue-plugins`: opencode plugin

Runtime facts (from research report
`.cue/master/doc/1781975079-858c351/opencode-plugin-guide.md`):

- opencode runs plugins under **Bun** -- `.ts` files are executed natively,
  no transpilation step.
- Plugin loading is a plain `await import(file_url)` -- no module injection.
- Auto-discovery glob (`{plugin,plugins}/*.{ts,js}`) is **non-recursive**.
  Any plugin outside a default `plugin/` directory must be declared explicitly.
- `fetch` is a Bun global -- no import needed.
- `@opencode-ai/plugin` / `@opencode-ai/sdk` are auto-installed only inside
  `.opencode/` directories. Plugins in external repos must use `import type`
  only for these packages (Bun strips type-only imports at runtime, so no
  runtime resolution failure occurs).
- Import extension convention in opencode's own codebase: **extensionless**.
  All three forms work at runtime; extensionless is the house style.

- [x] **4.1** Create `cue-plugins/package.json`:
  ```json
  {
    "type": "module",
    "devDependencies": {
      "@opencode-ai/plugin": "*",
      "@opencode-ai/sdk": "*",
      "@types/node": "*",
      "typescript": "*"
    }
  }
  ```
  Also created `flake.nix` with a Bun devshell (Node/npm not available in the
  Rust devshell). Run `nix develop` inside `cue-plugins/` then `bun install`.
  `@types/node` required for `process.env` and `Buffer` types used transitively
  by `@opencode-ai/plugin`.

- [x] **4.2** Create `cue-plugins/src/acuity-plugin.ts`:
  - `import type { Plugin } from "@opencode-ai/plugin"` and
    `import type { Event } from "@opencode-ai/sdk"` (both stripped at runtime)
  - `Client` is not exported from `@opencode-ai/sdk`; type flows from `PluginInput`
  - `fetch` used as a Bun global -- no import
  - `ACUITY_HOST`: `process.env.ACUITY_HOST ?? "http://localhost:33222"`
    (not hardcoded -- configurable per-environment at runtime)
  - `Plugin` is a **function** `(PluginInput) => Promise<Hooks>`, not a plain
    object; default export is `async ({ client, directory }) => { return { event: ... }; }`
  - On `session.idle`: call `client.session.get()`, unwrap `.data`, construct
    `SessionIdle` payload, POST to `${ACUITY_HOST}/events`
  - Errors logged via `client.app.log()` (not `console.error` -- would corrupt TUI)

- [x] **4.3** Register the plugin globally. Add to
  `~/.config/opencode/opencode.json` (create file if it does not exist):
  ```json
  {
    "plugin": ["/home/pl/code/palekiwi-labs/cue-plugins/src/acuity-plugin.ts"]
  }
  ```
  Relative paths in that file anchor at `~/.config/opencode/`; absolute
  path used here for clarity.

- [x] **4.4** Decommission the current notification plugin. Move
  `/home/pl/.config/opencode/plugin/notification.ts` out of the `plugin/`
  directory (e.g. to `~/.config/opencode/plugin/archive/notification.ts`).
  No config change needed -- auto-discovery will no longer find it.
  Gotify will then receive notifications only from `acuity`.

---

### Area 5 -- full-stack verification

- [x] **5.1** `cargo build --workspace` -- all six crates, zero warnings.

- [x] **5.2** Start `acuity`:
  ```bash
  ACUITY_GOTIFY_TOKEN=<token> cargo run -p acuity
  ```

- [x] **5.3** Smoke-test schema rejection:
  ```bash
  curl -s -w "\n%{http_code}" -X POST http://localhost:33222/events \
    -H "Content-Type: application/json" \
    -H "X-Acuity-Schema: 99" \
    -d '{"session_id":"x","project_dir":"/tmp","session_title":null}'
  ```
  Expected: `400`.

- [x] **5.4** Smoke-test valid event:
  ```bash
  curl -s -w "\n%{http_code}" -X POST http://localhost:33222/events \
    -H "Content-Type: application/json" \
    -H "X-Acuity-Schema: 1" \
    -d '{"session_id":"test-123","project_dir":"/home/pl/code/palekiwi-labs/cue","session_title":"test notification"}'
  ```
  Expected: `200` and a Gotify notification appears.

- [x] **5.5** Run a live agent session with the plugin active; observe Gotify
  notification on idle. This is the human-attested acceptance criterion.

---

## Steps
### Area 6 -- housekeeping

- [x] **6.1** Commit the `cue` workspace changes (acuity-schema + acuity crates).
  Commit: `5a3ea36`
- [x] **6.2** Commit the `cue-plugins` repo changes (updated `types.ts` + new plugin).
  Commits: `fc1373d`, `8fba80d`, `1e069a4`, `8d2cab1`
- [x] **6.3** Update task status:
  - Fill Evidence cells for criteria 1-4 in `acuity-stateless-mvp.md`
  - Criterion 5 (decommission ref server) requires human attestation
  - Set `status: complete` only after all Evidence cells are filled
- [x] **6.4** `cue log add` entry recording the milestone.
