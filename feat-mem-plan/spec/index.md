# mem plan

---

## Context

A single plan of MD does not reflect the complexity of planning. A plan mmay be a masterplan
that describes the high-level design or the architecture of a solution. Then a plan may
involve a scoped sequence of executions steps. For that reason, we would like to treat
plan artifacts as a distinct category with its own API.

## Purpose

Design and implement a way to treat plans as first-class citizens with its own handling commands.

I propose the following structure:

- a master plan living in `.mem/<branch-name>/plan/index.md`
- a execution plans living in `.mem/<branch-name>/plan/<timestamp>-<hash>/<plan-name>.md`

## Example API

mem plan add <name>              # → plan/<timestamp>-<hash>/<name>.md
mem plan init                    # → plan/index.md
mem plan list                    # → lists all, master first
