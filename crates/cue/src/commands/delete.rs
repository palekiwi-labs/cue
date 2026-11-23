use std::path::Path;

pub fn handle(
    address: String,
    dry_run: bool,
    json: bool,
    store_root: Option<&Path>,
) -> anyhow::Result<()> {
    let store = cuelib::store::root(store_root)?;
    let deletion = cuelib::delete::delete(&store, &address, dry_run)?;
    if !dry_run {
        crate::hooks::dispatch_delete(&deletion.address, &deletion.mutation_id, &store);
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&deletion)?);
        return Ok(());
    }
    let verb = if dry_run {
        "would be deleted"
    } else {
        "deleted"
    };
    println!("{} {verb}", deletion.address);
    for log in &deletion.warnings {
        eprintln!("warning: trace named by {log}");
    }
    Ok(())
}
