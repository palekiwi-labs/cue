---
status: closed
dissolved_into: "master/spec/acumen/index.md"
---
We currently use `cue context` subcommand to inject a manually
composed combination of files into the context of the agent.

Could we imagine a way to levarage the use of a graph database
like `neo4j` to create a richer, queryable and progressively
discoverable contexts?

For example:
- we work on a feature that references a branch that has already
  been merged but the "cue" context of that branch remain in ".cue/"
  directory. Perhaps that branch itself is related to some other piece
  of work.
- plans are related to tasks which are releated to specs which can be
  related to traces, todos, etc.

Would it be possible for us to design and build such a service ourselves
that is based on a graph database?

I have clone the gh repo of a Rust crate for `neo4j` for reference in:
`/home/pl/code/palekiwi-labs/cue/.ref/neo4rs/README.md`

other related notes:
- /home/pl/code/palekiwi-labs/cue/.cue/master/note/acumen-with-graph-db.md
- /home/pl/code/palekiwi-labs/cue/.cue/master/note/acumen/cue-context-with-graph-db.md
