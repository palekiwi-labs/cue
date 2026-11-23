use crate::cli::UpdateArgs;
use cuelib::update::{Parent, Refs, Request};
use serde_yaml::Value;
use std::path::Path;

pub fn handle(args: UpdateArgs, store_root: Option<&Path>) -> anyhow::Result<()> {
    let refs = refs(&args)?;
    let parent = match (args.parent, args.no_parent) {
        (Some(_), true) => anyhow::bail!("--parent cannot be used with --no-parent"),
        (Some(parent), false) => Some(Parent::Set(parent)),
        (None, true) => Some(Parent::Clear),
        (None, false) => None,
    };
    let request = Request {
        title: args.title,
        status: args.status,
        priority: args.priority,
        description: args.description,
        kind: args.kind,
        mode: args.mode,
        parent,
        refs,
    };
    let store = cuelib::store::root(store_root)?;
    let mutation = cuelib::update::update(&store, &args.address, &request, args.dry_run)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&mutation)?);
    } else {
        for change in &mutation.changes {
            println!(
                "{} {}: {} -> {}",
                mutation.address,
                change.field,
                display(change.old.as_ref()),
                display(change.new.as_ref())
            );
        }
    }
    Ok(())
}

fn refs(args: &UpdateArgs) -> anyhow::Result<Refs> {
    let flags = [
        ("--add-ref", !args.add_ref.is_empty()),
        ("--remove-ref", !args.remove_ref.is_empty()),
        ("--set-refs", !args.set_refs.is_empty()),
        ("--clear-refs", args.clear_refs),
    ];
    let used: Vec<&str> = flags
        .iter()
        .filter(|(_, on)| *on)
        .map(|(flag, _)| *flag)
        .collect();
    if let Some(replacing) = used
        .iter()
        .find(|flag| matches!(**flag, "--set-refs" | "--clear-refs"))
        && let Some(other) = used.iter().find(|flag| *flag != replacing)
    {
        let (first, second) = if other.starts_with("--add") || other.starts_with("--remove") {
            (other, replacing)
        } else {
            (replacing, other)
        };
        anyhow::bail!("{first} cannot be used with {second}");
    }
    Ok(if args.clear_refs {
        Refs::Clear
    } else if !args.set_refs.is_empty() {
        Refs::Set(args.set_refs.clone())
    } else {
        Refs::Edit {
            add: args.add_ref.clone(),
            remove: args.remove_ref.clone(),
        }
    })
}

fn display(value: Option<&Value>) -> String {
    match value {
        None => "(none)".to_string(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Sequence(items)) => format!(
            "[{}]",
            items
                .iter()
                .map(|item| display(Some(item)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Some(other) => serde_yaml::to_string(other)
            .unwrap_or_default()
            .trim_end()
            .to_string(),
    }
}
