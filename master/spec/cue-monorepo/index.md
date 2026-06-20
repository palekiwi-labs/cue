# Cue Monorepo

## The `cue` framework

The general concept of `cue` is described in the "cue" agent skill.

`cue` is a framework for context and memory sharing between humans
developers and AI agents.

`cue` aims to deliver a streamlined UX to both humans and agents
by providing an ecosystem of tools:

## `cuelib`

A shared crate for `cue`, `curator` and `acuity`.

## `cue`

A CLI utility for creation and management of artifacts

## `curator`

A TUI for visual kanban-style tracking tool for `cue` artifacts
and agent harness events supplied by `acuity`. 

## `acuity`

An observability platform (server) that collects data on lifecycle
hooks in agent harnesses, stores them and serves them to tools
like `curator` for consumption (SSE and queries).

## agent harness plugins

In order to connect to `acutiy`, we need to provide agent harness
plugins (e.g. for `opencode`) that plug into lifecycle hooks and
collect observablity data. Agent harnesses like `opencode` and `pi`
are written in Typescript.

