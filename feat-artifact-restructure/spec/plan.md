# Plan: Artifact Restructure

## Goal

Remove the hardcoded `MemType` enum and replace it with a dynamic, config-driven string
type system. Artifacts are saved under a `<timestamp>-<hash>` subdirectory by default.
Introduce `--root` as an explicit opt-in for saving artifacts at the root of the type
directory — intended for stable anchor documents.

## Design Decisions

- Artifact types are strings, validated against `config.artifact_types` at runtime.
- Default artifact types: `["spec", "trace", "tmp"]`.
- Unknown types produce a hard error listing valid types.
- **Default Storage**: Artifacts are saved under a `<timestamp>-<hash>` subdirectory by
  default for historical tracking.
- **Root Storage (`--root`)**: An explicit opt-in for saving artifacts directly at the
  root of the type directory. Used for stable anchor documents (e.g., `spec/index.md`).
- Ignored types (hidden from `list` unless `--include-gitignored`) are configurable via
  `config.ignored_types`, default `["tmp"]`.
- `list` detects timestamped artifacts structurally (comp_count >= 4 with a parseable
  `ts-hash` dir) rather than by type name.

## Affected Files

- `src/config.rs`
- `src/cli.rs`
- `src/commands/add.rs`
- `src/commands/list.rs`
- `src/commands/init.rs`
- `src/context/mod.rs`
- `src/main.rs` (minor signature updates)

---

## Implementation Steps

### Step 1: Update `src/config.rs`

Add two new fields to `Config`:

```rust
pub struct Config {
    pub branch_name: String,
    pub dir_name: String,
    pub artifact_types: Vec<String>,   // NEW
    pub ignored_types: Vec<String>,    // NEW
    pub context: ContextConfig,
}
```

Update `impl Default for Config`:

```rust
impl Default for Config {
    fn default() -> Self {
        Self {
            branch_name: "mem".into(),
            dir_name: ".mem".into(),
            artifact_types: vec!["spec".into(), "trace".into(), "tmp".into()],
            ignored_types: vec!["tmp".into()],
            context: HashMap::new(),
        }
    }
}
```

### Step 2: Update `src/cli.rs`

- Remove the `MemType` enum and its `ValueEnum` derive entirely.
- In `Commands::Add`: change `mem_type: MemType` to `mem_type: String`, add `pin: bool`.
- In `Commands::List`: change `mem_type: Option<MemType>` to `mem_type: Option<String>`.

New `Add` variant:
```rust
Add {
    filename: String,
    content: Option<String>,
    #[arg(short = 'f', long = "file", ...)]
    file: Option<String>,
    #[arg(short = 'c', long = "clipboard", ...)]
    clipboard: bool,
    /// Type of artifact (must be in configured artifact_types)
    #[arg(short = 't', long = "type", default_value = "spec")]
    mem_type: String,
    /// Save artifact under a <timestamp>-<hash> subdirectory
    #[arg(long)]
    pin: bool,
    #[arg(short = 'b', long)]
    branch: Option<String>,
    #[arg(long)]
    force: bool,
},
```

New `List` variant:
```rust
List {
    #[arg(long, conflicts_with = "all")]
    branch: Option<String>,
    #[arg(short = 'a', long)]
    all: bool,
    /// Filter by artifact type (any string)
    #[arg(short = 't', long = "type")]
    mem_type: Option<String>,
    #[arg(short = 'i', long)]
    include_gitignored: bool,
    #[arg(short = 'j', long)]
    json: bool,
},
```

### Step 3: Update `src/commands/add.rs`

New signature:
```rust
pub fn handle(
    cwd: &Path,
    filename: &str,
    content: Vec<u8>,
    mem_type: String,
    pin: bool,
    force: bool,
    branch_name: Option<String>,
) -> Result<()>
```

Replace the `match mem_type` block with:

```rust
// 6. Validate artifact type
if !config.artifact_types.contains(&mem_type) {
    bail!(
        "Unknown artifact type '{}'. Valid types: {}",
        mem_type,
        config.artifact_types.join(", ")
    );
}

// 7. Resolve destination directory
let type_dir = mem_path.join(&branch_dir).join(&mem_type);
let dest_dir = if pin {
    let ts = git::get_head_timestamp(&root)?;
    let hash = git::get_short_head_hash(&root)
        .context("Could not determine HEAD hash. Have you made your first commit yet?")?;
    type_dir.join(format!("{}-{}", ts, hash))
} else {
    type_dir
};
```

### Step 4: Update `src/commands/list.rs`

Update `handle` signature to accept `mem_type: Option<String>` and pass `config` to
helper functions.

Update `is_valid_mem_file`:
```rust
fn is_valid_mem_file(
    path: &Path,
    mem_path: &Path,
    mem_type: Option<&str>,
    include_gitignored: bool,
    ignored_types: &[String],
) -> bool
```

Replace the hardcoded `tmp`/`ref` ignore check with:
```rust
} else if !include_gitignored && ignored_types.iter().any(|t| t == category.as_ref()) {
    return false;
}
```

Replace the hardcoded `MemType` match in the type filter with a direct string comparison:
```rust
if let Some(requested) = mem_type {
    if category != requested {
        return false;
    }
}
```

Update `to_mem_file`: remove the `category == "trace" || category == "tmp"` guard.
Any category with `comp_count >= 4` and a parseable `ts-hash` component is treated as
pinned:

```rust
// Detect pinned artifacts structurally (any category)
let comp_count = rel_to_mem.components().count();
if comp_count >= 4 {
    // attempt to parse the 3rd component as <timestamp>-<hash>
    let mut comps = rel_to_mem.components();
    comps.next(); // branch
    comps.next(); // category
    if let Some(ts_hash_dir) = comps.next() {
        let ts_hash_str = ts_hash_dir.as_os_str().to_string_lossy();
        if let Some((ts_str, hash_str)) = ts_hash_str.split_once('-')
            && let Ok(ts) = ts_str.parse::<u64>()
        {
            mem_file.commit_timestamp = ts;
            mem_file.hash = Some(hash_str.to_string());
            mem_file.commit_hash = Some(hash_str.to_string());
            let prefix = mem_path.join(&branch).join(&category).join(ts_hash_dir);
            if let Ok(rel_name) = path.strip_prefix(&prefix) {
                mem_file.name = rel_name.to_string_lossy().to_string();
            }
            return Some(mem_file);
        }
    }
}
// Flat artifact: name is relative to category dir
let prefix = mem_path.join(&branch).join(&category);
if let Ok(rel_name) = path.strip_prefix(&prefix) {
    mem_file.name = rel_name.to_string_lossy().to_string();
}
```

### Step 5: Update `src/commands/init.rs`

Replace the hardcoded `.gitignore` and `.rgignore` generation with config-driven output:

```rust
let gitignore_lines: String = config.ignored_types
    .iter()
    .map(|t| format!("*/{}/\n", t))
    .collect();
fs::write(mem_path.join(".gitignore"), &gitignore_lines)?;

let rgignore_lines: String = config.ignored_types
    .iter()
    .map(|t| format!("!*/{}/\n", t))
    .collect();
fs::write(mem_path.join(".rgignore"), &rgignore_lines)?;
```

### Step 6: Update `src/context/mod.rs`

Update the `init_context` fallback discovery to recurse into subdirectories of `spec/`,
so pinned spec artifacts are also discovered. Replace the flat `read_dir` + `is_file`
loop with a recursive file collection (reuse the `collect_files` pattern from `list.rs`
or extract it into a shared utility).

### Step 7: Update `src/main.rs`

Update the `Commands::Add` dispatch to pass `pin` and the new `mem_type: String`.
Update the `Commands::List` dispatch to pass `mem_type: Option<String>`.

---

## Notes and Follow-ups

- The `opencode` `mem-add` skill tool (in `~/.config/opencode/skills/mem`) currently
  calls `mem add --type trace ...` and `mem add --type tmp ...` expecting automatic
  nesting. After this refactor, those calls will save flat. The skill must be updated to
  add `--pin` for trace and tmp invocations.
- Existing `.gitignore` files in already-initialized `.mem/` worktrees will still have
  `*/ref/` listed. This is harmless but may be cleaned up manually or via a future
  migration command.
- Tests for `config.rs` should cover: default field values, JSON override of
  `artifact_types`, env var override.
- Tests for `add.rs` should cover: valid type accepted, unknown type rejected with
  message, `--pin` produces ts-hash subdir, flat save without `--pin`.
- Tests for `list.rs` should cover: pinned artifacts of any type are discovered and
  parsed, ignored types respect `config.ignored_types`.
