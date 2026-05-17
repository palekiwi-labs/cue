use crate::context::gather_context;
use crate::git::get_git_root;
use std::path::Path;

pub fn handle(cwd: &Path, profile: Option<String>) -> anyhow::Result<()> {
    let git_root = get_git_root(cwd)?;
    let resolved = gather_context(cwd, profile.as_deref())?;

    for artifact in resolved.artifacts {
        let relative_path = artifact
            .path
            .strip_prefix(&git_root)
            .unwrap_or(&artifact.path);
        let normalized_path = relative_path.display().to_string().replace('\\', "/");

        println!(
            "<artifact path=\"{}\">\n{}\n</artifact>\n",
            normalized_path, artifact.content
        );
    }

    if let Some(diff_output) = resolved.diff {
        // We don't easily have the original diff_args here anymore, but we can reconstruct it
        // or just omit the attribute if it's not strictly needed for the XML.
        // Actually, let's just use a generic "resolved" for now or fix gather_context to return it.
        println!("<diff>\n{}\n</diff>", diff_output);
    }

    Ok(())
}
