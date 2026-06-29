---
status: open
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
