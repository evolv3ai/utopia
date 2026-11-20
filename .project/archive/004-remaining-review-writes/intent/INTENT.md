# Intent — utopia M004 remaining-review-writes

<!-- Written by /gsd-path-define in milestone + brownfield mode. Derived from
     the approved ROADMAP.md entry M004 and CHARTER.md; no re-interview.
     Constraints and vetoes here override everything downstream. -->

Lane: milestone
<!-- Derived from the approved roadmap entry M004 (owner approval of the roadmap, driver D025). The entry lists one open question, so milestone-scoped research and decide run before planning. -->

Review panel: off
<!-- Copied from CHARTER.md (Delegated, driver D008). -->

Finding skeptics: off

Surfaces: token-authenticated HTTP API, MCP endpoint, Review tab in the web app (Agent History and the item cards of each section)

## Summary

After M003 an agent holding a personal token reads every Review section and can keep, merge, revert and batch duplicate pairs. Every other Review write still answers a token with 401. This milestone opens the rest of the owner's ruled authority table to a write-scope token of an editor. Most of these operations are propose only: the call changes nothing and waits as a proposal that only a signed-in person can approve or decline in the Review tab. Two operations finalize at once (a mapping decision; a consistency check or inference run), and the kind-word decision, which already finalizes for write tokens, is recorded and attributed like the others. Each operation gets an MCP tool, each proposal is shown on its item's card, and the program closes with ADR 0076 and the API and MCP documentation matching what shipped. Source: ROADMAP.md entry M004; program spec `C:\Users\Owner\.gsd-path-driver\projects\utopia-ef4aa95f\SPEC.md`.

Lines marked `Delegated (driver D0NN)` were answered for the owner by the gsd-path driver under the owner's consent of 2026-11-19 ("you have my consent to be the human") and can be overruled; the log is `C:\Users\Owner\.gsd-path-driver\projects\utopia-ef4aa95f\DECISIONS.md`.

## Problem

The spec's goal is that an agent "can work every section of the Review tab", and at `d5d2356` it can act in two of them only (Duplicates and Merges). A token that confirms, rejects or closes a fact, resolves a conflict, or decides a violation, defect, errata action, alignment phrase or rule, mapping, ontology proposal or governor decision gets 401, and so does a consistency or inference run (codebase map F9 to F12). The proposal store, the answer route, the Agent History row and the web labels were built for merges and reverts only: no store function lands a propose-only proposal, the answer route knows three operations and answers 500 for any other, and a proposal has a card to appear on in Duplicates and Merges only (F3, F5, F7). The spec sets the limit on the fix: "a document can talk an agent into recording something; the same risk applies to an agent confirming or merging facts on the strength of text it just read" (spec, Goal). The owner's ruled table therefore makes these operations propose only, apart from the three rows it rules "finalize" (SYNTHESIS.md, Agent-authority policy).

## Users

- An agent (MCP client or script) holding a write-scope personal token whose owner is an editor of the base: proposes decisions in every Review section, decides mappings, runs the consistency check and inference, and reads back what happened to each proposal.
- A signed-in editor in the Review tab: sees each proposal in Agent History and on the item's card in its own section, and approves or declines one or many.
- A signed-in base Admin: reads every agent action in Activity.
- Technical users; today all of this is done by hand in the browser.

## Success criteria

1. Over the token API and over MCP, a write-scope token owned by an editor calls each propose-only operation on a seeded item: no fact, entity, review row, retrieval result or reasoning output changes, and one proposal appears for that target.
2. In the Review tab, a signed-in editor sees each such proposal in the Agent History section and on the item's card in its own section, approves it, and the graph changes exactly as the same decision made by that person would change it; a declined proposal changes nothing.
3. Over the token API and over MCP, a mapping decision and a consistency check run by a write token take effect at once, each with a ledger row of outcome `applied`.
4. A write token's kind-word decision behaves as it did before the program and now appears in Agent History and Activity as "owner via token name".
5. A pending fact proposed through `remember` is still asserted or rejected only by a signed-in person; a token call that tries to answer it never changes the pending row.
6. For every operation in the ruled authority table, a call with a write-scope token owned by an editor yields the outcome the table states, and the same call with a read-scope token or a below-editor owner is refused with a stable code, over the token API and over MCP.
7. Every action taken in criteria 1 to 4 appears in Agent History and in Activity naming the token owner and the token.
8. The API and MCP documentation list every opened Review route and tool with its ruling, and ADR 0076 matches the shipped behaviour.
9. All eight program success criteria of the charter hold on a build of this checkout, including an unchanged Review tab for a signed-in person.

## Edge coverage

<!-- Rulings marked Delegated were given for the owner by the driver. -->

| Edge | Criterion | Category | Disposition | Detail |
|------|-----------|----------|-------------|--------|
| E1 | SC1 | empty | held-out | Delegated (driver D129): "nothing changes" on a propose-only call is read as: every `facts`, `entities` and `fact_evidence` row of the base, the target's own row (conflict, violation, defect, errata action, phrase binding, implication rule, ontology proposal, pending fact, governor decision) and the count of queued jobs are equal before and after the call |
| E2 | SC1 | idempotency | held-out | Delegated (driver D129): there is one open proposal per base, operation and target; a repeat, or a second call arriving at the same moment, returns the existing proposal's id with `already_proposed` and writes no second row; two different operations on one target (confirm and reject of one fact) may both be open |
| E3 | SC1 | adjacency | held-out | Delegated (driver D129): a target id that is unknown or belongs to another base is not found; a target that has a decided state and is already decided (a conflict not open, a violation or defect not open, an errata action not held, an ontology proposal not open, a governor decision already answered, an invalidated fact for reject and close) answers `already_decided`; neither lands a proposal |
| E4 | SC1 | encoding | held-out | Delegated (driver D129): the body of a propose-only call is validated when it is proposed, with the same refusals and codes the person's route gives for that body (for example `bad_resolution`, `close_at_required`, `interval_inverted`, `unknown_property`, `bad_status`, an unknown conflict action); an invalid body lands no proposal |
| E5 | SC1 | boundary | held-out | Delegated (driver D129): proposals and refused calls do not count toward the 200-per-hour limit and a propose-only call is never answered `agent_rate_limited` |
| E6 | SC1 | concurrency | held-out | Delegated (driver D129): a token's proposed answer to a governor decision changes neither the governor's decision row nor the pair nor the governance fuse and starts no govern round; when a person approves it, the answer runs as that person's answer does today |
| E7 | SC2 | idempotency | held-out | Delegated (driver D129): answering needs a signed-in Editor of the base; a proposal already answered answers `already_decided` and nothing changes; approval runs the person's own store function for that operation with the person as actor and the proposal named in the audit detail |
| E8 | SC2 | concurrency | held-out | Delegated (driver D129): approving a proposal whose target is gone or was decided meanwhile closes it as `superseded`, changes nothing else, and the answer says so; for a target that has no decided state (confirm on a live fact, an alignment phrase or rule) approval applies the decision while the target exists |
| E9 | SC2 | ordering | held-out | Delegated (driver D129): approving an alignment phrase or rule proposal answers with the queued job's id, and the proposal's row says the change is queued, not that it is applied |
| E10 | SC2 | boundary | held-out | Delegated (driver D129): one bulk answer takes 1 to 500 proposal ids of any mix of operations and returns one outcome per id in request order; an empty list or 501 ids is refused as a whole |
| E11 | SC2 | adjacency | held-out | Delegated (driver D129): the open proposal is carried on its item in every section's list for session, token and MCP readers, and the key is present only while a proposal is open, so an item without one has the same keys as before; the Agent rail badge counts open token proposals beside the governor's (driver D128) |
| E12 | SC2 | encoding | held-out | Delegated (driver D129): every operation, reason and state this milestone writes has a label in English and Chinese and none is shown as a raw code; an Agent History row whose target is not a pair names its target in words and never prints a placeholder for a missing side |
| E13 | SC3 | idempotency | held-out | Delegated (driver D129): a mapping decision equal to the mapping's current status answers `already_decided` and writes no ledger row; a different status applies and writes one `applied` row; an unknown mapping id is not found; a status other than confirmed or rejected keeps `bad_status` |
| E14 | SC3 | concurrency | held-out | Delegated (driver D126): each consistency check and each inference run by a token writes one `applied` ledger row whose target is the base and returns the report a person gets; the run itself is today's code with no new lock; inference on a base with the setting off answers `inference_off` and writes no ledger row |
| E15 | SC3 | boundary | held-out | Delegated (driver D129, D126): mapping decisions, consistency checks and inference runs count toward the 200 finalized writes per token and base in a rolling hour; the call after the 200th answers `agent_rate_limited` with HTTP 429, changes nothing and writes no `applied` row |
| E16 | SC4 | precision | held-out | Delegated (driver D125): the kind-word REST route keeps its status, response body, `unknown_class` and `alignment_busy` codes and its plain 403 for a below-editor owner; a token call now also writes one `applied` ledger row and the token name in its audit detail; it is never refused by the rate limit, and its rows count toward the token's hourly total |
| E17 | SC5 | adjacency | held-out | Delegated (driver D127): after any token call about a pending fact, over REST or MCP, the `pending_facts` row is byte-equal and no fact was written or rejected; a pending id that is unknown or already nodded is not found; the person's nod route still answers a token as it did before unless it returns a proposal |
| E18 | SC5 | idempotency | held-out | Delegated (driver D127): when a signed-in editor approves a proposed answer to a pending fact, the nod is recorded as that person's (`fact.nod_confirmed` or `fact.nod_rejected` with the person as actor and the proposal named); declining leaves the pending row waiting |
| E19 | SC6 | boundary | held-out | Delegated (driver D129, corrected by C1, driver D131, D134): for each newly opened operation, over REST a read-scope token answers `token_read_only`, a below-editor owner answers `token_owner_below_editor`, and a write token whose bases exclude this one answers `token_kb_out_of_scope`, as the M001 to M003 writes answer; over MCP the write tools are not listed for a read-scope or below-editor caller and a call to one returns the tool error carrying the same code, as the M003 tools behave (the kind-word REST route keeps its plain 403, E16) |
| E20 | SC7 | encoding | held-out | Delegated (driver D129): the token name shown is the snapshot stored with the action; a proposal, its approval, decline or supersession, a mapping decision, a run and a kind-word decision each have an Agent History row and an Activity row reading "owner via token name" |
| E21 | SC8 | none | dismissed | A documentation criterion with no data shape; the final reviewer compares the documents with the route table in `crates/utopia-server/src/api/mod.rs` and the MCP tool list |
| E22 | SC9 | idempotency | held-out | Delegated (driver D129): the existing session tests of these sections pass unchanged; the only edits to existing tests are assertions that a token is refused on a route this milestone opens, exact tool lists and label lists that gain entries, and every assertion for a route or tool that stays closed keeps its expectation |

## Prohibitions

| Prohibition | Criterion | Must not | Disposition | Detail |
|-------------|-----------|----------|-------------|--------|
| N1 | SC1 | change a fact, entity, review row, retrieval or reasoning, or queue a job, on a propose-only call | criterion SC1 | Delegated (driver D129) |
| N2 | SC2 | record an approved proposal as the agent's act, or hide which proposal a person's decision came from | judgment | Delegated (driver D129): the reviewer checks that each approval is applied with the person as actor and that its audit detail names the proposal |
| N3 | SC2 | let any token approve or decline a proposal, its own included | judgment | Delegated (driver D129): the reviewer checks that the answer routes still take a session only, that no MCP tool answers a proposal, and that the M003 tests of this refusal are unchanged |
| N4 | SC5 | let a token assert or reject a pending fact by any path | criterion SC5 | Delegated (driver D127): charter veto on weakening the nod gate |
| N5 | SC5 | show a proposed answer to a pending fact without saying that a token proposed the answer, or drop the card's existing line naming who remembered the sentence | judgment | Delegated (driver D127): the reviewer checks on the nod card that the proposal names its owner and token and that the "proposed by" line of the pending fact is still shown, so a person sees when an agent vouches for an agent's sentence |
| N6 | SC2 | show an approved alignment change as applied while its delivery job is only queued | judgment | Delegated (driver D129): the reviewer checks the wording of the approved phrase and rule proposal rows in the Review tab and in the answer's body |
| N7 | SC3 | let a finalized token operation run without its ledger row or outside the rate limit | judgment | Delegated (driver D129): the reviewer checks that the mapping decision and both runs go through the shared operation layer's limit check and write their `applied` row with the change |
| N8 | SC4 | narrow the kind-word route for a token that uses it today | criterion SC4 | Delegated (driver D125) |
| N9 | SC6 | give a token an outcome stronger than its row of the ruled table through another route, tool or parameter | judgment | Delegated (driver D129): the reviewer checks that every new token entry asks the compiled policy for its operation, that no propose-only operation has a path that applies it, and that a read-scope or below-editor caller reaches no write over either surface |
| N10 | SC7 | show a token-agent action as the person's own decision, or without the token's name | criterion SC7 | Delegated (driver D129) |
| N11 | SC8 | describe a propose-only operation as something the agent does itself | judgment | Delegated (driver D129): the reviewer checks that ADR 0076, the API and MCP documentation and every new tool description say for each operation whether it finalizes or proposes, and what a person must do to answer a proposal |
| N12 | SC9 | change what a signed-in person's decision does in any of these sections | criterion SC9 | Delegated (driver D129) |

## Scope: in

- Propose-only operations, each landing in the proposal store and applied only when a signed-in person approves: confirm fact, reject fact, close fact; resolve conflict (close old, retime new, keep both, reject new); alignment phrase decision and alignment rule decision; errata decision (approve or reject a held action); ontology proposal decision (reject or adopt); violation decision; defect decision; a proposed answer to a pending fact; a proposed answer to a governor decision (route and tool: see Open questions).
- Finalized operations through the policy function: mapping decision, and the consistency check and the inference run, each writing an `applied` ledger row and counted against the per-token rate limit (Delegated, driver D126).
- The alignment kind-word decision keeps finalizing for write tokens exactly as today; it gains a ledger row, attribution and an MCP tool, and its existing route behaviour is not narrowed (Delegated, driver D125).
- Token and MCP reads of mappings and of open ontology proposals, with the fields a signed-in person sees, so an agent can find what it decides or proposes on (Delegated, driver D124).
- One MCP write tool per operation family for all of the above, through the shared operation layer, listed and callable only with scope `write` and an owner at least Editor, with REST codes in tool errors.
- The open proposal carried on the item in each section's token and MCP reads and shown on the item's card in the Review tab; approval, decline, supersession and the session bulk answer work for every proposal kind.
- Stable codes on every write: the scope and role refusals, `already_decided`, `already_proposed`, `agent_rate_limited`.
- Ledger rows and audit rows with owner and token for every operation, visible in Agent History and Activity with readable labels for token rows; the Agent rail badge counts open token proposals (Delegated, driver D128).
- Program close: ADR 0076 final against what shipped, and complete API and MCP documentation for every opened route and tool, including which operations finalize, which propose, and what a person must do to answer a proposal. The documentation fixes ruled below (audit queue 39 to 42) and the corrected `review_batch` tool description (Delegated, driver D128) are part of it.
- The existing tests that assert a token is refused on a route this milestone opens change with the behaviour.

## Scope: out (vetoes)

- A token finalizing any `pending_facts` decision, or any change to how `remember` waits for a nod: the nod stays with people (charter veto).
- A token approving or declining any proposal, its own or another agent's.
- New undo routes for confirm, reject, close, conflict, errata, ontology, violation or defect decisions: not in charter scope; these stay propose-only for that reason.
- Batch routes for sections other than Duplicates.
- Widening the Decisions section's four action families: accepted by the synthesis; Agent History and Activity carry the rest.
- Migrating `pending_facts` or the errata store into the ledger.
- Any change to a signed-in person's existing decisions in these sections (do not break a signed-in person's Review tab, its routes, their responses or their audit rows). The missing audit row on a person's ontology proposal rejection stays as it is.
- Any change to the M003 behaviour of keep, merge, revert, batch, the breaker and the proposal answer for merges and reverts.
- Activity labels for a person's own Review actions, a sweep that closes stale proposals without a person, MCP trimming of `action`, and the audit row of a refused MCP call: observed in the M003 final review, left as they are (Delegated, driver D128).
- Widening a token beyond its owner or its scope: a read-scope token, or a token whose owner is below Editor, gains no Review write (charter veto 3).
- A configurable per-base table, cap or limit; an idempotency key.
- Anything sent to the upstream repository or the owner's deployed instance.

## Constraints

- Stack is the existing one: Rust workspace (axum 0.8, sqlx, Postgres 16 with pgvector), React 19 / Vite web app; no new framework or service (CHARTER.md Constraints).
- Program synthesis decisions are settled (`.project/SYNTHESIS.md`): one shared operation function per Review operation for REST and MCP; the session branch of every handler keeps today's code; the policy function lives with the `execution_gate` module and holds the compiled-in table; the ledger `token_agent_actions` is the proposal store with one open proposal per base, operation and target; the answer route takes a session only; attribution is owner plus token; one MCP write tool per operation family; codes `already_decided`, `already_proposed`, `agent_rate_limited` (429), `token_read_only`, `token_owner_below_editor`; limit 200 finalized writes per rolling hour.
- The owner's authority ruling binds: "Option 1: the recommended per-operation table" (owner, 2026-11-18, SYNTHESIS.md User rulings). This milestone makes reachable every row M001 and M003 left closed, each with the ruling the table states.
- The MCP tool-name rules pinned by the M003 tests stay: no listed tool name contains `proposal`, `approve`, `decline` or `answer`, and `review_confirm`, `review_decide`, `review_pending` and `review_approve` stay unknown tools (codebase map F20), unless decide records a reason to change one of those tests.
- Delegated (driver D005): fork first, upstream later. Work lands on `evolv3ai/utopia` `dev` in `direct` mode. Upstream conventions bind: an ADR before a public-API or data-model change (ADR 0076 is amended by dated revisions), Chinese explanatory comments, sentence-named tests and migrations, one behaviour-sentence commit subject per change. Nothing is sent to `deeplethe/utopia`.
- Delegated (driver D102): DCO `Signed-off-by` is deferred to the upstream submission.
- Delegated (driver D006, D076): store and server behaviour is verified against a migrated Postgres 16 + pgvector with `UTOPIA_TEST_REQUIRE_DB=1` — the compose `db` service on `127.0.0.1:1517`, one database per task Verify, the `utopia-store` tests on their own freshly migrated database. Tests seed through store functions or direct inserts, not through model jobs.
- Delegated (driver D007): this machine's toolchain is acceptable Verify evidence. Host rules from `.project/LESSONS.md` apply: a Verify builds into its own empty `CARGO_TARGET_DIR` outside the worktree; PostgreSQL 16 client tools and the `tee` shim are on PATH; `psql` options come before the connection string.
- Do not run the full-repository suite as a task Verify for a narrow change; each task Verify names its own tests and its own database.
- A migration, if decide needs one, takes the lowest number above every merged and upstream-claimed number (0130 by the local count), is written idempotent, is checked with `scripts/check-upstream-numbers.sh` (a read of public upstream data; nothing is sent), and the same task bumps `CURRENT_SCHEMA_VERSION` in `crates/utopia-cli/src/main.rs` by one per file.
- Lessons bind the plan (`.project/LESSONS.md`): every existing test that asserts a refusal this milestone lifts is in the files of the task that lifts it; a task that adds a field, variant, tool or list entry carries every file with a full literal, exhaustive match or exact-shape assertion it breaks; a Verify filter uses the mounted module path and matches at least one test; a brief repair is committed before activation. The one test function that holds eleven of the 401 assertions (`review_token_reads_tests.rs`, map F19) must not be edited by two tasks that can run at once.
- Review tab criteria (SC2, and the kind-word and attribution parts of SC4 and SC7) are walked through in a browser on a running build at final review; the web test setup renders components statically and clicks nothing.
- No budget, deadline, paid service or deployment is in scope. The owner's vibecoding.bot instance is not touched.

## Current state (brownfield only)

- **What exists**: At `d5d2356` a token reads thirteen sections and writes keep, merge, manual merge, revert and duplicate-pair batch through `crates/utopia-server/src/api/review_ops.rs`, with six MCP review tools in `review_tools.rs`. The policy enum in `crates/utopia-store/src/execution_gate.rs` already holds all nineteen rows of the ruled table; only Keep, Merge and RevertMerge have callers, and "run consistency check / inference" is one row. Every remaining Review write is a working session-only handler (confirm, reject, close fact; resolve conflict; nod; governor answer; violation, defect, errata; alignment phrase and rule, which answer 202 with a job id; ontology proposal reject and adopt; mapping; consistency check; inference run) and answers a token with 401. The kind-word route already takes a write token and finalizes with 200, with no ledger row, no token name in audit, no rate limit and a plain 403 for a below-editor owner. A token cannot list mappings (`unknown_queue`) or ontology proposals (session-only route; they are keyed by section and key, and a person's reject writes no audit row and answers ok on a missing proposal). The proposal machinery is specific to merges: `land_proposal` is private and called by three merge functions, `held_back` errors for a propose-only row, the answer route dispatches on three operation strings and returns a merge id, `superseded` is detected from merge error texts, `agent_proposal` attaches to Duplicates items and Merges rows only, the Agent History row prints a pair, and the web label tables are closed lists. The ledger's `target_id` is `UUID NOT NULL` while a kind word, an ontology proposal and a run have no single uuid. Decided, missing and stale answers differ per operation, and confirm fact, mapping, alignment phrase and rule have no decided state. `CURRENT_SCHEMA_VERSION` is 115; the highest migration is 0129.
- **Must not break**: a signed-in person's decisions in every section and their responses and audit rows; the M001 keep, the M002 reads and the M003 merge, revert, batch, breaker, limit and proposal answers; every route and tool that stays closed to tokens (the proposal answer routes, Activity, the nod as a finalizing act); the governor's precedent reads and fuse; the nod gate on `remember`; the kind-word route's answers for the tokens that use it today.
- **Doc-vs-code rulings**: recorded in `research/DOCS-AUDIT.md` `## User rulings`. Queue 39 (ADR 0076 `## Numbers` omits migration 0129): fix-doc. Queue 40 (ADR 0076 open question "The precedent exclusion has not been exercised end to end"): fix-doc, reworded to what the store and HTTP tests cover and that no full govern round with a model was run after a token write. Queue 41 (`docs/design/access-and-audit.md` says every route apart from MCP is session-only): fix-doc. Queue 42 (`docs/design/chat-and-mcp.md` says an agent over MCP can only propose): fix-doc. All four: Delegated (driver D123), planned into this milestone's documentation task. Earlier rulings (queue 1, 4, 6, 11, 16, 17, 20, 28: fix-doc; 13, 14, 38: accept-drift) stand; the eight older fix-doc rulings are queued, none planned yet.
- **Ground truth**: `.project/research/evidence-codebase.md`, `.project/research/DOCS-AUDIT.md`

## Risks

- Twelve operation families in one milestone; they are uniform (propose, then a person applies the existing session function) but touch many store paths, and several of those paths are not one transaction.
- Approval fatigue: outside Duplicates there is no bulk path today, so the session bulk answer is the only relief on a large base, and it can reproduce blind acceptance.
- A proposal can go stale while it waits (a fact rejected, a conflict resolved, an errata action answered); each kind needs a correct `superseded` rule, and four kinds have no decided state to detect.
- Alignment phrase and rule approvals start asynchronous delivery jobs; a proposal's approval must surface that the change is queued, not applied.
- Bulk approval of proposed answers to pending facts lets a person nod many remembered sentences in one action, which is closer to the nod gate than any other proposal kind; an agent can propose confirming a sentence an agent remembered.
- The consistency check row has the thinnest evidence in the table; a run is synchronous, has no in-flight guard and is bounded only by the rate limit.
- The ledger's target column is a non-null uuid and three targets have none; a schema change means a migration and an upstream number check.
- One existing test function holds eleven refusal assertions that different tasks must lift.
- The Review tab has no automated click test; criterion 2 rests on a browser walkthrough plus static-render and API tests across nine sections.
- The ontology adoption reason rests on the M001 confirmation; a correction changes the ADR text, not the ruling.

## Open questions

- [RESEARCH] The ruled table makes "answer a governor decision" propose only for a token, while the synthesis keeps the governor's own decision route session-only. Through which token route and MCP tool does an agent propose an answer to a governor decision, given that in code that answer is a second way to merge, keep, revert or manually merge a pair as a person (map F12)? The behaviour is ruled in edge E6; the route and tool are open.
- [RESEARCH] How does the ledger name a target that has no single uuid (a kind word, an ontology proposal keyed by section and key, a run on the base), and does that need migration 0130 (map F4)?
- [RESEARCH] Where do the token and MCP reads of mappings and open ontology proposals sit: a new section name on the existing list, or their own routes opened to a token (map F11, F12)?
- [RESEARCH] How is "the target can no longer be decided" detected per operation without matching error texts, so that approval closes a stale proposal as `superseded` (map F5, F22)?
- [RESEARCH] Which MCP tool names and families satisfy "one write tool per operation family" under the name rules the M003 tests pin (map F20)?
- [RESEARCH] Does a token's proposal enter through the person's route with a token branch (the nod route included) or through a separate route, given the tests that pin 401 on those routes and the charter veto on the nod (map F13, F19)?

## Corrections

<!-- Verbatim user corrections from playback and later phases. Append-only,
     numbered from C1. -->
- C1 2026-11-20: Delegated (driver D131, D132, D134), from research and planning, before any task was activated. (a) Edge E19 misstated two shipped refusals. Evidence: a read-scope or below-editor caller of an M003 write tool gets a tool error carrying `token_read_only` or `token_owner_below_editor`, not an unknown tool (`crates/utopia-server/src/api/mcp.rs:210-233`, `review_tools.rs:482-486`, `review_mcp_tests.rs:783-799`; evidence-domain.md finding D18), and a write token whose bases exclude the base gets 403 `token_kb_out_of_scope` from `review_ops::authorize_write`, not a not-found (planner finding, PLAN.md Dependency notes). Ruling: the shipped refusals hold and E19 is reworded to them; no code changes for this. (b) Criterion 2's "on the item's card in its own section" is read, for an ontology proposal, as the proposal's card on the Ontology page plus the Agent History section of the Review tab, because an ontology proposal has no card in the Review tab and a person decides it on the Ontology page (SYNTHESIS.md User rulings, driver D132); no Review section is added. Success criteria SC1 to SC9 are unchanged in wording.
