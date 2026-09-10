use std::path::Path;

pub fn handle(
    cwd: &Path,
    entries: Vec<String>,
    context: Option<String>,
    store_root: Option<&Path>,
) -> anyhow::Result<()> {
    let rendered = crate::render::render(
        cwd,
        crate::render::RenderOptions {
            entries,
            scope_name: context,
            store_root: store_root.map(Path::to_path_buf),
        },
    )?;
    print!("{rendered}");
    Ok(())
}
