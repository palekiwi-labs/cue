# Research Report: Artifact Restructure Context

## Research Question
What is the current state of artifact management in the `mem` CLI and how will the proposed restructuring (unifying storage paths and introducing a `--traced` flag) affect the codebase?

## Current State Analysis

Currently, the `mem` CLI hardcodes the storage hierarchy based on the `MemType` enum.

### Storage Logic in `add` Command
The destination directory for an artifact is determined by its type in `src/commands/add.rs`.

**File:** `/home/pl/code/palekiwi-labs/mem/src/commands/add.rs`
**Symbol:** `handle`
**Snippet:**
```rust
    // 6. Resolve destination directory
    let dest_dir = match mem_type {
        MemType::Spec => mem_path.join(&branch_dir).join("spec"),
        MemType::Ref => mem_path.join(&branch_dir).join("ref"),
        MemType::Bin => mem_path.join(&branch_dir).join("bin"),
        MemType::Doc => mem_path.join(&branch_dir).join("doc"),
        MemType::Trace | MemType::Tmp => {
            let ts = git::get_head_timestamp(&root)?;
            let hash = git::get_short_head_hash(&root)
                .context("Could not determine HEAD hash. Have you made your first commit yet?")?;
            let type_dir = if matches!(mem_type, MemType::Trace) {
                "trace"
            } else {
                "tmp"
            };
            mem_path
                .join(&branch_dir)
                .join(type_dir)
                .join(format!("{}-{}", ts, hash))
        }
    };
```
- `Spec`, `Ref`, `Bin`, and `Doc` are saved directly in a subdirectory named after their type.
- `Trace` and `Tmp` are automatically nested under a directory named `<timestamp>-<short-hash>`.

### Discovery Logic in `list` Command
The `list` command relies on this hardcoded structure to identify and parse artifacts.

**File:** `/home/pl/code/palekiwi-labs/mem/src/commands/list.rs`
**Symbol:** `to_mem_file`
**Snippet:**
```rust
        // 5. Try to parse timestamp and hash for trace/tmp
        let mut timestamp = None;
        let mut hash = None;

        if (category == "trace" || category == "tmp") && comp_count >= 4 {
            if let Some(folder_name) = path.components().nth(2) {
                let folder_str = folder_name.as_os_str().to_string_lossy();
                if let Some((ts_str, hash_str)) = folder_str.split_once('-') {
                    timestamp = ts_str.parse::<u64>().ok();
                    hash = Some(hash_str.to_string());
                }
            }
        }
```
The logic explicitly checks if the category is `trace` or `tmp` before attempting to parse the timestamp and hash from the parent directory.

## Impact of Proposed Changes

### 1. Unified Storage Path
The proposal is to save all artifacts under `.mem/<branch-name>/<type>/<filename>`.
- **Breaking Change for `Trace`/`Tmp`**: Currently, they are nested. Making them flat by default will break the `list` command's metadata parsing (it expects `comp_count >= 4`).
- **Breaking Change for `list` command**: The `list` command's `is_valid_mem_file` and `to_mem_file` functions will need to be updated to handle flat files for all types.

### 2. Introduction of `--traced` Flag
The proposal is to allow *any* artifact type to be "traced" (nested under timestamp and hash).
- **Update to `list` command**: The metadata parsing logic must be decoupled from the `category` check. It should attempt to parse the parent directory as a `timestamp-hash` for *any* type if the directory structure matches.
- **Update to `context` module**: The `init_context` function in `src/context/mod.rs` assumes `spec/` is a flat directory. It will need to be updated to traverse nested directories if "traced" specs are introduced.

### 3. Configuration and Extensibility
The proposal mentions allowing users to define their own supported artifacts in the config.
- **Current Config**: Defined in `src/config.rs`.
- **Current `MemType`**: An enum in `src/cli.rs`. To support user-defined types, this may need to transition from a fixed enum to a string-based type or a more dynamic registry.

## Sourced Findings Summary

- **Entry Point**: `src/cli.rs` defines `Commands::Add` and `MemType`.
- **Storage Logic**: `src/commands/add.rs` contains the `match mem_type` block that determines path nesting.
- **Discovery Logic**: `src/commands/list.rs` contains the logic for parsing `timestamp-hash` based on category.
- **Git Primitives**: `src/git.rs` provides `get_short_head_hash` and `get_head_timestamp`.
- **Context Discovery**: `src/context/mod.rs` assumes `spec/` is flat.

## Confidence Notes
- High confidence in the locations of storage and discovery logic.
- Medium confidence in the full extent of the impact on the `context` module, as it was only briefly explored.
- The `init` command's `.gitignore` generation (`src/commands/init.rs`) will definitely need review if "traced" artifacts become more common across types.
