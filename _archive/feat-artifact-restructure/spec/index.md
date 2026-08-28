# Artifact restructure

---

## Context

Currently we are able to use `mem add` command with a `--type` flag that accepts an artifact type
determines where to save the artifact based on its type:
- directly in the subdirectory
- nested subdirectory with timestamp and hash

This logic arbitrarily decides the hierachy and organization of artifacts.

I propose that we unify it, do not differentiate between artifact types in a hardcoded way and
instead let the caller decide how they want to save their artifact.

## Proposal

`mem add --type(-t) <type> <filename>` always saves the file under `.mem/<branch-name>/<type>/<filename>`

We need to introduce a new flag to `mem add` that allows user to save their artifact under:
`.mem/<branch-name>/<type>/<commit-timestamp>-<commit-hash>/<filename>`


As a result we will achieve two benefits:
- unified API
- ability to introduce a feature for the user to define their own supported artifacts in the config, e.g.: `plan`, `todo`, etc.
