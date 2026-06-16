# Research Report: Implementation of `mem config show`

## Objective
Provide the necessary technical details to implement the `mem config show` subcommand, which prints the resolved configuration as JSON.

## Findings

### 1. Configuration Management
The configuration is defined in `src/config.rs`. The `Config` struct derives `Serialize` and `Deserialize`.

- **File**: `/home/pl/code/palekiwi-labs/mem/src/config.rs`
- **Struct**: `Config`
- **Method**: `Config::load(project_root: &Path) -> Result<Config>` loads the configuration by merging defaults, global config, project config, and environment variables.

```rust
// src/config.rs
#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    pub branch_name: String,
    pub dir_name: String,
    #[serde(default)]
    pub artifact_types: Vec<String>,
    #[serde(default)]
    pub ignored_types: Vec<String>,
    #[serde(default)]
    pub context: ContextConfig,
}
```

### 2. CLI Architecture
The CLI is built using `clap` in `src/cli.rs`. Dispatching happens in `src/main.rs`.

- **File**: `/home/pl/code/palekiwi-labs/mem/src/cli.rs`
- **Enum**: `Commands` (Add `Config` variant)
- **New Enum**: `ConfigCommands` (Add `Show` variant)

```rust
// Proposed change to src/cli.rs
#[derive(Subcommand)]
pub enum Commands {
    // ... existing variants
    Config {
        #[clap(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    Show,
}
```

### 3. Subcommand Implementation Pattern
Existing subcommands follow a pattern of having a `handle` function in `src/commands/<name>.rs`.

- **Pattern**:
    - Load the required data.
    - Serialize to JSON using `serde_json::to_string_pretty`.
    - Print to stdout.

```rust
// Example from src/commands/context.rs
pub fn handle(cwd: &Path, command: ContextCommands) -> Result<()> {
    match command {
        ContextCommands::Show => {
            let config = load_context_config(cwd)?;
            println!("{}", serde_json::to_string_pretty(&config)?);
        }
    }
}
```

## Proposed Implementation Steps
1. **Modify `src/cli.rs`**: Add `Config` and `ConfigCommands`.
2. **Create `src/commands/config.rs`**: Implement `handle` function.
3. **Update `src/commands/mod.rs`**: Add `pub mod config;`.
4. **Update `src/main.rs`**: Add match arm for `Commands::Config`.

## Confidence Note
High. The codebase patterns are consistent across `list` and `context` subcommands for JSON output.
