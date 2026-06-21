---
research_date: 2026-06-21
source_ref: ref/opencode
topic: opencode-plugin-authoring-guide
---
# OpenCode Plugin Authoring — Comprehensive Guide

**Research target:** How to author, register, and structure TypeScript plugins for opencode — including plugins that live outside `.opencode/plugin/`, plugins cloned as git repos without npm publication, and multi-plugin repos that may also contain plugins for other agent harnesses.

**Source root for all paths below:** `/home/pl/.config/opencode/ref/opencode/`

**Research date:** 2026-06-21

All confidence markers are high unless noted. This guide consolidates and supersedes `opencode-plugin-typescript-guide.md` and `opencode-plugin-discovery-and-multi-repo.md`.

---

## Table of Contents

1. [Plugin Registration](#1-plugin-registration)
2. [Plugin Resolution](#2-plugin-resolution)
3. [Runtime Behavior](#3-runtime-behavior)
4. [Dependencies & Types](#4-dependencies--types)
5. [Multi-Plugin Repo Patterns](#5-multi-plugin-repo-patterns)
6. [Cross-Cutting Notes](#6-cross-cutting-notes)
7. [Confidence Notes & Gaps](#7-confidence-notes--gaps)
8. [Methodology](#8-methodology)

---

## 1. Plugin Registration

There are **two independent** registration mechanisms. They do not consult each other.

### 1.1 Explicit `plugin:` array in `opencode.json`

Each entry is one of:

| Spec form | Example |
| --- | --- |
| npm spec (latest) | `"opencode-gemini-auth"` |
| npm spec (pinned) | `"opencode-foo@1.2.3"` |
| relative file path | `"./local-plugin.ts"` |
| absolute path | `"/home/pl/dev/cue-plugins/src/acuity-plugin.ts"` |
| file URL | `"file:///home/pl/dev/cue-plugins/src/acuity-plugin.ts"` |
| directory (relative or absolute) | `"./plugin/cue-plugins"` |
| tuple form (spec + options) | `["opencode-bar", { "option": "value" }]` |

**Relative paths anchor at the declaring config file's directory** — not the cwd or worktree root.

- `packages/opencode/src/config/plugin.ts:49-54` — `resolvePluginSpec`:
  ```ts
  const base = path.dirname(configFilepath)
  const file = (() => {
    if (spec.startsWith("file://")) return spec
    if (path.isAbsolute(spec) || /^[A-Za-z]:[\\/]/.test(spec)) return pathToFileURL(spec).href
    return pathToFileURL(path.resolve(base, spec)).href
  })()
  ```
- `packages/opencode/src/plugin/shared.ts:171-173` — `isPathPluginSpec` recognizes `file://`, leading `.`, and absolute paths:
  ```ts
  export function isPathPluginSpec(spec: string) {
    return spec.startsWith("file://") || spec.startsWith(".") || isAbsolutePath(spec)
  }
  ```

**Example:** a spec like `"./plugin/cue-plugins/src/acuity-plugin.ts"` in `~/.config/opencode/opencode.json` resolves against `~/.config/opencode/`, yielding `~/.config/opencode/plugin/cue-plugins/src/acuity-plugin.ts`.

### 1.2 Auto-discovery

**Important:** The auto-discovery glob is **non-recursive**. Only files **directly** inside `plugin/` or `plugins/` are discovered — nested subdirectories are not descended into.

- `packages/opencode/src/config/plugin.ts:21-28`:
  ```ts
  for (const item of await Glob.scan("{plugin,plugins}/*.{ts,js}", {
    cwd: dir,
    absolute: true,
    dot: true,
    symlink: true,
  })) {
    plugins.push(pathToFileURL(item).href)
  }
  ```

**Consequence:** A repo cloned to `~/.config/opencode/plugin/cue-plugins/` will **never** be auto-discovered, regardless of what files it contains. Explicit `plugin:` entries are mandatory for any subdirectory plugin.

### 1.3 Default scan locations

Auto-discovery runs the same non-recursive `{plugin,plugins}/*.{ts,js}` glob against each of these directories:

1. **Global config dir:** `~/.config/opencode/plugin/` and `~/.config/opencode/plugins/` (both names accepted everywhere)
2. **Project-local:** `<project>/.opencode/plugin/` and `<project>/.opencode/plugins/`, plus any ancestor `.opencode/` discovered by walking up from the project root
3. **`OPENCODE_CONFIG_DIR` env var:** if set, that directory is scanned
4. **`OPENCODE_CONFIG` env var:** if pointing to a file, that file's parent directory is scanned

- `packages/opencode/src/config/config.ts:415-459` — `loadInstanceState` iterates `directories` and calls `ConfigPlugin.load(dir)` for each:
  ```ts
  for (const dir of directories) {
    // ...
    const list = yield* Effect.promise(() => ConfigPlugin.load(dir))
    yield* mergePluginOrigins(dir, list)
  }
  ```
- `packages/opencode/src/config/config.ts:398-400` — config files are walked and merged.

### 1.4 No equivalent of `skills.paths` for plugins

Unlike `skills.paths`, there is **no config field** for customizing plugin discovery directories. No `plugin.paths`, `plugins.paths`, or `plugin_directories` field exists in the published schema at `https://opencode.ai/config.json`. The only plugin-related env var in the codebase is `OPENCODE_PLUGIN_META_FILE` (`packages/opencode/src/plugin/meta.ts`), which controls metadata storage, **not** discovery.

**Implication:** The only way to load a plugin from a non-default location is to add an explicit entry to the `plugin:` array.

---

## 2. Plugin Resolution

### 2.1 Load call

All plugins — regardless of registration mechanism — are normalized to a `file://` URL and loaded via a plain dynamic import. **No module-resolution magic.**

- `packages/opencode/src/plugin/loader.ts:139`:
  ```ts
  mod = await import(row.entry)
  ```

### 2.2 File-spec resolution

File-path specs (relative, absolute, `file://`) resolve directly to the target file. No depth or naming restrictions beyond standard filesystem constraints.

### 2.3 Directory-spec resolution (two-stage)

**Stage 1 — `resolvePathPluginTarget`** (`packages/opencode/src/plugin/shared.ts:175-192`):
```ts
if (await Filesystem.exists(path.join(file, "package.json"))) {
  return pathToFileURL(file).href
}
```
- If a `package.json` exists at the directory root, the directory itself is the target.
- Otherwise, scan for an index file.

**Stage 2 — `resolvePluginEntrypoint`** (`packages/opencode/src/plugin/shared.ts:136-169`):
- For a directory with `package.json`: consult `exports['./server']` first, then `main` (via `resolvePackageEntrypoint`, lines 103-114).
- Fall back to `resolveDirectoryIndex` only when the source is a file-path spec and no package entrypoint resolves.

**Index-file scan** (`packages/opencode/src/plugin/shared.ts:121-126`):
```ts
async function resolveDirectoryIndex(dir: string) {
  for (const name of INDEX_FILES) {
    const file = path.join(dir, name)
    if (await Filesystem.exists(file)) return file
  }
}
```

**Resolution precedence:** `package.json` → `index.ts` → `index.tsx` → `index.js` → `index.mjs` → `index.cjs`.

**A directory resolves to exactly one entrypoint.** Registering multiple plugins in one directory requires multiple file-path entries (see §5).

### 2.4 Multi-entry deduplication

Multiple `plugin:` entries pointing into the same repo are fully supported. Deduplication keys on the exact `file://` URL string — distinct files produce distinct URLs and load independently.

- `packages/opencode/src/config/plugin.ts:64-77`:
  ```ts
  const name = spec.startsWith("file://") ? spec : parsePluginSpecifier(spec).pkg
  if (seen.has(name)) continue
  ```

### 2.5 Required-export shape

The loader only inspects the module's default export. It does not import `@opencode-ai/plugin` types as values.

- `packages/opencode/src/plugin/shared.ts:278-281`:
  ```ts
  const value = mod.default
  if (!isRecord(value)) {
    if (mode === "detect") return
    throw new TypeError(`Plugin ${spec} must default export an object with ${kind}()`)
  }
  ```

---

## 3. Runtime Behavior

### 3.1 TypeScript runtime: Bun

Plugins are executed by **Bun**, which runs `.ts` files natively. No transpiler (`tsx`, `ts-node`, `esbuild`, `vite`) is involved in the plugin load path.

- `packages/opencode/package.json:10` — `"test": "bun test --timeout 30000"`.
- `packages/opencode/src/plugin/index.ts:162` — Bun APIs are first-class:
  ```ts
  $: typeof Bun === "undefined" ? undefined : Bun.$,
  ```
- `packages/opencode/tsconfig.json:3` — `"extends": "@tsconfig/bun/tsconfig.json"`.

### 3.2 Co-located module imports

Bun accepts all three of these forms at runtime:
- `import type { SessionIdle } from "./types.ts"` — works
- `import type { SessionIdle } from "./types.js"` — works at runtime in Bun (extension is not validated against the actual file)
- `import type { SessionIdle } from "./types"` — works

**Repo convention (what opencode itself does):** **extensionless**.

- `packages/opencode/src/plugin/index.ts:27`:
  ```ts
  import { parsePluginSpecifier, readPluginId, readV1Plugin, resolvePluginId } from "./shared"
  ```
- `packages/opencode/src/plugin/github-copilot/copilot.ts:7`:
  ```ts
  import { CopilotModels } from "./models"
  ```

**Caveat (medium confidence).** The subagent did not find an explicit `moduleResolution` setting in `packages/opencode/tsconfig.json`; the behavior follows from `@tsconfig/bun/tsconfig.json`. Your own `tsconfig.json` does not need to mirror opencode's — Bun resolves at runtime regardless. For editor/tsc type-checking, pick any `moduleResolution` that supports your chosen extension style (`bundler` or `node` both work for extensionless; `node16`/`nodenext` would require `.ts` extensions unless using `bundler`).

### 3.3 `package.json` requirement

**Not required** for a plugin referenced by direct file path. Required only for:
- npm-spec plugins, and
- directory-spec plugins (a directory must contain either `index.ts`/`index.js` **or** a `package.json`)

There is **no mandatory `"type": "module"` check** — Bun treats `.ts` as ESM by default.

- `packages/opencode/src/plugin/shared.ts:191`:
  ```ts
  throw new Error(`Plugin directory ${file} is missing package.json or index file`)
  ```
- `packages/opencode/src/plugin/shared.ts:227` — `readPluginPackage` is invoked for npm specs and (leniently) for local-directory specs:
  ```ts
  source === "npm" ? await readPluginPackage(target) : await readPluginPackage(target).catch(() => undefined)
  ```

A `package.json` is typically desirable even for file-spec plugins, for managing dev dependencies (`@opencode-ai/plugin`, `@opencode-ai/sdk`, `typescript`) and editor tooling.

### 3.4 `fetch` availability

**Yes.** Bun provides `fetch` as a global. Internal opencode plugins use it directly with no import.

- `packages/opencode/src/plugin/digitalocean.ts:95`:
  ```ts
  const res = await fetch(tokenUrl, {
  ```
- `packages/opencode/src/plugin/openai/codex.ts:109`:
  ```ts
  const response = await fetch(`${ISSUER}/oauth/token`, {
  ```

---

## 4. Dependencies & Types

### 4.1 Critical: no module-resolution magic

Three independently-verified facts:

1. **Auto-install is scoped to `.opencode/` only.** The config loader runs `npm install` for `@opencode-ai/plugin` exclusively inside the `.opencode` directory:
   - `packages/opencode/src/config/config.ts:429-436`:
     ```ts
     const dep = yield* npmSvc
       .install(dir, {
         add: [
           {
             name: "@opencode-ai/plugin",
             version: InstallationLocal ? undefined : InstallationVersion,
           },
         ],
       })
     ```
   Here `dir` is the `.opencode` directory, **not** the plugin's own directory.

2. **The loader does plain `await import()`** with no virtual-module injection, no custom resolver, no `NODE_PATH` manipulation, no alias map.
   - `packages/opencode/src/plugin/loader.ts:139` — `mod = await import(row.entry)`

3. **Standard Bun/Node walk-up resolution applies.** When a plugin file at `/home/pl/dev/cue-plugins/src/acuity-plugin.ts` imports `@opencode-ai/plugin`, Bun walks up from `cue-plugins/src/` looking for `node_modules/@opencode-ai/plugin`. The `.opencode/node_modules/` install from fact (1) is **not on this walk-up path** if `cue-plugins/` is a sibling of `.opencode/`, not a descendant.

### 4.2 Consequence for value imports

A non-type-only import such as `import { Plugin } from "@opencode-ai/plugin"` from a plugin outside `.opencode/` will **fail at runtime** unless the plugin's own `node_modules/@opencode-ai/plugin` exists (i.e. `npm install` was run inside the plugin's repo).

### 4.3 Mitigation via type-only imports

Bun strips `import type` at runtime. The plugin can use `import type { Plugin } from "@opencode-ai/plugin"` and `import type { Event } from "@opencode-ai/sdk"` for **type positions only**, and these imports will never reach the runtime resolver. opencode's own internal plugins follow exactly this pattern:

- `packages/opencode/src/plugin/digitalocean.ts:1`:
  ```ts
  import type { Hooks, PluginInput } from "@opencode-ai/plugin"
  ```

### 4.4 Where the published packages live

- `packages/plugin/package.json` defines `@opencode-ai/plugin` and depends on `@opencode-ai/sdk` via `"workspace:*"` (`packages/plugin/package.json:20`).
- `packages/sdk/` defines `@opencode-ai/sdk`.

### 4.5 npm publication status (medium confidence)

Whether `@opencode-ai/plugin` and `@opencode-ai/sdk` are published to the public npm registry (vs. `private: true` workspace packages) was not directly verified. The presence of `InstallationVersion` in `config.ts:436` strongly implies a published version exists, but this should be confirmed separately if you intend to rely on `npm install` from the registry.

---

## 5. Multi-Plugin Repo Patterns

Given the §1 constraints (non-recursive auto-discovery, no `plugin.paths`), the supported structures for a cloned repo are:

### Option A — Flat single-file plugins (auto-discoverable)

Clone or symlink plugins such that each sits as a top-level `.ts`/`.js` file directly inside `plugin/` or `plugins/`:
```
~/.config/opencode/plugin/
  acuity-plugin.ts          <- auto-discovered
  calendly-plugin.ts        <- auto-discovered
```
- Requires either multiple single-file repos or symlinking from a cloned subdirectory.
- A multi-file repo cloned as a subdirectory is **not** auto-discoverable.

### Option B — Cloned subdirectory with explicit `plugin:` entries (most flexible)

Clone the repo anywhere (e.g. `~/.config/opencode/plugin/cue-plugins/`) and declare each plugin file explicitly:
```jsonc
{
  "plugin": [
    "./plugin/cue-plugins/src/acuity-plugin.ts",
    "./plugin/cue-plugins/src/calendly-plugin.ts"
  ]
}
```
- Works for any directory layout in the repo
- Non-opencode files in the repo are inert
- Each entry resolved against the declaring `opencode.json` directory
- No depth or naming restrictions

### Option C — Directory spec with `package.json` entrypoint (single plugin per directory)

Place a `package.json` at the repo root declaring the entrypoint:
```jsonc
// ~/.config/opencode/plugin/cue-plugins/package.json
{
  "name": "cue-plugins",
  "exports": { "./server": "./src/acuity-plugin.ts" }
}
```
Then register the directory:
```jsonc
{ "plugin": ["./plugin/cue-plugins"] }
```
- Limited to **one** entrypoint per directory
- `exports['./server']` is consulted before `main`

### Option D — Directory spec with `index.ts` (single plugin per directory)

If no `package.json` is present, opencode looks for `index.ts`/`index.tsx`/`index.js`/`index.mjs`/`index.cjs` at the directory root. The repo's root `index.ts` can re-export or compose internal modules into a single registration.

### Global vs. project-scoped loading

Note that the declaring config file determines load scope:
- An entry in `~/.config/opencode/opencode.json` (global) loads the plugin for **every** opencode session on the machine.
- An entry in `<project>/opencode.json` or `<project>/.opencode/opencode.json` (project-local) loads the plugin only for sessions in that project.

---

## 6. Cross-Cutting Notes

1. **Dependency availability asymmetry.** Plugins inside `.opencode/plugin/` get `@opencode-ai/plugin` auto-installed for free (§4.1 fact 1). Plugins referenced by file path from a sibling repo do **not**. Either install those packages in the plugin's own repo or restrict to `import type` usage.
2. **No transpilation step.** Your `.ts` file is loaded verbatim by Bun. Whatever Bun's current TypeScript support accepts is what runs.
3. **`verbatimModuleSyntax` / `isolatedModules`** were not found set explicitly in `packages/opencode/tsconfig.json`. Your own `tsconfig.json` settings govern only your type-checking, not runtime behavior.
4. **Auto-discovery vs. `plugin:` array are independent.** They do not consult each other. A plugin in a non-default location requires an explicit `plugin:` entry; it will never be auto-discovered.

---

## 7. Confidence Notes & Gaps

1. **(medium)** No in-tree examples of directory-spec plugins were found; opencode's internal plugins use different load mechanisms (built-in plugin index, npm specs). The directory resolution algorithm is verified from source, but real-world usage patterns are inferred.
2. **(medium)** Whether `@opencode-ai/plugin` and `@opencode-ai/sdk` are published to the public npm registry was not directly verified.
3. **(medium)** The exact `moduleResolution` setting in the base `@tsconfig/bun/tsconfig.json` was not read — only the `extends` reference at `packages/opencode/tsconfig.json:3`.
4. **(low)** The behavior when a directory contains both `package.json` and `index.ts`, AND the `package.json` declares no valid entrypoint — the fallback path in `resolvePluginEntrypoint` (`packages/opencode/src/plugin/shared.ts:159-166`) is condition-dependent and was not exhaustively traced.
5. **(not investigated)** Whether a single module can register multiple plugins by exporting an array, factory, or composition root. The hook-surface shape (`Plugin = (input, options?) => Promise<Hooks>`) suggests one default export per module, but the multi-registration question was not probed.

---

## 8. Methodology

- **Phase 1 (breadth, very thorough):** Single `@explore` subagent mapped the plugin system end-to-end across `packages/opencode/src/plugin/`, `packages/opencode/src/config/`, `packages/plugin/`, and `packages/sdk/`.
- **Phase 2 (depth) — round 1:** Focused `@explore` subagent verified three edge cases specific to out-of-`.opencode` plugin placement: cross-directory module resolution, type-only import behavior at runtime, and relative-path spec resolution anchor.
- **Phase 2 (depth) — round 2:** Focused `@explore` subagent verified directory-spec resolution, multi-plugin repo support, and the absence of any `plugin.paths` config option.
- All claims are grounded in code at absolute paths under `/home/pl/.config/opencode/ref/opencode/`. No skill content was cited as a source (the `customize-opencode` skill was used only as orientation).

### Supersedes

- `opencode-plugin-typescript-guide.md` (2026-06-21)
- `opencode-plugin-discovery-and-multi-repo.md` (2026-06-21)

Those two prior snapshots remain in the doc store as historical record; this consolidated guide is the canonical reference.

