# High-Level Domain Mapping: `mem` (Rust)

## 1. Command Registration and Dispatch
Commands are registered using the `clap` crate with a declarative approach in `src/cli.rs`. Dispatching occurs in `src/main.rs` via a match statement on the parsed `Commands` enum.

### Registration (`src/cli.rs`)
The `Cli` struct and `Commands` enum define the CLI structure:
```rust
#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Init,
    Add { ... },
    List { ... },
    Log { ... },
}
```

### Dispatch (`src/main.rs`)
The `main` function parses the CLI and matches on the command:
```rust
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    // ...
    match cli.command {
        Commands::Init => {
            commands::init::handle(&cwd)?;
        }
        // ...
    }
    Ok(())
}
```

## 2. Current Commands and Implementation Files

| Command | Subcommand | Implementation File |
| :--- | :--- | :--- |
| `init` | - | `src/commands/init.rs` |
| `add` | - | `src/commands/add.rs` |
| `list` | - | `src/commands/list.rs` |
| `log` | `add` | `src/commands/log/add.rs` |
| `log` | `list` | `src/commands/log/list.rs` |

The `src/commands/mod.rs` and `src/commands/log/mod.rs` files act as module aggregators.

## 3. Extension Mechanisms
Currently, the codebase uses a **static dispatch** pattern via `match` statements in `src/main.rs` and `src/commands/log/mod.rs`. 

- **No Trait-based Plugin System**: There are no traits defined for commands that would allow for dynamic registration or extension.
- **Hardcoded Commands**: Adding a new command requires updating `src/cli.rs`, `src/main.rs`, and adding a new module in `src/commands/`.
- **No External Tool Config**: There's no evidence of configuration files or mechanisms to delegate to external binaries or scripts beyond the built-in commands.

## 4. Key Files Analyzed
- `src/main.rs`: Entry point and top-level dispatch.
- `src/cli.rs`: CLI definition (Subcommands and Arguments).
- `src/commands/mod.rs`: Root module for command handlers.
