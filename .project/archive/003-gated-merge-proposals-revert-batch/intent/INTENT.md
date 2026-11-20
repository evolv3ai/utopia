# Intent — utopia M003 gated-merge-proposals-revert-batch

<!-- Written by /gsd-path-define in milestone + brownfield mode. Derived from
     the approved ROADMAP.md entry M003 and CHARTER.md; no re-interview.
     Constraints and vetoes here override everything downstream. -->

Lane: milestone
<!-- Derived from the approved roadmap entry M003 (owner approval of the roadmap, driver D025). The entry lists no open questions, so research and decide do not run for this milestone. -->

Review panel: off
<!-- Copied from CHARTER.md (Delegated, driver D008). -->

Finding skeptics: off

Surfaces: token-authenticated HTTP API, MCP endpoint, Review tab in the web app (Agent History, Duplicates and Merges sections)

## Summary

After M002 an agent holding a personal token reads every Review section but can change one thing only: it can keep a duplicate pair. This milestone gives a write-scope token of an editor the three writes the owner ruled "finalize behind the server gate": merge (the queue decision and the manual merge), revert of a merge, and batch on duplicate pairs. Every token merge passes the ADR 0027 merge gate first; a call the gate holds changes nothing and waits as a proposal that only a signed-in person can answer in the Review tab. Every agent merge can be reverted by the agent or by a person, a batch has an identity and a unit revert, and finalized token writes are rate limited and guarded by a per-token breaker. Source: ROADMAP.md entry M003; program spec `C:\Users\Owner\.gsd-path-driver\projects\utopia-ef4aa95f\SPEC.md`.

Lines marked `Delegated (driver D0NN)` were answered for the owner by the gsd-path driver under the owner's consent of 2026-11-19 ("you have my consent to be the human") and can be overruled; the log is `C:\Users\Owner\.gsd-path-driver\projects\utopia-ef4aa95f\DECISIONS.md`.

## Problem

The spec's example is a base where "'Cursor' is eight entities, 'Claude Code' is four", and "Clearing that by hand, one click at a time, is not feasible, and the agents that can already read the graph and spot these problems cannot act on them" (spec, Goal). At `6c52ce8` a token's merge on the queue decision route answers 403 `token_operation_not_open`, and the manual merge, revert and batch routes answer a token with 401. The spec also sets the limit on the fix: "The milestone must also decide which operations an agent may finalize and which it may only propose", because "a document can talk an agent into recording something; the same risk applies to an agent confirming or merging facts on the strength of text it just read" (spec, Goal). The owner ruled merge, revert and batch "finalize behind the server gate" (SYNTHESIS.md, Agent-authority policy). Today no person's merge path calls that gate, the gate knows nothing of pairs a person kept or merges that were reverted, there is no store for a held call, no batch identity, no rate limit, and no stored link from a merge to the token that made it.

## Users

- An agent (MCP client or script) holding a write-scope personal token whose owner is an editor of the base: merges, reverts and batches duplicate pairs, and reads back what happened to each call.
- A signed-in editor in the Review tab: answers the agent's proposals in Agent History, sees an open proposal on the pair's card, and reverts an agent merge in the Merges section.
- A signed-in base Admin: reads every agent action in Activity.
- Technical users; today all of this is done by hand in the browser.

## Success criteria

1. Over the token API and over MCP, a write-scope token owned by an editor merges a duplicate pair the gate passes: the entities merge, the response names the merge, and a ledger row with outcome `applied` exists.
2. The same call on a pair the gate holds (a functional contradiction, a live derivation, an entity named in an answer, an identity source, a pair a person kept, or a pair with a reverted merge) changes nothing in the graph and lands one proposal with the gate's reason; a repeat answers `already_proposed`.
3. In the Review tab, a signed-in editor sees that proposal in the Agent History section and on the pair's card in Duplicates, approves it, and the merge is applied with the person as actor; a declined proposal changes nothing; the editor answers several proposals in one action.
4. A write token that calls the proposal answer is refused, over the token API and over MCP.
5. Over the token API and over MCP, the agent reverts its own merge and the graph returns to its state before the merge (facts, entities and evidence compared before and after); a second revert answers `already_decided`.
6. A token revert of a merge made by a person or by the governor lands as a proposal, changes nothing, and does not count toward the governance fuse.
7. In the Review tab, a signed-in editor reverts an agent merge from the Merges section and the graph returns to its state before the merge.
8. A token batch of 51 ids is refused; a batch of mixed pairs returns per-item outcomes with gated members as proposals and a batch id; the unit revert of that batch reverts every merge it made.
9. A token that exceeds the finalized-write limit receives `agent_rate_limited`; after two reverts by a person of one token's merges within seven days that token's next merge on the base lands as a proposal.
10. Every merge, revert, batch member and proposal appears in Agent History and in Activity as "owner via token name", batch members carrying the batch id.
11. A signed-in person's merge, 500-id batch and revert behave as before, and a token merge does not lower the governor's bar or start a govern round.

## Edge coverage

<!-- Rulings marked Delegated were given for the owner by the driver. -->

| Edge | Criterion | Category | Disposition | Detail |
|------|-----------|----------|-------------|--------|
| E1 | SC1 | idempotency | held-out | Delegated (driver D106): a token merge of a pair that is already decided, by anyone, answers `already_decided`; no second merge happens and no second `applied` ledger row is written |
| E2 | SC1 | concurrency | held-out | Delegated (driver D106): when a person, the governor and a token decide one pair at once exactly one decision is applied and a losing token call answers `already_decided`, not a not-found or an uncoded conflict; a pair locked by a running governor round gets the refusal the M001 keep returns for it today |
| E3 | SC1 | adjacency | held-out | Delegated (driver D106): a token manual merge of an entity with itself keeps the existing `self_merge` refusal; an entity already merged into another answers `already_decided`; an entity id that is unknown or belongs to another base is not found; none of these lands a proposal or changes the graph |
| E4 | SC1 | ordering | held-out | Delegated (driver D106, D099): a token queue merge merges in the direction the person's queue merge takes for that pair; a token manual merge takes the caller's source and target, and when it applies it closes that pair's own pending review as merged with the token recorded |
| E5 | SC2 | adjacency | held-out | Delegated (driver D098): each of the six hold reasons lands a proposal whose reason names that hold. "A pair a person kept" means the latest decided review of those two entities, in either order, was decided keep by a person and not by a token; "a pair with a reverted merge" means any merge between those two entities, in either direction, has been reverted. Both are looked up by entity ids, for the queue merge and the manual merge alike |
| E6 | SC2 | idempotency | held-out | Delegated (driver D107, D098): there is one open proposal per base, operation and target; a queue merge's target is its review id and a manual merge's target is the unordered pair of entity ids; a repeat, or a second call arriving at the same moment, returns the existing proposal's id with `already_proposed` and writes no second row |
| E7 | SC2 | empty | criterion SC2 | "Changes nothing in the graph" is read as: the facts, entities, evidence and review rows of the base are equal before and after the held call (Delegated, driver D107) |
| E8 | SC3 | idempotency | held-out | Delegated (driver D107): answering a proposal needs a signed-in Editor of the base; a proposal that was already answered answers `already_decided` and nothing changes; approval runs the person's own merge or revert path, which calls no gate |
| E9 | SC3 | concurrency | held-out | Delegated (driver D107): approving a proposal whose target can no longer be decided (the pair was decided meanwhile, or an entity was merged away) closes it as `superseded`, applies no merge, and the answer says so |
| E10 | SC3 | boundary | held-out | Delegated (driver D107): one bulk answer takes 1 to 500 proposal ids and returns one outcome per id; an empty list or 501 ids is refused as a whole and answers nothing |
| E11 | SC4 | adjacency | held-out | Delegated (driver D107): the proposal answer route, single and bulk, answers every personal token with 401 as the other session-only Review routes do; the MCP endpoint lists no tool that answers a proposal and a call to an invented one is an unknown tool; the proposal stays open |
| E12 | SC5 | precision | held-out | Delegated (driver D094, corrected by C1, driver D116): "state" is every `facts` row of the base, every `entities` row except `updated_at`, and every `fact_evidence` row, compared for equality before the merge and after the revert; for `typed_fact_sources`, every row present before the merge is present after the revert, and the only rows added are statement links the merge copied onto a surviving fact from a duplicate typed fact it deduplicated. Review rows, recorded signature violations and `updated_at` are outside the comparison. The existing revert is not changed: a target that had no type before the merge keeps the type it took from the source, and a surviving fact keeps the statement links the merge copied onto it; the comparison is made on typed entities and those two differences are documented |
| E13 | SC5 | idempotency | criterion SC5 | A second revert answers `already_decided` |
| E14 | SC5 | adjacency | held-out | Delegated (driver D105): a token reverts outright a merge that the ledger shows a token agent made on that base, whichever token, and that has not been reverted; a merge id that is unknown or of another base is not found |
| E15 | SC6 | idempotency | held-out | Delegated (driver D105, D107): a merge a person made, including one applied by a person approving a token's proposal, and a merge the governor made are proposals when a token reverts them; a repeat answers `already_proposed`; when a person approves the proposal the revert runs as that person's and the fuse counts it as it does today |
| E16 | SC7 | idempotency | held-out | Delegated (driver D110): after a person reverts an agent merge the merge's ledger row shows the later state `undone`, and a token revert of the same merge answers `already_decided` |
| E17 | SC7 | encoding | held-out | Delegated (driver D096): a merge made by a token agent is shown in the Merges section, and returned in the merges list to session and token readers, as "owner via token name" from the ledger link; every other merge row is unchanged |
| E18 | SC8 | boundary | held-out | Delegated (driver D108): a token batch takes 1 to 50 ids; 0 or 51 ids is refused as a whole with the existing `batch_size` code, nothing changes and no ledger row is written; an action other than merge or keep refuses the whole batch |
| E19 | SC8 | ordering | held-out | Delegated (driver D108): outcomes come back in the order of the request, one per id, each with its outcome (`applied`, `proposed` or an error) and stable code; an unknown id is that item's `not_found` and does not fail the batch; an id given twice gets, the second time, the answer its state then gives |
| E20 | SC8 | idempotency | held-out | Delegated (driver D108, D103): a unit revert reverts each not yet reverted merge of the batch and reports each already reverted one as `already_decided`; a second unit revert changes nothing; keeps are left as they are; an unknown batch id is not found. A write token of an editor and a signed-in editor may both call it; no Review tab button is added for it |
| E21 | SC9 | boundary | held-out | Delegated (driver D109): the limit is 200 finalized writes per token and base in a rolling hour, a named constant; the 200th applies and the next answers `agent_rate_limited` with HTTP 429, changes nothing and writes no `applied` row; proposals and refused calls do not count; keep, merge and revert count; inside a batch every item past the limit gets `agent_rate_limited` as its own outcome |
| E22 | SC9 | boundary | held-out | Delegated (driver D109, D104): the breaker trips at the second revert by a person of one token's finalized merges on a base within seven days; one such revert does not trip it; reverts made by a token and reverts of another token's merges do not count; a tripped token's merge and revert, batch merge members included, land as proposals whose reason names the breaker; its keep still finalizes; when the seven days have passed the token finalizes again |
| E23 | SC10 | encoding | held-out | Delegated (driver D110, D101): the token name shown is the snapshot stored with the action, whatever happens to the token afterwards; batch members carry the batch id in Agent History and in Activity; every operation and every state this milestone writes (`approved`, `declined`, `superseded`, `undone` included) has a label in the web app and none is shown raw; Activity is read by a signed-in base Admin |
| E24 | SC11 | idempotency | held-out | Delegated (driver D110, D100): the existing session tests for merge, batch and revert pass; the only edits to existing tests are the assertions that a token is refused on merge, manual merge, revert and batch, and every assertion for a route that stays closed keeps its expectation |
| E25 | SC11 | precision | held-out | Delegated (driver D097): a live token merge counts in no governor precedent read; a merge made by a token and reverted by a token counts in none either; a person's revert of a token merge counts as that person's revert precedent as any revert does. No token action lowers the governor's bar, settles a pair as decided by people, or starts a govern round |

## Prohibitions

| Prohibition | Criterion | Must not | Disposition | Detail |
|-------------|-----------|----------|-------------|--------|
| N1 | SC1 | let any token merge reach the merge without the server gate being asked first | judgment | Delegated (driver D111): the reviewer checks that every token merge entry — queue decision, manual merge, batch member, each over REST and MCP — calls the gate before the merge, and that no token caller reaches the session branch's ungated code — charter veto on straight-through agent writes |
| N2 | SC1 | report a merge as failed, or leave it without its ledger and audit rows, when the entities were merged | judgment | Delegated (driver D095, D111): the reviewer checks that the ledger row naming the merge and the audit row are written with the merge, and that a failure of a later follow-up step is reported on an `applied` result |
| N3 | SC2 | change a fact, entity, review row, retrieval or reasoning on a held or propose-only call | criterion SC2 | Delegated (driver D111) |
| N4 | SC3 | record an approved proposal as the agent's act, or hide which proposal it came from | criterion SC3 | Delegated (driver D111): the merge is applied with the person as actor and its audit detail names the proposal |
| N5 | SC4 | let any token approve or decline a proposal, its own included | criterion SC4 | Delegated (driver D111) |
| N6 | SC5 | describe a revert as a full undo | judgment | Delegated (driver D094, D111): the reviewer checks that ADR 0076, the API and MCP documentation and the revert tool description say that a revert does not recall derivations, violations, exports or answers drawn while the merge was live, and name the untyped-target difference and the copied statement links a revert leaves (C1) |
| N7 | SC6 | let a token finalize a revert of a person's or the governor's merge, or move the governance fuse | criterion SC6 | Delegated (driver D111) |
| N8 | SC7 | show a merge a token agent made as the owner's own merge in the Merges section | judgment | Delegated (driver D096, D111): the reviewer checks that the Merges row for a token-made merge reads "owner via token name" |
| N9 | SC8 | give a batch member anything the single-item token path would refuse or hold | judgment | Delegated (driver D111): the reviewer checks that each member runs through the single-item token path, so gate, limit and breaker apply per member |
| N10 | SC9 | let a rate-limited or breaker-tripped token finalize through another entry | judgment | Delegated (driver D111): the reviewer checks that the limit and the breaker sit in the shared operation layer and so hold for the queue merge, the manual merge, batch members and the MCP tools alike |
| N11 | SC10 | show a token-agent action as the person's own decision or as a governor row | criterion SC10 | Delegated (driver D111) |
| N12 | SC11 | change what a signed-in person's merge, 500-id batch or revert does, or let a token action steer the governor | criterion SC11 | Delegated (driver D111) |

## Scope: in

- Token merge, both the queue decision and the manual merge, through the policy function: every token merge calls the ADR 0027 merge gate first, and the gate also holds a pair a person kept or whose merge was reverted. A pass applies the same store function the person's path calls; a hold becomes a proposal carrying the gate's reason, never an error and never a merge.
- The proposal store: a held or propose-only call waits as a ledger row holding the full request and changing no fact, entity, review row, retrieval or reasoning. One open proposal per base, operation and target; a repeat returns the existing proposal with `already_proposed`.
- The proposal answer, session only: a person approves or declines in the Agent History section; approval runs the existing session store function with the person as actor and names the proposal in the audit detail; a target that can no longer be decided closes the proposal as `superseded`. A person may answer many proposals in one request, capped at 500. No token may answer a proposal.
- The open proposal shown on the item's card in the Duplicates and Merges sections and carried on those items in token and MCP reads.
- Token revert of a merge: finalized when the merge was made by a token agent and has not been reverted; a revert of a person's or the governor's merge is a proposal, so a token cannot trip the governance fuse. A person reverts an agent merge in the Merges section as today.
- A stored link from a merge to the token agent that made it, shown in the Merges section as "owner via token name" (Delegated, driver D096).
- Token batch on duplicate pairs: at most 50 ids, each through the single-item token path with per-item outcomes, a server-issued batch id on every ledger and audit row, and a unit revert of a batch that reverts its not yet reverted merges best-effort with per-item outcomes, callable by a write token of an editor and by a signed-in editor (Delegated, driver D103).
- Per-token and per-base rate limit on finalized writes computed from the ledger, answering `agent_rate_limited` with HTTP 429, applied to every finalized token write including keep.
- Per-token breaker: two reverts by a person of one token's finalized merges on a base within seven days make that token propose-only for merge and revert on that base for the rest of the window (Delegated, driver D104).
- MCP write tools for the merge, revert and batch families through the shared operation layer, with the REST codes in tool errors.
- Ledger later states (`approved`, `declined`, `superseded`, `undone`) and attribution of merges, reverts, batches and proposals in Agent History and Activity.
- Governor precedent reads exclude merges made and reverted by tokens (Delegated, driver D097).
- ADR 0076 and the API and MCP documentation extended for these routes and tools, stating that a revert does not recall derivations, violations, exports or answers drawn while the merge was live.
- The existing tests that assert a token is refused on merge, manual merge, revert and batch change with the behaviour (Delegated, driver D100).

## Scope: out (vetoes)

- Straight-through agent writes: no token merge without the gate — "Without it the factory is likely to build straight-through agent writes and remove the safeguard the product was designed around." (spec, Hard constraint)
- Proposals for operations other than merge and revert: M004.
- Batch for anything other than duplicate pairs; an idempotency key.
- Token answers to the governor's own decisions: the governor's decision route stays session-only; see M004.
- The session caller's ungated merge, its 500-id batch and its revert: unchanged. The governance fuse keeps counting a person's reverts as today. The existing `revert_merge` store function is not changed (do not break a signed-in person's merge, batch or revert) (Delegated, driver D094).
- A configurable per-base table, cap or limit.
- The nod gate on `remember` and every `pending_facts` decision: unchanged, never finalized by a token (charter veto).
- Widening a token beyond its owner or its scope: a read-scope token, or a token whose owner is below Editor, gains no Review write (charter veto 3).
- Every Review write not named in Scope in (confirm, reject, close, resolve, violations, defects, alignment, errata, ontology, mappings): stays closed to tokens until M004.
- A Review tab button for the unit revert of a batch; opening Activity to tokens or to roles below Admin.
- Anything sent to the upstream repository or the owner's deployed instance.

## Constraints

- Stack is the existing one: Rust workspace (axum 0.8, sqlx, Postgres 16 with pgvector), React 19 / Vite web app; no new framework or service (CHARTER.md Constraints).
- Program synthesis decisions are settled (`.project/SYNTHESIS.md`): handlers change extractor in place; one shared operation function per Review operation for REST and MCP; the session branch of every handler keeps today's code; the policy function lives with the `execution_gate` module and holds the compiled-in table; the ledger `token_agent_actions` is the proposal store; attribution is owner plus token; codes `already_decided`, `already_proposed`, `agent_rate_limited` (429), `token_read_only`, `token_owner_below_editor`; token batch cap 50, limit 200 finalized writes per rolling hour, both named constants; the breaker uses the governor's constants; unit revert at `POST /kbs/{id}/review/agent-batches/{batch_id}/revert`; no idempotency key.
- The owner's authority ruling binds: "Option 1: the recommended per-operation table" (owner, 2026-11-18, SYNTHESIS.md User rulings). This milestone makes reachable exactly its rows Merge, Revert merge and Batch, beside M001's keep.
- Delegated (driver D005): fork first, upstream later. Work lands on `evolv3ai/utopia` `dev` in `direct` mode. Upstream conventions bind: an ADR before a public-API or data-model change (ADR 0076 is amended; a new record number, if one is needed, is checked with `scripts/check-upstream-numbers.sh`), Chinese explanatory comments, sentence-named tests and migrations, one behaviour-sentence commit subject per change. Nothing is sent to `deeplethe/utopia`.
- Delegated (driver D102): DCO `Signed-off-by` is deferred to the upstream submission; it is the owner's personal attestation and the pipeline's landing commits have a fixed format.
- Delegated (driver D006, D076): store and server behaviour is verified against a migrated Postgres 16 + pgvector with `UTOPIA_TEST_REQUIRE_DB=1` — the compose `db` service on `127.0.0.1:1517`, one database per task Verify, the `utopia-store` tests on their own freshly migrated database. Tests seed through store functions or direct inserts, not through model jobs. A plain `cargo test` without a database is not evidence.
- Delegated (driver D007): this machine's toolchain (Windows, Node 26, pnpm 10.14) is acceptable Verify evidence. Host rules from `.project/LESSONS.md` apply: a Verify builds into its own empty `CARGO_TARGET_DIR` outside the worktree; PostgreSQL 16 client tools and the `tee` shim are on PATH; `psql` options come before the connection string.
- Do not run the full-repository suite as a task Verify for a narrow change; each task Verify names its own tests and its own database (SYNTHESIS.md, Verification stack).
- Every migration this milestone adds takes the lowest number above every merged and upstream-claimed number, is written idempotent, and the same task bumps `CURRENT_SCHEMA_VERSION` in `crates/utopia-cli/src/main.rs` by one per file (`.project/LESSONS.md`).
- Lessons of M002 bind the plan (`.project/LESSONS.md`): every existing test that asserts a refusal this milestone lifts is in the files of the task that lifts it, no later Verify freezes such a file, and a task that changes behaviour an earlier task's Verify pins declares `supersedes` and carries that Verify's remaining checks.
- Review tab criteria (SC3, SC7) are walked through in a browser on a running build at final review; the web test setup renders components statically and clicks nothing (codebase map).
- No budget, deadline, paid service or deployment is in scope. The owner's vibecoding.bot instance is not touched.

## Current state (brownfield only)

- **What exists**: At `6c52ce8` a token reads all thirteen sections and makes one write, keep, through `crates/utopia-server/src/api/review_ops.rs`, the MCP tools in `review_tools.rs` (`review_list`, `review_item`, `review_keep`) and the ruled table in `crates/utopia-store/src/execution_gate.rs` (`agent_ruling`, `agent_policy`; Merge, RevertMerge and Batch are already `FinalizeBehindGate` rows with no caller). The token branch of the queue decision route refuses every action but keep with 403 `token_operation_not_open`; `batch`, `revert_merge` and `manual_merge` take a session only and answer a token with 401. `hold_for_merge` has four hold reasons (identity source, functional contradiction, live derivation, entity named in an answer), consults neither kept pairs nor reverted merges, and is called only by the adjudicator and the governor; no person's path calls it. A person's queue merge is read-then-write with no status guard and a best-effort audit row; the session batch is sequential, capped at 500, with a size-only `batch` detail and silently droppable audit rows. `merge_entities` commits and then runs follow-up steps that can fail after the merge. `revert_merge` restores facts, evidence and the target's profile but leaves review rows, recorded signature violations and a type an untyped target took from the source. `entity_merges` records a user id and a free-text reason and nothing about a token. The ledger `token_agent_actions` (migration 0128) has outcome `applied|held|proposed` and no later-state, gate-reason, batch-id, undo-reference or answerer column and no unique index on open proposals. The governance fuse counts reverted governor rows per base (two in seven days). No rate limiter exists and no error variant renders 429. The Review tab is one 2,347-line component; `TokenAgentRow` renders a ledger row with no control; the Duplicates card already shows the governor's proposal chip; vitest renders components statically. `CURRENT_SCHEMA_VERSION` is 114, the migration file count; the highest migration number is 0128.
- **Must not break**: a signed-in person's merge, 500-id batch, revert, governor answers and Review tab; the M001 keep and the M002 reads with their codes and attribution; every Review route that stays closed to tokens; the MCP endpoint's existing tools and its 403 for an out-of-scope token; the governor's precedent reads and fuse; the nod gate on `remember`.
- **Doc-vs-code rulings**: recorded in `research/DOCS-AUDIT.md` `## User rulings`. The M003 audit at `6c52ce8` raised no new item needing a ruling; queue 35, 36 and 37 were fixed in M002 and are retired. Earlier rulings (queue 1, 4, 6, 11, 16, 17, 20, 28: fix-doc; 13, 14, 38: accept-drift) stand; the eight fix-doc rulings are queued, none planned yet.
- **Ground truth**: `.project/research/evidence-codebase.md`, `.project/research/DOCS-AUDIT.md`

## Risks

- A merge is the one finalized agent write that changes facts; what a wrong merge sent out of the graph is not recalled by revert, and criterion 5 is verified on graph state only.
- The gate was written for the product's own deciders; pointing it at token calls, and adding the kept-pair and reverted-merge holds, may hold far more or far fewer pairs than expected.
- Races between a person, the governor and a token on one pair have partial guards today; a lost race must map to `already_decided`, not a generic error.
- A person bulk-approving proposals can reproduce blind-acceptance errors; approved proposals count as human precedent as accepted governor proposals do today.
- Batch audit rows can go missing silently on the existing path; the token path must treat a failed snapshot or audit write as a failed item.
- The cap of 50 and the limit of 200 per hour are not measured on this product; a running batch is not stopped by token revocation, which the cap bounds.
- `merge_entities` commits before its follow-up steps; writing the ledger and audit rows with the merge may need the merge to run inside the token path's own transaction, which touches a function the person's path shares.
- The Review tab has no automated click test; criteria 3 and 7 rest on a browser walkthrough plus static-render and API tests.
- This is the largest milestone; if the gate, the proposal store and the batch cannot ship together it may be re-sliced with batch and breaker split out.

## Open questions

- None

## Corrections

<!-- Verbatim user corrections from playback and later phases. Append-only,
     numbered from C1. -->
- C1 2026-11-19: Delegated (driver D116), from build task T001: the comparison of edge E12 is corrected for `typed_fact_sources`. Evidence: the existing merge copies the statement links of each deduplicated typed fact onto the surviving fact (`crates/utopia-store/src/resolution.rs:2278` at `545b54c`, on a signed-in person's merge too) and the existing `revert_merge` removes only copied `fact_evidence` rows, so a merge followed by its revert leaves one extra `typed_fact_sources` row when both sides hold the same typed fact; `facts`, `entities` without `updated_at`, and `fact_evidence` compare equal (T001 Log, test `a_token_merge_then_its_revert_returns_facts_entities_and_evidence_to_their_state`). Ruling: `facts`, `entities` except `updated_at` and `fact_evidence` stay an exact comparison; for `typed_fact_sources` no row present before the merge may be missing after the revert, and the only added rows allowed are links the merge copied onto a surviving fact from a deduplicated duplicate's statements. `resolution::revert_merge` stays unchanged (SC11, Scope out). ADR 0076, the API and MCP documentation and the revert tool description name this leftover link beside the untyped-target difference (N6). Recording and removing the copied links in `revert_merge` is a candidate change for upstream and is not built in this program. Success criteria SC1 to SC11 are unchanged in wording.
