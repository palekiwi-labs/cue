---
status: complete
---

This plan implements Phase 6 of the artifact-frontmatter feature: allowing users to embed YAML
frontmatter at artifact creation time via `mem add -f key=value`. It covers the four files that
must change (`src/cli.rs`, `src/main.rs`, `src/commands/add.rs`, `tests/add.rs`) and the exact
shape of each change, so a fresh agent can execute from here without additional research.

Prerequisites:
- Phases 1–5 are complete (serde_yaml in Cargo.toml, --frontmatter on list, env isolation in tests).
- The existing `tests/helpers.rs` provides `helpers::mem_cmd()` for subprocess isolation.

No new dependencies are required; `serde_yaml` is already in Cargo.toml.

---

## Steps

### 1. Add value parser + reclaim `-f` in `src/cli.rs`

- [x] Add a free function `parse_frontmatter_field` just above the `Commands` enum:

```rust
fn parse_frontmatter_field(s: &str) -> Result<(String, String), String> {
    s.split_once('=')
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .ok_or_else(|| format!("Expected key=value, got '{}'", s))
}
```

- [x] In the `Add` variant, change the `file` field:
  - Remove `short = 'f'` from `--file` (keep `long = "file"` only).
  - Add a new field after `clipboard`:

```rust
        /// Frontmatter fields to prepend (repeatable, key=value format)
        #[arg(short = 'f', long = "frontmatter", value_name = "KEY=VALUE",
              value_parser = parse_frontmatter_field)]
        frontmatter: Vec<(String, String)>,
```

  - Update `conflicts_with_all` on `file` to remove any reference to `clipboard` (the existing
    conflict with `content` and `clipboard` via those field names remains unchanged; just drop
    the short flag).

### 2. Update `src/main.rs` dispatch

- [x] Add `frontmatter` to the destructuring of `Commands::Add`:

```rust
        Commands::Add {
            filename,
            content,
            file,
            clipboard,
            frontmatter,   // <-- new
            mem_type,
            root,
            force,
            branch,
        } => {
```

- [x] Pass `frontmatter` as the fourth argument to `commands::add::handle(...)`:

```rust
            commands::add::handle(
                &cwd,
                &filename,
                resolved_content,
                frontmatter,   // <-- new
                mem_type,
                root,
                force,
                branch,
            )?;
```

### 3. Implement frontmatter assembly in `src/commands/add.rs`

- [x] Add `serde_json` and `serde_yaml` to the imports at the top of the file.

- [x] Add `frontmatter: Vec<(String, String)>` as the fourth parameter to `handle`, shifting
  the others down (wrapped in AddOptions struct to satisfy clippy::too_many_arguments):

```rust
pub fn handle(
    cwd: &Path,
    filename: &str,
    content: Vec<u8>,
    frontmatter: Vec<(String, String)>,
    mem_type: String,
    save_at_root: bool,
    force: bool,
    branch_name: Option<String>,
) -> Result<()> {
```

- [x] Add a private helper `build_frontmatter_bytes` below `validate_filename`:

```rust
fn build_frontmatter_bytes(fields: &[(String, String)]) -> Result<Vec<u8>> {
    use serde_yaml::Mapping;
    let mut map = Mapping::new();
    for (k, v) in fields {
        let yaml_val: serde_yaml::Value = serde_yaml::from_str(v)
            .unwrap_or_else(|_| serde_yaml::Value::String(v.clone()));
        map.insert(serde_yaml::Value::String(k.clone()), yaml_val);
    }
    let yaml_str = serde_yaml::to_string(&map)
        .context("Failed to serialize frontmatter to YAML")?;
    let mut out = b"---\n".to_vec();
    out.extend_from_slice(yaml_str.as_bytes());
    out.extend_from_slice(b"---\n");
    Ok(out)
}
```

  Note: `serde_yaml::from_str(v)` gives type coercion for free — `"true"` becomes a bool,
  `"42"` becomes an integer — matching the behaviour already established in the filter parser.

- [x] In `handle`, replace step 10 (the `fs::write` call) with:

```rust
    // 10. Assemble final content (prepend frontmatter if provided)
    let final_content = if frontmatter.is_empty() {
        content
    } else {
        let mut fm = build_frontmatter_bytes(&frontmatter)?;
        fm.extend_from_slice(&content);
        fm
    };

    // 11. Write file
    fs::write(&file_path, final_content)
        .with_context(|| format!("Failed to write to {}", file_path.display()))?;
```

  (Renumber the confirmation step comment to 12 accordingly.)

### 4. Integration tests in `tests/add.rs`

- [x] Add a test `add_with_single_frontmatter_field`:
  - Run `mem add note.md "body" -f status=todo`.
  - Read the created file.
  - Assert it starts with `---\n`.
  - Assert it contains `status: todo`.
  - Assert it ends with (or contains) `body`.

- [x] Add a test `add_with_multiple_frontmatter_fields`:
  - Run `mem add note.md "body" -f title=Hello -f priority=high`.
  - Assert both `title: Hello` and `priority: high` appear inside the `---` fences.

- [x] Add a test `add_frontmatter_type_coercion`:
  - Run `mem add note.md "" -f done=true -f count=3`.
  - Assert output contains `done: true` and `count: 3` (no quotes around the values).

- [x] Add a test `add_frontmatter_roundtrip_with_list`:
  - Create a file with `-f status=active`.
  - Run `mem list --frontmatter --json`.
  - Parse the JSON output.
  - Assert the artifact entry has `frontmatter.status == "active"`.

- [x] Run the full test suite with `cargo test -- --test-threads=1` and confirm 100% pass rate.

### 5. Smoke-test manually

- [x] Run `mem add test.md "hello" -f status=todo -f priority=high` in a real repo.
- [x] `cat` the resulting file to confirm correct YAML fences.
- [x] Run `mem list --frontmatter` and confirm the fields appear in JSON output.
- [x] Confirm `mem add --help` shows `--file` (long only) and `-f / --frontmatter`.
