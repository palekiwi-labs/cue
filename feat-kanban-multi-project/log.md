# Project Log

## [55d53fb] Kanban multi-project redesign — design decisions locked + artifacts created

Design session for the highest-priority initiative: making the curator kanban useful for daily multi-project practice. Five trade-off questions resolved. This supersedes the out-of-scope marker on multi-project kanban in spec/curator/curator-improvements.md:80 — the kanban itself now becomes multi-project, while the cross-project filter (existing projects-view.md task) remains deferred. Task, plan, and todo created on master; implementation starting with Slice 1.

- **Found:** Priority sort already exists (classify_tasks app.rs:620-641); the perceived unsorted issue is low visual prominence of the trailing [normal] tag
- **Found:** ArtifactMeta (cuelib/artifact.rs:210-222) has no project attribution — only absolute path; needs a curator-local KanbanTask wrapper
- **Found:** Split-pane pattern already implemented for Activity view (ActivityLayout app.rs:23-31, render_detail_pane ui.rs:333-420, static info block ui.rs:255-327) — reusable for vertical kanban detail split
- **Found:** Removing curator --root supersedes the open cross-binary-dir-flag-naming todo (curator --root vs cue --dir)
- **Found:** ProjectStore (cuelib/project.rs:71-150) maps key -> Vec<PathBuf>; curator does not consult it today
- **Decided:** D1: Remove --root entirely; kanban always global via ProjectStore
- **Decided:** D2: Card label = basename; detail pane = full path
- **Decided:** D3: Read first path only per project key (avoid duplicate cards)
- **Decided:** D4: Detail pane reflective-only; Enter toggles; j/k navigates columns
- **Decided:** D5: Title char-wrap, 2-line cap, … ellipsis
- **Decided:** Priority sort already exists (classify_tasks app.rs:620-641) — preserved, not re-added
- **Decided:** View keys stay 1=Kanban/2=Activity/3=Diagnostics — no Slice-7 renumber conflict
- **Decided:** New task kanban-multi-project.md + executive plan created; projects-view.md (Slice 8 filter) left open for a later PR
- **Decided:** Todo created: reconsider ProjectStore Vec<PathBuf> -> single path (multi-checkout footgun)
- **Open:** Close cross-binary-dir-flag-naming todo as superseded by --root removal (Slice 2)

## [dee05a8] Slices 1+2 implemented: KanbanTask, collect_tasks, --root removed

Implemented Slices 1 and 2 of the multi-project kanban redesign in a single atomic commit (dee05a8).

- **Found:** KanbanTask struct needs #[allow(dead_code)] on project_key/project_root fields since ui.rs only accesses task.meta fields for now (Slice 4 will use project_root for the detail pane)
- **Found:** collect_tasks iterates BTreeMap entries (sorted by key) giving deterministic ordering across projects
- **Found:** reload_tasks reloads ProjectStore from disk on each Refresh — cheap and ensures fresh registrations are picked up
- **Decided:** Slices 1 and 2 committed together as one atomic unit — they are inseparable since changing Vec<ArtifactMeta> to Vec<KanbanTask> requires UI and main.rs changes in the same build
- **Decided:** tempfile added to curator dev-dependencies for the new multi-project collector tests
- **Decided:** kanban_empty_store field added to App but UI hint deferred to after Slice 3/4 review
- **Open:** Slice 3 (multi-line card rendering + wrap_title) is next
- **Open:** Slice 4 (detail split pane) follows — project_root field on KanbanTask will be used there

## [fef68cf] Slice 3 committed: wrap_title + 3-line kanban cards

- **Found:** wrap_title ellipsis path triggers when rest.len() > width, not rest.len() == width — test initially had wrong expectation for ' world' (6 chars > width 5)
- **Found:** inner_width = area.width.saturating_sub(4): 2 for borders + 2 for '> ' highlight symbol
- **Decided:** project_root.file_name() used directly in render_column rather than reusing the string-based project_basename helper (different input type: PathBuf vs &str)

## [a2115ff] Slice 4 committed: KanbanLayout detail pane

- **Found:** KanbanTask import needed in ui.rs for the type annotation on the task variable in render_kanban_detail
- **Found:** render_kanban_detail uses task.meta.path for file path and task.project_root for project path (D2: card=basename / detail=full path)
- **Decided:** Split layout proportions: 70/30 (Ratio 7:3) matching the Activity view split aesthetic
- **Decided:** Help bar dynamically shows 'Enter detail' vs 'Enter close' based on kanban_layout state

## [3708f90] Option D committed: blank top-padding line per kanban card

QA feedback identified cards as too cramped. Option D (blank top-padding line) implemented as the lowest-risk card separation approach — one Line::default() prepended via std::iter::once().chain(...). Cards are now 4 lines tall (blank + up to 2 title lines + priority/project line). Awaiting user visual evaluation before deciding whether to proceed to Option A (bordered cards) or adopt this as final.

- **Found:** Reference kanban project (.ref/kanban) uses single-line items in a Paragraph widget — no bordered cards, no blank separators. Not a usable pattern for our multi-line List approach.
- **Decided:** Option D (blank padding line) implemented as interim card separator — 3708f90
- **Decided:** Remaining QA feedback items (remove > symbol, purple project name, drop brackets, swap order) deferred pending user evaluation of Option D
- **Open:** User needs to visually evaluate Option D — sufficient or proceed to Option A (bordered cards)?
- **Open:** Four other QA feedback items still unaddressed

## [02e4ea5] Kanban card margin + horizontal padding committed (02e4ea5)

- **Found:** ratatui 0.29 has no item_gap API — margin semantics require interleaving blank ListItems
- **Found:** HighlightSpacing::Never suppresses the symbol column reservation entirely
- **Found:** Visual index = sel * 2 + 1 keeps App model clean; only render_column knows about the doubling
- **Decided:** Blank separator as separate ListItem (never selected) achieves true margin: gap is outside highlight
- **Decided:** Leading space prefix on each content span provides 1-char left pad; inner_width unchanged at saturating_sub(4)
- **Decided:** highlight_symbol removed; HighlightSpacing::Never added
- **Open:** Remaining QA items: purple project name, drop brackets from priority, swap project/priority order

## [92644ce] Title-only highlight + bottom padding committed (92644ce)

Replaced whole-item background highlight with title-only color change (Yellow) on the selected card. Reverted from margin (interleaved blank ListItems) back to padding — single ListItem per card with a blank line appended at the bottom. Simpler model: no visual-index doubling, no highlight_style on the List widget. The is_active differentiation for selected items was removed (active column is still indicated by the thick cyan border).

- **Found:** Without highlight_style, the List widget applies Style::default() (no-op) to selected items — selection is visually indicated only by the yellow title
- **Decided:** Title-only highlight: selected card's title spans get Style::default().fg(Color::Yellow); all other spans unchanged
- **Decided:** Bottom padding: blank Line::default() appended to each card (padding, not margin — it's inside the ListItem but no background highlight so invisible)
- **Decided:** Removed highlight_style entirely from List; kept HighlightSpacing::Never to suppress symbol column
- **Decided:** Reverted flat_map + sel*2+1 back to map + enumerate + sel — simpler model
- **Open:** Remaining QA items: purple project name, drop brackets from priority, swap project/priority order

## [993a5c0] All five QA items addressed (993a5c0)

Five QA feedback items addressed in a single commit (993a5c0):

1. Highlight color changed from Yellow to Cyan — Yellow conflicted with high priority color.
2. Title highlight now active-column-only: condition changed from `idx == sel` to `is_active && idx == sel`. Inactive columns show no title highlight.
3. wrap_title rewritten from char-wrap to word-wrap: words accumulated per line until next word exceeds width; single words longer than width are char-broken; 2-line cap with ellipsis preserved. Two new tests added for multi-word scenarios.
4. Project name color changed from DarkGray to Magenta (matching detail pane).
5. Line 3 order swapped from `[priority]  project` to `project  priority`; brackets dropped from priority label.

- **Found:** Yellow highlight color was same as high priority color (priority_colour returns Color::Yellow for high)
- **Found:** Magenta is the existing project color in detail pane (ui.rs:137, 328, 395)
- **Decided:** Cyan for active selected title — matches border theme, distinct from all priority colors
- **Decided:** Word-wrap algorithm: collect all lines then cap at 2 with ellipsis (no early termination — titles are short)
- **Decided:** Char-break fallback for words longer than width uses slice.chunks(width) for clean iteration
- **Decided:** Project (Magenta) before priority (priority_colour) — swapped from previous order
- **Open:** Visual QA needed: does cyan title + cyan border look coherent or confusing?

## [dee4aa2] Slice 2 complete: empty-store hint wired into kanban help bar (dee4aa2)

Slice 2's last remaining item — the status-bar hint when the project store is empty — is now implemented (dee4aa2). The kanban_empty_store flag had been populated at startup and on Refresh since dee05a8 but was never read by the UI; a user launching curator with no registered projects saw three empty columns with no explanation.

The inline kanban help-bar construction was extracted into a pure kanban_help_line(layout, empty_store) helper mirroring the existing activity_help_line pattern. When empty_store is true it appends a trailing "no projects registered" hint. Three unit tests cover the non-empty/empty/layout branches via a flatten_line helper that concatenates span content.

All 124 curator tests pass and clippy is clean. The added lines are themselves fmt-clean.

- **Found:** The curator crate (app.rs, main.rs, sse.rs, ui.rs) has accumulated pre-existing rustfmt debt across prior commits — cargo fmt -p curator reformats ~140 lines purely whitespace/line-wrapping. This is unrelated to the empty-store hint and was NOT fixed (out of scope; surfaced to user for a decision).
- **Found:** kanban_empty_store flag (app.rs:170) was set at main.rs:74 and main.rs:177 since dee05a8 but never read by any consumer until now
- **Found:** activity_help_line (ui.rs:801) is the established pure-function pattern for help/status bars — mirrored exactly for kanban_help_line
- **Decided:** Trailing hint appended to the existing help line (user-chosen option a) rather than a separate centered board message — matches the Activity/Diagnostics help-bar idiom
- **Decided:** Hint text uses Span::raw (default color) matching the descriptive-label idiom (quit/views/reload are all raw); color can be tuned in a later QA pass if the user wants more prominence
- **Decided:** Trigger condition is kanban_empty_store only (zero registered projects); the 'store non-empty but all tasks filtered out' case is a separate future concern
- **Decided:** Did NOT bundle the crate-wide rustfmt cleanup into this commit — committed the feature only to respect scope, and surfaced the fmt debt to the user for a separate style commit decision
- **Decided:** Respected no-amend rule despite backtick shell-substitution corrupting two function names in the commit body; the subject line is clean
- **Open:** User decision: perform a separate style commit to clear pre-existing rustfmt debt across the curator crate (app.rs, main.rs, sse.rs, ui.rs)?
- **Open:** Visual QA: does the trailing 'no projects registered' hint read well, or does it need a distinct color (e.g. warning tone) for prominence?

## [dee4aa2] Task complete: multi-project kanban acceptance criteria satisfied

All 10 acceptance criteria for the multi-project kanban task are now satisfied and the master task file is flipped to status: complete.

The user gave explicit attestation for the three manual criteria (C4 card rendering, C6 --root removal + launch-from-any-dir, C7 Enter detail-pane toggle). The remaining seven were objectively verified by automated tests (collect_tasks_*, wrap_title_*, priority_sort, kanban_help_line_*) plus cargo test (124 passed) and clippy (exit 0).

- **Found:** The kanban-multi-project task is the first multi-slice task on this branch to reach complete: all 4 slices (data model, wiring, card rendering, detail pane) plus the deferred Slice 2 empty-store hint are done
- **Decided:** Task status: in-progress -> complete on the master task file; all 10 Evidence cells filled with dated test names or user attestation
- **Decided:** Manual criteria (C4/C6/C7) attested by user 2026-07-12 in conversation; recorded as such rather than self-attested

## [dee4aa2] Code review: feat/kanban-multi-project branch diff

Reviewed the full branch diff (Slices 1-4 + QA) for the multi-project kanban redesign in the curator crate. Read app.rs, main.rs, ui.rs, and the underlying cuelib APIs (ProjectStore=BTreeMap-backed, read_artifacts, ArtifactMeta). Overall solid, well-tested, faithfully mirrors the ActivityLayout pattern. No critical issues.

- **Found:** M1 (Major): highlight_symbol/highlight_style removed from render_column (ui.rs:226-235) while cards became 4 visual rows each; selected card may run off-screen since ratatui scroll-into-view is driven by the highlight. Needs manual verification with a long column.
- **Found:** M2 (Major): KanbanTask.project_key (app.rs:111) is populated in collect_tasks but never read; struct-level #[allow(dead_code)] (app.rs:106) masks it and is load-bearing under the -D warnings gate.
- **Found:** m1: wrap_title ellipsis >= width boundary (ui.rs:758) is correct but subtle; no real overflow because loop only emits <= width chunks. Worth a clarifying comment.
- **Found:** m2: collect_tasks Err(_) => continue (app.rs:701) silently swallows real IO/parse errors (read_artifacts already returns Ok([]) for missing dirs).
- **Found:** m3: ui.rs:1 import line ~104 cols, exceeds repo 80-col style; cargo fmt should wrap it.
- **Found:** Test coverage strong; gaps: render scroll-into-view for multi-line cards, wrap_title whitespace-collapse cases, collect_tasks with closed-status task.
- **Decided:** Flagged M1 and M2 as the two items to resolve before merge; everything else discretionary.
- **Open:** Does ratatui 0.29 ListState scroll the viewport to a selected multi-line item without a highlight symbol/style? Needs manual runtime check.
- **Open:** Is project_key intended for a future slice, or should it be removed along with the key.clone() allocation?

## [949ca9f] M1 investigation: scroll-into-view already works; reviewer diagnosis disproven

Investigated Opus review item M1 ("selected card scrolls off-screen because highlight was removed"). Read ratatui 0.29 source (widgets/list/rendering.rs:41-142) and wrote two regression tests (render_column_scroll_keeps_{first,last}_selected_visible) that render render_column into a TestBackend buffer with 15 tasks.

The reviewer's stated cause is factually wrong: ratatui List scroll-into-view is driven by state.selected (set via list_state.select(Some(sel)) at ui.rs:232), NOT by highlight_style or highlight_symbol. The highlight_style only paints the selected row (rendering.rs:137-139); highlight_symbol only sets the prefix glyph. Both tests PASS on the current code (commit dee4aa2) — the selected card is always visible, pinned to the bottom of the viewport. Verified with both uniform-height and variable-height (1- and 2-line wrapped title) cards.

The scroll tests are committed as a standalone regression addition (949ca9f). Surfaced to the user as a decision checkpoint: 'add the highlight' would NOT change scroll mechanics (scroll already works), but WOULD revert the earlier deliberate title-only-Cyan-highlight decision (commits 92644ce/993a5c0). The user's perceived 'disappear/reappear' is likely a visual-tracking issue (only a Cyan title marks the selection, hard to follow during viewport jumps), which a background highlight would address as a UX change — but that's a different justification than the bug-fix premise.

- **Found:** ratatui List scroll-into-view is driven by state.selected (rendering.rs:65-66), not highlight_style/symbol — Opus review M1 diagnosis is incorrect
- **Found:** Two regression tests (render_column_scroll_keeps_{first,last}_selected_visible) PASS on current HEAD: selected card always visible in a 15-task column
- **Found:** Confirmed with variable-height cards (mixed 1-line and 2-line wrapped titles) — viewport adjusts correctly in all cases
- **Found:** items_after_test_module clippy lint is PRE-EXISTING on HEAD (--tests mode only); project green gate (clippy without --tests) passes — unrelated to this work
- **Decided:** Committed scroll-into-view regression tests (949ca9f) as the reviewer's highest-value missing test — independent of the M1 decision
- **Decided:** Raised M1 as a decision checkpoint rather than blindly applying the requested fix, since the fix premise is disproven and it reverts a prior deliberate aesthetic decision
- **Open:** User decision on M1: (a) add whole-card background highlight anyway as a UX-tracking improvement [reverts title-only], (b) keep title-only and accept current scroll, (c) middle ground e.g. scroll_padding to reduce jump jarring
- **Open:** Pre-existing items_after_test_module lint (ui.rs has pub(crate) fns after the test module) — candidate for the app.rs/ui.rs refactor task (m2)

## [949ca9f] Review feedback triaged: M1 = keep title-only; fixed-card-height proposed; 5 fixes approved + plan created

User reviewed the M1 finding and confirmed option (b): keep the title-only Cyan highlight as-is (scroll genuinely works, proven by the two regression tests in 949ca9f). The user then identified the REAL visual issue via two screenshots (cards-scroll-01/02.txt): trailing whitespace at the bottom of columns when scrolling. Root cause confirmed: variable card heights (3 lines for short titles, 4 lines when the title wraps to 2 lines) interact with ratatui's whole-item list rendering — the viewport shows only whole cards, leaving a variable-height blank gap, which shifts as you scroll over mixed-height cards.

The user proposed a fix: give every card a FIXED height by padding short titles to a 2-line title block, so all cards are uniform (eliminates the shifting whitespace and makes scroll jumps consistent). This is marked as a decision-required item in the new review-fixes plan (whether to implement now + exact height).

The other five review items are approved for execution: M2 (drop KanbanTask.project_key), m1 (wrap_title ellipsis comment), m3 (wrap the over-length import), m4 (empty-store hint in Red), and m2-task (create a master task for collect_tasks error surfacing + app.rs 700-line refactor). An executive plan (review-fixes.md) has been created capturing the agreed steps + the open decisions; awaiting go-ahead before execution.

- **Found:** cards-scroll-02.txt:8-10 shows 3 trailing blank lines in the Open column — caused by whole-item rendering over variable card heights (3 or 4 lines), not a scroll bug
- **Found:** Fixed card height (pad short titles to 2-line title block => uniform 4-line cards) would make trailing whitespace CONSISTENT (viewport mod 4) but cannot fully eliminate it — ratatui shows whole items only; partial-card display or stretching isn't supported by default
- **Found:** Pre-existing items_after_test_module clippy lint (ui.rs pub(crate) fns after the test module, --tests mode only) is real on HEAD — candidate for the app.rs/ui.rs refactor (m2-task)
- **Decided:** M1: option (b) — keep title-only Cyan highlight, no code change. Scroll-into-view already works (regression tests 949ca9f); the reviewer diagnosis was wrong
- **Decided:** Five review items approved: M2 (drop project_key), m1 (wrap_title comment), m3 (wrap import), m4 (red hint), m2-task (master task for error handling + app.rs refactor)
- **Decided:** User proposed fixed-card-height as the real fix for the trailing-whitespace visual issue — captured as decision-required in the plan, not yet committed
- **Open:** Q1 (decision): implement fixed 4-line cards now, defer, or different height?
- **Open:** Q2 (decision, carry-over): separate style commit to clear crate-wide rustfmt debt (~140 lines, app.rs/main.rs/sse.rs/ui.rs)?
- **Open:** Awaiting user go-ahead to execute the five approved review-fix steps

## [7c2d338] Step 1 (M2) committed: drop dead KanbanTask.project_key

Executed step 1 of the review-fixes plan (M2): removed the never-read project_key field from KanbanTask and the load-bearing struct-level #[allow(dead_code)]. collect_tasks now iterates store.entries().values() (clippy::for_kv_map). Both test helpers (make_kanban_task in app.rs, kanban_task_with_title in ui.rs) updated. 126 tests pass, clippy clean.

- **Found:** Dropping project_key made the loop key unused, triggering clippy::for_kv_map under -D warnings — fixed by iterating .values() directly
- **Found:** After removing the field + #[allow(dead_code)], clippy stays clean — the field was the only dead-code reason the attribute existed
- **Decided:** Iterate store.entries().values() rather than binding (_key, paths) — clippy-preferred and clearer intent
- **Open:** Steps 2-5 of review-fixes plan still pending: m1 wrap_title comment, m3 import wrap, m4 red hint, m2-task master task

## [666ff26] Step 2 (m1) committed: clarify wrap_title ellipsis boundary

Executed step 2 of the review-fixes plan (m1): added a 4-line clarifying comment at the wrap_title line2 truncation branch (ui.rs:756-770) explaining that line2 is at most width chars by construction, so the >= width check only trims the exact-fill case to make room for the ellipsis. No behavior change. 126 tests pass, clippy clean.

Note: backticks around 'width' in the commit body were shell-substituted to empty; subject line is clean. Per the no-amend rule, left as-is.

- **Decided:** Comment-only change (no behavior change) — verified by 126 passing tests unchanged
- **Open:** Steps 3-5 still pending: m3 import wrap, m4 red hint, m2-task master task

## [7682add] Step 3 (m3) committed: wrap over-length use import in ui.rs

Executed step 3 of the review-fixes plan (m3): wrapped the 108-col single-line `use crate::app::{...}` import at ui.rs:1 into the standard multi-line form. Also reordered alphabetically (ActivityLayout before AcuityStatus) to satisfy cargo fmt's reorder_imports. The wrapped line is 94 cols (under rustfmt default max_width=100). Scoped to this one import only — the broader ~140-line crate-wide rustfmt debt remains a separate Q2 decision. 126 tests pass, clippy clean, import line is fmt-clean.

- **Found:** rustfmt default max_width=100 (no rustfmt.toml in repo); the wrapped line at 94 cols is fmt-clean
- **Found:** rustfmt reorder_imports=true reorders alphabetically: ActivityLayout < AcuityStatus because 't' < 'u' at the third char
- **Decided:** Matched fmt's preferred alphabetical order rather than preserving the original AcuityStatus-first order — keeps the line fmt-clean
- **Open:** Steps 4-5 still pending: m4 red hint, m2-task master task
- **Open:** Q2 (crate-wide rustfmt debt) still unaddressed — separate decision

## [6ccd708] Step 4 (m4) committed: empty-store hint colored red

Executed step 4 of the review-fixes plan (m4): changed the trailing 'no projects registered' span in kanban_help_line (ui.rs:835-840) from Span::raw to Span::styled with Color::Red, giving it warning-tone prominence. The existing 3 kanban_help_line tests assert content via flatten_line (not color) and all still pass. 126 tests pass, clippy clean.

- **Decided:** Color::Red for warning tone — matches the established priority/warning color idiom
- **Open:** Step 5 (m2-task) still pending: create master task for collect_tasks error surfacing + app.rs refactor

## [6ccd708] Step 5 (m2-task) done: master task created for error surfacing + app.rs split

Executed step 5 of the review-fixes plan (m2-task): created master task `curator-error-surfacing-and-app-split.md` capturing two out-of-scope items from the Opus review: (1) collect_tasks silently swallows real IO/parse errors at app.rs:696-698 (Err(_) => continue) — a misconfigured project is invisible; (2) app.rs is 1611 lines — split into modules, and fold in the pre-existing items_after_test_module clippy lint in ui.rs (7 pub(crate) fns at ui.rs:1386-1469 defined after the test module at ui.rs:854).

Task has 3 acceptance criteria (error surfacing test, module split, clippy --tests clean). refs point to the review trace + the kanban-multi-project task. This is a cue artifact (master task), not a code change — no commit.

- **Found:** collect_tasks Err(_) => continue is at app.rs:696-698 after the M2 edits (was 699-702 in the plan)
- **Found:** app.rs is now 1611 lines (plan said 700+)
- **Found:** items_after_test_module lint source: 7 pub(crate) fns (format_datetime_on, harness_abbrev, trunc_pad, format_tokens, format_event_datetime, session_unique_agents, session_unique_models) at ui.rs:1386-1469, after the test module at ui.rs:854
- **Decided:** Task created on master with normal priority — not blocking, but tracked for a future PR
- **Open:** All 5 review-fix steps are now complete; Q1 (fixed card height) and Q2 (crate-wide rustfmt debt) remain as open decisions for the user

## [442231c] Q1 (a) committed: uniform 4-line kanban cards via title padding

Implemented Q1 option (a): fixed 4-line kanban cards. Short titles are now padded to a 2-line title block (blank line 2) so every card is a uniform 4 rows: title-1, title-2-or-blank, project/priority, blank-pad. This eliminates the shifting trailing whitespace at the bottom of columns when scrolling over mixed-height cards.

The fix is minimal: after wrap_title returns, if only 1 line came back, push an empty String to pad to 2 lines. The rest of the card construction (project/priority line + blank pad) is unchanged, so all cards are now exactly 4 rows.

TDD: wrote render_column_cards_have_uniform_height first (RED — project lines at [2, 6, 9], non-uniform), then implemented the pad (GREEN — project lines at [3, 7, 11], uniform 4-apart). 127 tests pass, clippy clean.

Limitation noted in the plan: a fixed height makes trailing whitespace CONSISTENT (viewport height mod 4) but cannot fully eliminate it — ratatui shows whole items only.

- **Found:** Before fix: short-title cards were 3 rows, wrapped-title cards were 4 rows — project/priority lines at [2, 6, 9] (non-uniform gaps)
- **Found:** After fix: all cards are 4 rows — project/priority lines at [3, 7, 11] (uniform 4-apart)
- **Found:** wrap_title always returns 1 or 2 elements (capped at 2 with ellipsis; empty input returns vec![String::new()]), so a simple len()==1 check is sufficient — no while loop needed
- **Decided:** Pad with String::new() (empty line) rather than a styled blank — matches the existing blank-pad line idiom (Line::default())
- **Decided:** Used if title_lines.len() == 1 rather than while < 2 — wrap_title's contract guarantees 1 or 2 elements, defensive while is unnecessary
- **Open:** User visual QA: does the uniform 4-line card height resolve the perceived trailing-whitespace issue? Trailing whitespace is now consistent (viewport mod 4) but not fully eliminated (ratatui shows whole items only)
- **Open:** Q2 (crate-wide rustfmt debt) still unaddressed — separate decision

## [442231c] Bordered cards research complete: two doc artifacts saved, ready for discussion

Delegated two parallel research tasks on bordered kanban cards:
1. Ratatui 0.29 List internals — confirmed List cannot render bordered items (ListItem holds Text=Vec<Line>, written flat to buffer, no per-item Block). Hacking border chars into Lines breaks in 5 ways. Manual construction (per-card Rect + Block render) is the only clean path. Scroll-into-view for fixed-height cards collapses to ~5 lines.
2. Reference kanban (.ref/kanban/) — uses Paragraph with 1-row cards, no borders. Less directly useful for bordered cards, but its patterns (virtual viewport, decoupled selection/pagination) are conceptually aligned.

Findings saved to two doc artifacts. Key implementation decisions surfaced for user discussion: card height (6 with borders, or 5 if we drop the now-redundant blank pad), selection highlighting (border color+type vs background), and offset persistence.

- **Found:** List render loop (rendering.rs:115): item.content.render_ref(item_area, buf) writes Vec<Line> flat — no per-item Block possible
- **Found:** ListState scroll algorithm (get_items_bounds, rendering.rs:146-224) collapses to ~5 lines for fixed-height cards: offset = sel < visible_count ? 0 : sel - visible_count + 1
- **Found:** Curator creates fresh ListState each frame (ui.rs:239) with offset=0 — offset is NOT persisted; scroll recomputed from selected every render
- **Found:** Block::render_ref(area, buf) can draw borders around any arbitrary Rect (block.rs:702-711); Rect::intersection clips to viewport (rect.rs:188-199)
- **Found:** Reference kanban uses 1-row cards via Paragraph — no borders, no variable heights. Not directly reusable but virtual-viewport pattern is conceptually aligned
- **Decided:** Saved both research reports as doc artifacts for future reference
- **Decided:** Reported key implementation decisions to user for discussion before implementation
- **Open:** Card height: 6 (4 content + 2 borders) or 5 (drop blank pad since border provides separation)?
- **Open:** Selection highlighting: border color+type (Cyan+Thick vs DarkGray+Plain) vs background fill vs both?
- **Open:** Offset persistence: keep current recompute-from-selected (simpler, jumps) or persist in App (smoother)?
- **Open:** Title highlight: still Cyan on selected, or border alone suffices?

## [442231c] Bordered-cards plan created; design decisions locked

Research confirmed List cannot render bordered items (ratatui rendering.rs:115 writes Lines flat; no per-item Block hook). Manual Block-per-card approach is the only clean path. Two doc artifacts saved from research. User reviewed findings and locked five design decisions. An executive plan (bordered-cards.md) has been created for handoff to an implementing agent.

- **Found:** Reference kanban (.ref/kanban/) uses Paragraph with 1-row single-line cards — not directly reusable for bordered cards, but confirms the virtual-viewport pattern (compute visible indices, render only those) and minimal-scroll semantics (scroll_offset_to_keep_visible in pagination.rs:22)
- **Found:** scroll-into-view for fixed-height cards is O(1) arithmetic — negligible performance cost vs List's O(n) get_items_bounds scan
- **Found:** frame.render_widget and frame.buffer_mut() cannot be mixed in one draw closure due to borrow rules — must use block.render_ref(area, buf) for the outer column block if mixing with direct buffer writes
- **Found:** Cell<usize> is the idiomatic way to persist offset from render_column (which takes &App) without requiring &mut App or restructuring the draw callback in main.rs:95
- **Decided:** D1: Drop blank-pad line — borders provide separation; card content = 3 lines (title-1, title-2-or-blank, project/priority)
- **Decided:** D2: CARD_HEIGHT = 5 (3 content + 2 border rows)
- **Decided:** D3: Border-only selection: Cyan+Thick (selected), DarkGray+Plain (unselected); no background fill
- **Decided:** D4: Drop title Cyan highlight — all title spans Style::default(); border color is the sole selection signal
- **Decided:** D5: Persist scroll offset in App as Cell<usize> per column (offset_open, offset_in_progress, offset_complete); replicates ListState::get_items_bounds fixed-height case with stay-put semantics instead of pin-at-bottom
- **Open:** Implementation handoff: implementing agent should follow bordered-cards.md plan (4 steps); green gate is cargo test -p curator + cargo clippy -p curator -- -D warnings after each step

## [ada5e23] [ada5e23] Bordered kanban cards implemented (bordered-cards.md plan complete)

Replaced List/ListItem/ListState rendering in render_column with a manual Block-per-card loop. All four steps of bordered-cards.md executed in a single atomic commit (ada5e23).

- **Found:** CARD_HEIGHT=5 (1 top-border + 2 title rows + 1 proj/prio + 1 bottom-border) is the correct uniform height for bordered cards
- **Found:** Widget::render(self, ...) consumes the Block — must call card_block.inner(card_area) before rendering to preserve the inner Rect
- **Found:** WidgetRef is private in ratatui 0.29; Widget (public) is the correct trait to import for calling .render() on Block
- **Found:** render_column_cards_have_uniform_height expected [3,7,11] (old 4-line cards); updated to [4,9,14] for 5-line bordered cards
- **Found:** Scroll tests (render_column_scroll_keeps_first/last_selected_visible) pass unchanged — scroll-into-view arithmetic in render_column now uses column_offset Cell<usize> not ListState
- **Found:** No Cyan title highlight remains (D4): all title spans are Style::default(); border colour is the sole selection signal
- **Decided:** D1-D5 from bordered-cards.md all implemented: blank-pad dropped, CARD_HEIGHT=5, border-only selection (Cyan+Thick/DarkGray+Plain), title highlight removed, Cell<usize> scroll offsets in App
- **Decided:** Committed as one atomic unit — Steps 1-4 are inseparable since the offset fields, render_column rewrite, and test update all compile/fail together
- **Open:** User visual QA: do bordered cards resolve the trailing-whitespace issue and look coherent?
- **Open:** Q2 (crate-wide rustfmt debt across app.rs/main.rs/sse.rs/ui.rs) still unaddressed

## [7a56ec7] [7a56ec7] QA pass: compact cards + gray column border

- **Found:** CARD_HEIGHT=4 (top-border + 1 title + proj/prio + bottom-border) eliminates the blank title-padding line from CARD_HEIGHT=5
- **Found:** Column border was DarkGray for inactive, Cyan+Thick for active — now always DarkGray+Plain; active state shown via title span style only
- **Decided:** Column border: always DarkGray+Plain; active column title: Cyan+Bold
- **Decided:** CARD_HEIGHT=4; single title line (first line from wrap_title); no blank padding
- **Decided:** Updated uniform-height test expectation to [3,7,11]

## [5504ccd] [5504ccd] QA pass: compact card stride + gray border + bright detail pane

- **Found:** CARD_STRIDE=CARD_HEIGHT-1=4 causes the bottom border of card N and top border of card N+1 to share one row; the later render overwrites, halving the visual separator to 1 row
- **Found:** visible_count formula changes to 1 + (H - CARD_HEIGHT) / CARD_STRIDE for the stride-based geometry
- **Found:** DarkGray label spans in render_kanban_detail were hard to read because detail pane focus can never be lost; changed to Span::raw (default white)
- **Decided:** CARD_HEIGHT=5 kept (3 content rows preserved)
- **Decided:** CARD_STRIDE=4 (border overlap) for tighter inter-card visual gap
- **Decided:** Column border: always DarkGray+Plain; active title Cyan+Bold
- **Decided:** Detail pane border: Cyan (always visible); field labels: default white

## [e6fa762] [e6fa762] Fix broken card borders: drop bottom border, single-row separator

- **Found:** CARD_STRIDE=CARD_HEIGHT-1 overlap approach caused all cards except the last to lose their bottom border — visually broken
- **Found:** Correct approach: Borders::TOP|LEFT|RIGHT only; no card has a bottom border so all look consistent; top border of card N+1 is the 1-row separator
- **Decided:** CARD_HEIGHT=4 (top-border + title1 + title2/blank + proj/prio, no bottom border)
- **Decided:** Dropped CARD_STRIDE constant; stride = CARD_HEIGHT (contiguous stacking, no overlap)

## [1dd5126] [1dd5126] Restore full card borders: CARD_HEIGHT=5, Borders::ALL

The e6fa762 approach (TOP|LEFT|RIGHT only, CARD_HEIGHT=4, relying on next card's top border as separator) did not produce a visible bottom border — the cards appeared open-bottomed. Restored Borders::ALL and CARD_HEIGHT=5 (top-border + title1 + title2/blank + proj/prio + bottom-border). User confirmed this fixed the issue.

- **Decided:** Borders::ALL + CARD_HEIGHT=5 is the correct geometry; the no-bottom-border approach from e6fa762 is abandoned

## [0112b92] [0112b92] Gray card borders distinguish cards from column border

- **Decided:** Unselected card border: Gray (lighter); column border: DarkGray (darker); selected card: Cyan+Bold — three distinct levels

