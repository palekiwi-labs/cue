# Extension Points Identification

## 1. Configuration Analysis (`src/config.rs` & `src/cli.rs`)
The current configuration system is focused on directory and branch naming conventions.

- **Config Loading:** Configuration is loaded using `Figment`, merging defaults, global JSON (`~/.config/mem/mem.json`), project JSON (`mem.json`), and environment variables (`MEM_`).
- **Support for Aliases/Custom Commands:** Currently **no support** for aliases or custom command definitions in `Config`.
- **Snippet (`src/config.rs:24-39`):**
```rust
pub fn load(project_root: &Path) -> anyhow::Result<Self> {
    let mut builder = Figment::from(Serialized::defaults(Config::default()));

    if let Ok(home) = std::env::var("HOME") {
        let global_config = Path::new(&home).join(".config/mem/mem.json");
        builder = builder.merge(Json::file(global_config));
    }

    let project_config = project_root.join("mem.json");
    let config = builder
        .merge(Json::file(project_config))
        .merge(Env::prefixed("MEM_"))
        .extract()?;

    Ok(config)
}
```

## 2. Content Handling Analysis (`src/commands/add.rs` & `src/commands/log/add.rs`)
Content resolution is currently split between `src/main.rs` and the command handlers.

- **`mem add` (General Artifacts):** Content resolution happens in `src/main.rs`. It supports raw string arguments, files, clipboard, and stdin (`-`).
- **`mem log add` (Project Log):** Content resolution happens in `src/commands/log/add.rs`. It supports raw arguments or a JSON file.
- **Fetchers Potential:** The logic in `src/main.rs` for `Commands::Add` is already a candidate for generalization. It resolves various inputs into a `Vec<u8>`. A "fetcher" abstraction could replace the inline `if clipboard ... else if file ...` logic.
- **Snippet (`src/main.rs:28-43`):**
```rust
let resolved_content: Vec<u8> = if clipboard {
    resolve_clipboard(&filename)?
} else if let Some(path) = file {
    std::fs::read(&path).with_context(|| format!("Failed to read file {}", path))?
} else {
    let c = content.unwrap_or_else(|| "-".to_string());
    if c == "-" {
        let mut buf = Vec::new();
        io::stdin()
            .read_to_end(&mut buf)
            .context("Failed to read from stdin")?;
        buf
    } else {
        c.into_bytes()
    }
};
```

## 3. External Process Execution
The codebase is very conservative with external processes.

- **Findings:** Only `git` is executed via `std::process::Command` in `src/git.rs`. All other matches for `Command::new` are in test files.
- **Pattern:** `src/git.rs` uses a helper `run_git` to execute `git` commands.
- **Snippet (`src/git.rs:12-25`):**
```rust
let output = Command::new("git")
    .args(args)
    .current_dir(cwd)
    .output()
    .context("Failed to execute git command")?;

if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    bail!("git command failed: {}", stderr.trim());
}

Ok(output)
```

## 4. Potential Extension Points
1. **Configurable Fetchers:** Extend `Config` to allow custom shell commands to "fetch" content for `mem add`.
2. **Command Aliases:** Add an `aliases` map to `Config` to allow users to define shorthand for common `mem` operations.
3. **External Plugins:** If `mem` needs to support domain-specific logic (e.g., fetching Jira tickets for `log add`), a "shell-out" fetcher pattern in `Config` would be the path of least resistance.
