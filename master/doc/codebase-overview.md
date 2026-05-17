# Codebase Overview: mem

## Purpose

`mem` is a Command Line Interface (CLI) tool designed to manage **Agent Memory** and maintain context across AI agent sessions. It provides a structured framework for recording intent (what is being done) and history (what was discovered) within a Git-tracked project.

The tool ensures that AI agents have access to a persistent, branch-isolated memory that transcends individual chat sessions, preventing redundant research and maintaining continuity in complex tasks.

## Core Concepts

### 1. Intent vs. History

The system distinguishes between stable goals and cumulative findings:

- **Spec (Intent)**: Stable context including goals (`index.md`), technical plans (`plan.md`), and roadmaps (`todo.md`).
- **Log (History)**: A cumulative record of discoveries, decisions, and open questions managed via `mem log`.

### 2. Git Isolation

A key architectural feature is the isolation of memory artifacts from the main codebase.

- **Orphan Branch**: Artifacts are stored on a separate orphan branch (usually named `mem`).
- **Git Worktree**: This branch is mounted to the `.mem/` directory via a worktree, allowing metadata to be versioned independently of the source code.
- **Branch-Specific Contexts**: Memory is partitioned by Git branch names to ensure that different features have isolated contexts.

## Technical Architecture

### Key Entry Points

The application is written in Rust and uses the `clap` crate for command parsing.

- **Entry Point**: `src/main.rs`
- **Subcommand Handling**: `src/commands/` (e.g., `init.rs`, `add.rs`, `list.rs`, `log/`)

### Artifact Types

Artifacts are categorized to maintain hygiene:

- `spec`: Stable context and execution roadmap.
- `log`: Historical logs and milestones.
- `doc`: Long-form research and documentation.
- `trace` / `tmp`: Ephemeral or commit-linked execution data.

### CLIP & Image Support

The tool can capture content from `stdin`, files, or the system clipboard—including image data which is saved as binary artifacts.

## Sourced Findings

### Initialization and Isolation

The `init` command sets up the orphan branch and worktree.

- **File**: `/home/pl/code/palekiwi-labs/mem/src/commands/init.rs`
- **Snippet**:
  ```rust
  // src/commands/init.rs:63
  } else {
      git::add_worktree_orphan(root, mem_path, branch)?;
      // Initialize orphan branch with .gitignore and .rgignore
  }
  ```

### Artifact Storage Logic

Artifacts are placed in directories corresponding to the sanitized branch name.

- **File**: `/home/pl/code/palekiwi-labs/mem/src/commands/add.rs`
- **Snippet**:
  ```rust
  // src/commands/add.rs:41
  let branch_dir = branch.replace(['/', '\\'], "-");
  // ...
  let dest_dir = match mem_type {
      MemType::Spec => mem_path.join(&branch_dir).join("spec"),
      // ...
  };
  ```

### CLI Command Structure

The main loop dispatches to specific command handlers.

- **File**: `/home/pl/code/palekiwi-labs/mem/src/main.rs`
- **Snippet**:
  ```rust
  // src/main.rs:105
  match cli.command {
      Commands::Init => { commands::init::handle(&cwd)?; }
      Commands::Add { ... } => {
          commands::add::handle(&cwd, &filename, resolved_content, mem_type, force, branch)?;
      }
      Commands::List { ... } => { commands::list::handle(&cwd, ...)?; }
      Commands::Log { command } => { commands::log::handle(&cwd, command)?; }
  }
  ```
