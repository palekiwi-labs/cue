# Collection query scopes

`cue context list` and `cue context pins` accept `--scope <name>`.
Built-in `repo` selects the cwd repository; `store` selects the whole store.
Plain output always contains canonical `<org>/<repo>/<context>` addresses.
JSON context rows retain their existing shape.

Define named scopes in global `$CUE_CONFIG_DIR/cue.json`, or
`~/.config/cue/cue.json` when that variable is unset:

```json
{
  "scopes": {
    "team": ["palekiwi/*", "palekiwi-labs/*"]
  },
  "default_scope": "team"
}
```

A project `cue.json` at the Git toplevel overrides global config, including
when invoked from a subdirectory or with `-C`. Scope definitions merge by
name; a project matcher array replaces the global array of the same name.
Unknown config keys and malformed matchers are errors. No environment
config layer is used.

Explicit `--scope` overrides `default_scope`; without config, the default
is `repo`. Named definitions may shadow `repo` and `store`.

Matchers accept exact `<org>/<repo>` or `<org>/*`, optionally prefixed
with `!` to exclude matches. They select existing scope directories in the
chosen store, not Git remotes. Positive matches are unioned, then all
exclusions are subtracted regardless of order. Overlapping matchers do not
duplicate results; no positive matches means an empty listing, including
an exclusion-only array.

For example, `["palekiwi/*", "!palekiwi/spabreaks-org"]` selects every
existing palekiwi scope except spabreaks-org. `!palekiwi/*` excludes an
entire organization. This is not Gitignore syntax: `!` excludes rather
than re-includes, and a later positive matcher cannot undo an exclusion.
Named pin queries omit scopes existing only in pin state, whereas built-in
`store` continues to report all pins, including stale ones.

```sh
cue context list --scope team --filter kind=work
cue context list --scope team --pinned --limit 10
cue context pins --scope team
```

Named and whole-store queries work outside a Git repository. Only built-in
`repo` requires a cwd origin. Scope config affects neither singular
operations nor selectors: a bare context slug still resolves against the
cwd repository, regardless of `default_scope`. `cue list` is unchanged.
