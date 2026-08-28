# Project Log

## [d0f7f65] Research complete: store resolution and scope resolution state mapped

Kicked off the design session for first-class git worktree support.
Delegated two explore agents (store resolution; scope/HEAD resolution) and
read the existing cue specs. Research findings summarized in the session
report to the user; open forks listed there.

- **Found:** Worktree auto-resolution spec exists at .cue/master/spec/cue/worktree-auto-store-resolution.md but is NOT implemented; resolve_store (crates/cuelib/src/store.rs:31-66) still only follows .cue/STORE
- **Found:** Existing spec deliberately RETAINS cue link/STORE as escape hatch (rung 1) and per-worktree HEAD inheritance; user's new proposal removes link/STORE entirely - a documented-decision reversal to discuss
- **Found:** cue link writes .cue/STORE at crates/cue/src/commands/link.rs:29-31 and has pre-existing hardcoded '.cue' bug at link.rs:16
- **Found:** Scope resolution chokepoint is cuelib resolve_scope (crates/cuelib/src/head.rs:30-32); CLI call sites check --task flag first (e.g. crates/cue/src/add/mod.rs:51-56)
- **Found:** CUE_ env var precedent exists: CUE_CONFIG_DIR (crates/cuelib/src/config.rs:53), CUE_DATA_DIR (crates/cuelib/src/project.rs:9), CUE_-prefix figment merge (config.rs:64)
- **Found:** cue switch now owns branch-task association via git config branch.<name>.cue-task (crates/cue/src/commands/switch.rs:26-34,62-74); only consulted on explicit no-arg switch, not in read path
- **Found:** task-mode.md 'Deferred: Git worktrees' (lines 280-287) prescribed --task flag mitigation for multi-worktree agents; $CUE_TASK is the stronger version of exactly that
- **Found:** cue-agent task card (.cue/master/task/cue-agent.md) already plans a --task flag and an env overlay with redaction for spawned pi processes
- **Found:** cue init materializes .cue/ as a git worktree of the cue branch (crates/cue/src/init/mod.rs:36-75); git forbids the same branch in two worktrees, so the store is structurally singular per repo
- **Decided:** Design task card created at .cue/master/task/design-worktree-store-and-task-env.md (in-progress, kind: design)
- **Open:** Does killing STORE abandon the hard-linked local clone use case intentionally?
- **Open:** Per-worktree local HEAD (old spec) vs collapsed single store HEAD - which model under the new design?
- **Open:** Stray .cue/master/ in a worktree: ignore or error?
- **Open:** $CUE_TASK validation/empty/status-display/switch-guard semantics
- **Open:** Revise worktree-auto-store-resolution.md spec in place vs new spec?

## [855ff6a] Design forks resolved; superseded spec removed; working spec created

User resolved all five design forks. Superseded spec deleted locally;
working spec created at .cue/design-worktree-store-and-task-env/spec/index.md.
Remaining open questions are output-shape details only.

- **Found:** palekiwi control-center (.cue) is mounted read-only in this sandbox: /home/pl/code/palekiwi/palekiwi/.cue/master/task/worktree-auto-store-resolution.md and .cue/worktree-auto-store-resolution/ could NOT be deleted; user must remove them manually (path has git history there, recoverable)
- **Found:** CUE_TASK slug validation precedent: .cue/STORE content (a path) is strictly validated today (relative/nonexistent/no-master all bail, crates/cuelib/src/store.rs:31-66); .cue/HEAD content is NOT validated at all - HEAD is the precedent to follow for $CUE_TASK
- **Decided:** Kill cue link and .cue/STORE entirely; no escape hatch, one repo one store. User does not want to maintain unused code
- **Decided:** Store is always <git-root>/.cue (main worktree via git worktree list), loud cue init error otherwise
- **Decided:** Stray .cue content in a worktree is ignored silently
- **Decided:** HEAD is worktree-local with NO inheritance from git root: local .cue/HEAD or master. This is existing resolve_scope behavior; the old spec's inheritance rung is explicitly rejected
- **Decided:** Precedence: --task > $CUE_TASK > local .cue/HEAD > master; empty env = unset; NO special validation for $CUE_TASK (same treatment as HEAD content; STORE's strict validation dies with STORE)
- **Decided:** cue status / cue context must respect the precedence chain and report provenance
- **Decided:** cue switch warns (not refuses) when $CUE_TASK is set
- **Decided:** Old worktree-auto-store-resolution task/spec superseded: spec deleted from this repo; new task slug is design-worktree-store-and-task-env
- **Open:** cue status / context provenance label wording and whether to print the resolved store path
- **Open:** Should cue switch's $CUE_TASK warning also appear in --json output?
- **Open:** Audit external surfaces (cue.nvim, cue-plugins, skills/docs) for cue link / STORE references before removal ships

## [855ff6a] Open questions resolved; external surfaces audit complete

Cleared the last three open questions and completed the delegated external
surfaces scan. Spec updated with an Observability section and a Resolved
questions section; audit saved as a trace artifact for build planning.

- **Found:** External audit verdict: NO external code depends on cue link or STORE. cue.nvim and cue-plugins shell out to the cue CLI and do no independent store-path resolution; only doc/comment refreshes are needed
- **Found:** Stale-doc sites to refresh: cue.nvim lua/cue/core.lua:427 and lua/cue/picker.lua:492 (claim scope resolves solely from .cue/HEAD); cue-plugins --task help text (src/opencode/cue-add.ts:29,46); cue skill SKILL.md:73,234 and reference/cli.md:8,33; README.md:30
- **Found:** Audit trace saved at .cue/design-worktree-store-and-task-env/trace/1787568301-855ff6a/external-surfaces-audit.md with a 6-step rollout checklist
- **Decided:** cue status / cue context: print provenance labels (flag)/(env)/(head)/(default) in human output, structured provenance field in --json, and print the resolved store path in both (so agents never look in the wrong location)
- **Decided:** cue switch $CUE_TASK warning: stderr only, never in --json output
- **Decided:** cue skill must be updated at build time with the precedence chain and the store-location rule (user: agents should not look in the wrong location)

## [855ff6a-dirty] Design complete; build task and master plan created

User directed creation of the build task, signaling design convergence.
Design task and spec marked complete. Build task
`worktree-store-and-task-env-impl` created (open, kind: build, priority
high) with a seeded master plan (9 phases, TDD-ordered: cuelib store
resolution, link removal, call-site migration, $CUE_TASK rung, switch
updates, observability, init-in-worktree, docs rollout, final
validation). Spec and audit trace are referenced from both the card and
the plan, making the build task self-contained.

- **Found:** Task card naming inconsistency: cue-task created the build card without .md extension (worktree-store-and-task-env-impl) while the design card carries .md (design-worktree-store-and-task-env.md) - possibly manual or tooling rename; harmless but worth normalizing eventually
- **Decided:** Design task design-worktree-store-and-task-env marked complete; spec status complete
- **Decided:** Build task slug: worktree-store-and-task-env-impl; branch feat/worktree-store-cue-task, worktree worktrees/feat-worktree-store-cue-task, base master
- **Decided:** Plan phase order: library first, then removal, then migration, then behavior additions, docs last

## [82e74da-dirty] Root-caused extensionless task card defect (defense-in-depth failure)

Investigated the recurring extensionless-task-card defect observed after
task creation. Traced every layer; root cause is a chain of optional
conventions with no enforcement anywhere. Details reported to the user;
fix recommendation: normalize in `cue add` (CLI chokepoint) for markdown
artifact types.

- **Found:** Root cause chain: caller passes slug without .md (agent habit: filename arg is slug-like) -> plugin forwards verbatim (src/opencode/cue-task.ts:41) -> pi tool forwards verbatim (feat-pi-extension/src/pi/tools/cue-task.ts:87, only description-level guidance at :7) -> cue add joins verbatim (crates/cue/src/add/mod.rs:86; validate_filename at :186-202 only blocks traversal) -> collect_files lists everything (crates/cuelib/src/artifact.rs:150-165) so the defect is invisible to tooling and only surfaces to humans
- **Found:** The mid-session .md appearance on the design card was a manual mv (mtime preserved: build card 00:11 = cue-task creation, design card 00:12:52 = status:complete edit); no rename code exists in any cue crate, and .cue git history has NEVER committed an extensionless task card (checked git log --all)
- **Found:** Slug derivation is extension-agnostic (file_stem), so impact is convention/cosmetic: breaks the documented '.cue/master/task/<slug>.md' contract (task-mode spec, SKILL.md) but not board listing
- **Decided:** No fix implemented yet - awaiting user's choice of where it belongs (this defect is separate from the worktree design scope)
- **Open:** Where to fix: cue add normalization (append .md for markdown types when no extension) vs zod pattern on plugin tools vs both; trace/bin types need exemption

## [82e74da-dirty] Filename normalization spun off as fix-add-filename-normalization (high)

User decided the filename-normalization defect gets its own build task
(separate from worktree-store-and-task-env-impl, which it only
coincidentally touches in add/mod.rs). Created
`fix-add-filename-normalization` (high, open) with the root-cause
chain, the cue-add normalization fix, plugin hardening as optional,
and acceptance criteria.

- **Decided:** Fix lives in cue add (CLI chokepoint), type-scoped to markdown-emitting artifact types; plugin zod patterns are optional hardening, not part of the required fix
- **Decided:** Spun off as its own build task rather than folding into worktree-store-and-task-env-impl

## [82e74da-dirty] Markdown coupling verified; fix card updated with severity, design, and scope fork

User confirmed the markdown-coupling analysis and asked for the task card
update. Updated fix-add-filename-normalization.md with the severity
upgrade, the MARKDOWN_TYPES design, and the scope fork.

- **Found:** Markdown IS structurally ingrained on the read side: read_artifacts hard-filters .md (tested deliberately at artifact.rs:561); cue.nvim gates on %.md$ (core.lua:627); but the type->markdown mapping exists nowhere as data (config artifact_types is a flat string list, CANONICAL_TYPES carries no format metadata) - the invariant is implicit and asymmetrically enforced (write accepts anything, read silently drops)
- **Decided:** Fix design: declare MARKDOWN_TYPES const in cuelib artifact.rs beside CANONICAL_TYPES; cue add appends .md to extensionless filenames whose type is in the set; exempt types excluded by construction
- **Decided:** Card severity upgraded from cosmetic to silent data loss: extensionless task cards invisible in curator board (read_artifacts .md filter, artifact.rs:252-254) and cue.nvim listings (core.lua:627), while visible in cue list - inconsistent across surfaces
- **Open:** MARKDOWN_TYPES scope fork recorded on the card: full conventional set (doc note plan ref spec task todo, my lean) vs task-only minimum (what today's machine gates enforce); user to decide before implementation

## [c9a7039] Implementation completed without spec deviations

The implementation task `worktree-store-and-task-env-impl` completed all
behavioral and documentation phases. Shared git-root store resolution,
STORE/`cue link` removal, `$CUE_TASK` precedence, worktree switch/init
behavior, and status/context observability match the approved specification.

- **Found:** Final reruns remain constrained by sandbox OS error 11 despite earlier green full workspace tests and clippy
- **Found:** Workspace-wide rustfmt drift predates and is unrelated to the feature
- **Decided:** No behavioral deviations from the approved specification
- **Decided:** Treat sandbox validation limits and pre-existing formatting drift as delivery caveats, not specification deviations
- **Open:** Persist cue.nvim documentation commit ef5f2df from its temporary writable clone
