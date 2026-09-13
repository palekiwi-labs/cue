use anyhow::Context;
use std::io::BufRead;
use std::path::Path;

pub fn handle(
    cwd: &Path,
    entries: Vec<String>,
    context: Option<String>,
    store_root: Option<&Path>,
) -> anyhow::Result<()> {
    let mut stdin_consumed = false;
    let mut expanded_entries = Vec::new();
    for entry in entries {
        if entry != "-" {
            expanded_entries.push(entry);
            continue;
        }
        if stdin_consumed {
            continue;
        }

        stdin_consumed = true;
        for line in std::io::stdin().lock().lines() {
            let line = line.context("Failed to read entries from stdin")?;
            if !line.trim().is_empty() {
                expanded_entries.push(line);
            }
        }
    }

    let rendered = crate::render::render(
        cwd,
        crate::render::RenderOptions {
            entries: expanded_entries,
            scope_name: context,
            store_root: store_root.map(Path::to_path_buf),
        },
    )?;
    print!("{rendered}");
    Ok(())
}
