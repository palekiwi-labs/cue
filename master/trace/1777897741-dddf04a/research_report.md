# Research Report: Extensibility and Domain-Specific Commands in `mem`

## 1. Executive Summary
The current `mem` implementation in Rust is a focused, CLI-driven tool that manages project context by creating and listing files in a dedicated `.mem` directory structure. Its architecture is currently **static and closed to external extensions**. However, the codebase exhibits clear patterns that could be generalized to support domain-specific commands or a plugin-like system without compromising its core responsibility.

## 2. Current State Analysis

### 2.1 Command Dispatch (`src/main.rs`, `src/cli.rs`)
Commands are defined declaratively using `clap` and dispatched via exhaustive `match` statements.
- **Source:** `src/cli.rs:19-64` (Commands definition)
- **Source:** `src/main.rs:26-38` (Match statement)

```rust
// src/main.rs snippet
match cli.command {
    Commands::Init => { commands::init::handle(&cwd)?; }
    Commands::Add { ... } => { ... }
    // ...
}
```

### 2.2 Content Resolution (`src/main.rs`)
The `mem add` command has a resolution phase where it determines the content from stdin, a file, or the clipboard.
- **Source:** `src/main.rs:28-51`

```rust
let resolved_content: Vec<u8> = if clipboard {
    resolve_clipboard(&filename)?
} else if let Some(path) = file {
    std::fs::read(&path)...
} else {
    // Stdin or raw string
}
```

### 2.3 Configuration (`src/config.rs`)
Configuration is managed via `Figment`, supporting multiple layers. Currently, it only stores `branch_name` and `dir_name`.
- **Source:** `src/config.rs:10-21`

## 3. Potential Extension Points

### 3.1 Generalized "Fetchers"
The content resolution logic in `src/main.rs` could be abstracted into a "Fetcher" trait. This would allow adding new ways to retrieve content (e.g., from an external command) easily.

### 3.2 Configuration Hooks / Aliases
Adding an `aliases` or `hooks` map to the `Config` struct would allow users to define domain-specific commands in their `mem.json`.
- **Location:** `src/config.rs`
- **Mechanism:** `HashMap<String, String>` mapping a name to a shell command.

### 3.3 External Subcommands
Following the `git` or `kubectl` pattern, `mem` could attempt to execute binaries named `mem-<subcommand>` found in the user's `PATH`.

## 4. Architectural Considerations (SRP)
The "Single Responsibility Principle" for a Unix tool like `mem` is to be an efficient "context sink".
- **Supporting specific domains (GitHub, RSpec)** directly in the core might violate SRP.
- **Providing a generic interface for external fetchers** maintains SRP by keeping `mem` focused on *storage* and *organization*, while delegating *acquisition* to specialized tools.

## 5. Sourced Artifacts
Detailed trace artifacts generated during this research:
- `.mem/master/trace/1777897741-dddf04a/domain_mapping.md`
- `.mem/master/trace/1777897741-dddf04a/extension_points_identification.md`
