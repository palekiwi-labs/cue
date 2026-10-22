//! The context hierarchy, built from `parent` edges alone.
//!
//! The tree is built over an already selected set of contexts, so an edge to
//! a context outside that set is not an error: the child stands as a root.
//! `refs` never shape the tree.

use super::ContextListEntry;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::HashMap;

#[derive(Serialize)]
pub(super) struct Node {
    #[serde(flatten)]
    entry: ContextListEntry,
    children: Vec<Node>,
}

#[derive(Serialize)]
pub(super) struct Diagnostic {
    kind: &'static str,
    contexts: Vec<String>,
}

#[derive(Serialize)]
pub(super) struct Tree {
    roots: Vec<Node>,
    diagnostics: Vec<Diagnostic>,
}

fn address(entry: &ContextListEntry) -> String {
    format!("{}/{}", entry.scope, entry.context)
}

/// Build the forest over `contexts`, which arrive in canonical address order.
///
/// A cycle is an illegal state, but one bad edge must not break a picker:
/// every member of a cycle loses its parent edge and stands as a root, and the
/// cycle is reported as a diagnostic.
pub(super) fn build(
    contexts: Vec<ContextListEntry>,
    root: Option<&str>,
    recency: bool,
) -> anyhow::Result<Tree> {
    let addresses: Vec<String> = contexts.iter().map(address).collect();
    let index: HashMap<&str, usize> = addresses
        .iter()
        .enumerate()
        .map(|(i, address)| (address.as_str(), i))
        .collect();

    let mut parent_of: Vec<Option<usize>> = contexts
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            entry
                .parent
                .as_deref()
                .and_then(|parent| index.get(parent).copied())
                .filter(|&parent| parent != i)
        })
        .collect();

    let diagnostics = break_cycles(&mut parent_of, &addresses);

    let mut children: Vec<Vec<usize>> = vec![Vec::new(); contexts.len()];
    let mut roots = Vec::new();
    for (i, parent) in parent_of.iter().enumerate() {
        match parent {
            Some(parent) => children[*parent].push(i),
            None => roots.push(i),
        }
    }

    if recency {
        let mut newest = vec![None; contexts.len()];
        for &root in &roots {
            subtree_newest(root, &children, &contexts, &mut newest);
        }
        // Stable over canonical address order, so ties keep that order.
        let key = |i: &usize| Reverse(newest[*i]);
        roots.sort_by_key(key);
        for siblings in &mut children {
            siblings.sort_by_key(key);
        }
    }

    if let Some(root) = root {
        let selected = index.get(root).copied().ok_or_else(|| {
            anyhow::anyhow!("root context {root} is not among the selected contexts")
        })?;
        roots = vec![selected];
    }

    let mut entries: Vec<Option<ContextListEntry>> = contexts.into_iter().map(Some).collect();
    let roots = roots
        .into_iter()
        .map(|i| assemble(i, &children, &mut entries))
        .collect();
    Ok(Tree { roots, diagnostics })
}

/// Detect every cycle, drop its members' parent edges, and report it.
fn break_cycles(parent_of: &mut [Option<usize>], addresses: &[String]) -> Vec<Diagnostic> {
    const UNSEEN: u8 = 0;
    const ON_PATH: u8 = 1;
    const DONE: u8 = 2;
    let mut state = vec![UNSEEN; parent_of.len()];
    let mut cycles = Vec::new();
    for start in 0..parent_of.len() {
        let mut path = Vec::new();
        let mut current = Some(start);
        while let Some(i) = current {
            match state[i] {
                UNSEEN => {
                    state[i] = ON_PATH;
                    path.push(i);
                    current = parent_of[i];
                }
                ON_PATH => {
                    let from = path.iter().position(|&p| p == i).unwrap_or(0);
                    cycles.push(path[from..].to_vec());
                    break;
                }
                _ => break,
            }
        }
        for i in path {
            state[i] = DONE;
        }
    }

    cycles
        .into_iter()
        .map(|mut members| {
            for &member in &members {
                parent_of[member] = None;
            }
            members.sort_unstable();
            Diagnostic {
                kind: "cycle",
                contexts: members.iter().map(|&i| addresses[i].clone()).collect(),
            }
        })
        .collect()
}

fn subtree_newest(
    i: usize,
    children: &[Vec<usize>],
    contexts: &[ContextListEntry],
    newest: &mut [Option<u64>],
) -> Option<u64> {
    let mut value = contexts[i].last_logged_nanos;
    for &child in &children[i] {
        value = value.max(subtree_newest(child, children, contexts, newest));
    }
    newest[i] = value;
    value
}

fn assemble(i: usize, children: &[Vec<usize>], entries: &mut [Option<ContextListEntry>]) -> Node {
    let entry = entries[i].take().expect("each context is placed once");
    let children = children[i]
        .iter()
        .map(|&child| assemble(child, children, entries))
        .collect();
    Node { entry, children }
}

pub(super) fn print(tree: &Tree, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(tree)?);
        return Ok(());
    }
    for diagnostic in &tree.diagnostics {
        eprintln!(
            "warning: parent {}: {}",
            diagnostic.kind,
            diagnostic.contexts.join(", ")
        );
    }
    for root in &tree.roots {
        println!("{}", address(&root.entry));
        print_children(&root.children, "");
    }
    Ok(())
}

fn print_children(nodes: &[Node], prefix: &str) {
    for (position, node) in nodes.iter().enumerate() {
        let last = position + 1 == nodes.len();
        let (branch, indent) = if last {
            ("└─ ", "   ")
        } else {
            ("├─ ", "│  ")
        };
        println!("{prefix}{branch}{}", address(&node.entry));
        print_children(&node.children, &format!("{prefix}{indent}"));
    }
}
