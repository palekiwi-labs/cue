---
status: open
priority: low
refs: undefined
---
# Reconsider ProjectStore: Vec<PathBuf> -> single project path

`ProjectStore` maps a project key to `Vec<PathBuf>`, allowing one project to
live in multiple checkout locations. During the kanban multi-project design we
decided this is a footgun: reading from multiple checkouts produces duplicate
cards and complicates the "first path only" rule adopted for the kanban
(D3 in kanban-multi-project.md).

Consider changing the data model from `Vec<PathBuf>` to a single `PathBuf` per
key. This would simplify:
- `ProjectStore::add_path` semantics (replace vs reject)
- the `projects.json` schema (degenerate arrays of length 1)
- consumers (curator kanban, future Projects view) that assume one path per key

Gated on confirming no real-world workflow relies on multiple checkouts per
project key. If pursued, this is a breaking change to `projects.json` (acceptable
at prototyping stage).
