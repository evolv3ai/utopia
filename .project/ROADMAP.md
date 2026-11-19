# Roadmap — Agents can drive the Review tab

<!-- Written and owned by /gsd-path-roadmap. Rolling-wave: milestone-granularity
     scope for EVERY milestone; waves and tasks are planned only for the active
     milestone. Never edit a shipped entry except its Status, Archive, and
     Integrated fields; never edit an abandoned entry at all. Lives at
     .project/ top
     level and persists across milestones; it never archives. -->

## Milestones

### M001 — token-review-skeleton

Goal: An agent holding a personal token lists Duplicates and Overview and finalizes one keep through the server-side authority policy, over both the token API and MCP, with the action attributed to "owner via token name" in Agent History and Activity.
Depends on: []
Surfaces: token-authenticated HTTP API, MCP endpoint, Review tab in the web app (Agent History section), Activity view in the base settings of the web app
Status: shipped
Archive: .project/archive/001-token-review-skeleton
Integrated: null

Scope: in
- Decision record ADR 0076, written before the first route or schema change: the opened Review routes and MCP tools, the owner's ruled per-operation authority table with its reasons, the `token_agent_actions` ledger, attribution as owner plus token, the named constants (batch cap 50, 200 finalized writes per rolling hour), and the statement that a merge revert restores graph state only. The low-confidence "ontology adoption has no undo" reason is confirmed or corrected against the store before the record is final. ADR and migration numbers start at 0076 and 0128, re-checked against upstream before each landing; migration SQL is idempotent.
- Verification ground: the existing token read and token setup tests pass against the compose Postgres 16 + pgvector instance with `UTOPIA_TEST_REQUIRE_DB=1` (the program's proof that ADR 0014 decision 7 holds on this build), and per-run database isolation on that one instance is shown to work from this machine's toolchain.
- Read slice: a personal token (read or write scope, owner at least Viewer) lists the Duplicates section and the Overview summary through the same handlers the Review tab uses, and reads one duplicate item's evidence; a token whose `kb_ids` exclude the base gets 404.
- MCP read tools `review_list` (section parameter, existing 1 to 200 page clamp) and `review_item`, listed for any token that reaches the base with an owner at least Viewer; in this milestone they serve Duplicates and Overview.
- The shared operation layer: each Review operation is one server function taking the resolved caller (owner, token, base), called by both the REST token branch and the MCP tool. The session branch of every handler runs today's code unchanged.
- The server-side policy function, living with the execution gate module, holding the compiled-in authority table and returning `applied`, `held` or `proposed`. In this milestone only the keep row is reachable by a token.
- Write slice: keep on one duplicate pair with a write-scope token owned by an editor, over REST and over an MCP write tool that is listed and callable only with scope `write` and an owner at least Editor. Tool annotations are hints only; elicitation is not used.
- Stable refusal codes on that write: `token_read_only`, `token_kb_out_of_scope`, the new `token_owner_below_editor`, and `already_decided` for a lost race or repeated call; a refused MCP tool call carries the same code in its structured error.
- The ledger table `token_agent_actions` and one row per token-agent action; every token Review write also writes an audit row with the owner as actor plus `token_id` and a `token_name` snapshot. A failed snapshot or audit write fails the action.
- Precedent isolation: token decisions are never human precedent. The governor's precedent queries and its settled-by-people check exclude token rows, review rows carry the deciding token, and a token decision starts no govern round and settles no governor proposal.
- Attribution display: Agent History returns governor rows unchanged plus ledger rows told apart by a source field, shown as "owner via token name"; the Activity view gains optional token id, token name and declared client (the stored User-Agent, marked as declared) and renders token rows as "owner via token name" with a readable action label.
- Documentation of the routes and tools opened by this milestone in the product's API and MCP documentation.

Scope: out
- Token merge, manual merge, revert and batch: closed to tokens until M003, because a merge may not ship before the gate and the proposal store exist (charter veto on straight-through agent writes).
- Every propose-only operation and the proposal store: M003 and M004.
- Token listing of the other eleven sections and evidence for their items, proof routes, true section totals: M002.
- Per-token rate limit and breaker: M003.
- Any change to what a signed-in person sees or can do in the Review tab beyond the added token rows in Agent History and Activity; the person's ungated merge and 500-id batch stay as they are.
- The nod gate on `remember` and every `pending_facts` decision: unchanged, never finalized by a token (charter veto).
- A per-base authority setting, a base-level machine token or service account, MCP `clientInfo` as agent name: ruled out by the synthesis.
- Labels for existing machine actors in Activity (governor, adjudicator, errata agent): unchanged.
- Anything sent to the upstream repository or the owner's deployed instance.

Success criteria
1. Against the compose Postgres 16 + pgvector instance with `UTOPIA_TEST_REQUIRE_DB=1`, the existing token read and token setup tests pass, and two builds with different migration sets each migrate and test their own database on that instance.
2. Over the token API, a read-scope token of a base Viewer lists Duplicates and Overview and receives the same items and counts a signed-in editor sees in those sections of the Review tab; a token whose `kb_ids` exclude the base receives 404.
3. Over MCP, `review_list` returns the same Duplicates items and Overview summary as the token API for the same token, and `review_item` returns the evidence the Review tab shows on that pair's card.
4. Over the token API and over MCP, a write-scope token owned by an editor keeps one duplicate pair: the pair leaves the Duplicates queue, no fact or entity changes, and one ledger row with outcome `applied` and one audit row with `token_id` and `token_name` exist.
5. The same keep is refused with `token_read_only` for a read-scope token and with `token_owner_below_editor` for a write token whose owner is below Editor, over the token API and as an MCP tool error carrying the same code; a repeat of a decided keep answers `already_decided`; the MCP write tool is not listed for either refused caller.
6. A token call to merge, revert or batch changes nothing in the graph and is refused.
7. In the Review tab, a signed-in editor sees the token keep in the Agent History section as "owner via token name", beside unchanged governor rows.
8. In the Activity view, a signed-in base Admin sees the token keep as "owner via token name" with a readable action label.
9. A database-backed test shows a token keep does not lower the governor's bar, does not settle the pair as decided by people, and starts no govern round.
10. A signed-in person's Review tab requests and responses for Duplicates, Overview, keep, merge and batch are unchanged, and `remember` still lands a pending fact that waits for a nod.
11. ADR 0076 exists in the decision records with the ruled authority table, and its number and migration 0128 do not collide with upstream at landing.

Risks
- Windows-side builds may not reach or migrate the compose database, or per-run databases may not isolate; every later milestone's evidence depends on this.
- The precedent exclusion is medium confidence and was never exercised end to end; if a token row still steers the governor, no agent finalize is safe and M003 changes shape.
- One shared operation function for REST and MCP is medium confidence; if the MCP tool path cannot share it, both surfaces drift and every later milestone doubles.
- Finalized keeps are not rate limited until M003; exposure is bounded because a keep changes only the review row and is excluded from precedent.
- Number collision with upstream (migration 0127 is already claimed by an open upstream pull request); renumbering is expected at any upstream submission.
- Activity is readable only by a base Admin in a session, so criterion 8 cannot be observed by an editor.
- Local toolchain differs from CI (accepted, driver D007).

Open questions
- None

### M002 — token-review-reads

Goal: An agent holding a personal token lists every one of the thirteen Review sections and reads any listed item's evidence over the token API and MCP, seeing the same items a signed-in editor sees.
Depends on: [M001]
Surfaces: token-authenticated HTTP API, MCP endpoint
Status: pending
Archive: null
Integrated: null

Scope: in
- Token access, at owner role Viewer and through the handlers the Review tab already uses, to the eleven queue sections (Awaiting your nod, Duplicates, Conflicts, Unconfirmed, Low confidence, Axioms, Ontology, Alignment, Errata, Agent History, Merges), the Overview summary and the Decisions history. No new list route per section.
- Item evidence for every section: the list item's own fields plus the per-item reads, with the violation proof and derived-fact proof reads opened to tokens at Viewer beside the already open fact evidence and entity reads.
- Every list response carries the true total of the listed section, so paging reaches the last item (the Alignment badge count omits implication rules).
- Agent History read by a token returns the governor's rows and the token-agent ledger rows with their source field.
- MCP `review_list` covers all thirteen sections and `review_item` returns evidence for every item kind, both through the shared operation layer.
- Refusals: 404 when the token's `kb_ids` exclude the base, 401 for a dead token, the existing coded error for an unknown section.
- Documentation of the full read surface in the API and MCP documentation; ADR 0076 amended if a read shape differs from what it records.

Scope: out
- Every Review write beyond the keep shipped in M001: M003 and M004.
- Carrying an open token-agent proposal on a queue item: arrives with the proposal store in M003 and M004.
- A fix for an empty Low confidence section on a base where every fact is at full confidence: the charter does not scope it.
- Widening the Decisions section beyond its four action families, or opening the Activity view to tokens or to roles below Admin.
- Any change to a signed-in person's Review tab responses other than the added section total.

Success criteria
1. Over the token API, a read-scope token of a base Viewer lists each of the thirteen sections and receives the same items a signed-in editor sees in that section of the Review tab, on a base seeded with at least one item per section.
2. Over MCP, `review_list` returns the same items as the token API for each of the thirteen sections with the same token.
3. For one listed item of each section, the token API and MCP `review_item` return the sentences, facts or entities the Review tab shows for it, including the proof chain of an Axioms item.
4. A token client that pages the Alignment section by the returned total reaches the last implication-rule item.
5. A token whose `kb_ids` exclude the base receives 404 on every Review read over the token API and cannot list or read it over MCP.
6. Agent History read by a token shows the M001 keep as a ledger row naming the owner and the token, beside governor rows.
7. A signed-in person's Review tab lists every section as before.

Risks
- "Same items a signed-in editor sees" must be shown per section; some sections need seeded data that is expensive to build (errata, alignment rules, derived contradictions).
- The Low confidence section may list nothing on the owner's base; the criterion is met on seeded data only.
- Published images predate token reads, so the read surface can be demonstrated only on a build of this checkout.
- Adding a total to list responses touches the shape the web app reads; a mistake breaks the session tab.

Open questions
- None

### M003 — gated-merge-proposals-revert-batch

Goal: An agent merges, reverts and batches duplicate pairs behind the server gate, a held or propose-only call waits as a proposal that only a signed-in person can answer, and every agent merge can be reverted by the agent or by a person.
Depends on: [M001, M002]
Surfaces: token-authenticated HTTP API, MCP endpoint, Review tab in the web app (Agent History, Duplicates and Merges sections)
Status: pending
Archive: null
Integrated: null

Scope: in
- Token merge, both the queue decision and the manual merge, through the policy function: every token merge calls the ADR 0027 merge gate first, and the gate also holds a pair a person kept or whose merge was reverted. A pass applies the same store function the person's path calls; a hold becomes a proposal carrying the gate's reason, never an error and never a merge.
- The proposal store: a held or propose-only call waits as a ledger row holding the full request and changing no fact, entity, review row, retrieval or reasoning. One open proposal per base, operation and target; a repeat returns the existing proposal with `already_proposed`.
- The proposal answer, session only: a person approves or declines in the Agent History section; approval runs the existing session store function with the person as actor and names the proposal in the audit detail; a target that can no longer be decided closes the proposal as `superseded`. A person may answer many proposals in one request, capped at 500. No token may answer a proposal.
- The open proposal shown on the item's card in the Duplicates and Merges sections and carried on those items in token and MCP reads.
- Token revert of a merge: finalized when the merge was made by a token agent and has not been reverted; a revert of a person's or the governor's merge is a proposal, so a token cannot trip the governance fuse. A person reverts an agent merge in the Merges section as today.
- Token batch on duplicate pairs: at most 50 ids, each through the single-item token path with per-item outcomes, a server-issued batch id on every ledger and audit row, and a unit revert of a batch that reverts its not yet reverted merges best-effort with per-item outcomes.
- Per-token and per-base rate limit on finalized writes computed from the ledger, answering `agent_rate_limited`, applied to every finalized token write including keep.
- Per-token breaker: two reverts by a person of one token's finalized merges on a base within seven days make that token propose-only on that base for the rest of the window.
- MCP write tools for the merge, revert and batch families through the shared operation layer, with the REST codes in tool errors.
- Ledger later states (`approved`, `declined`, `superseded`, `undone`) and attribution of merges, reverts, batches and proposals in Agent History and Activity.
- ADR 0076 and the API and MCP documentation extended for these routes and tools, stating that a revert does not recall derivations, violations, exports or answers drawn while the merge was live.

Scope: out
- Proposals for operations other than merge and revert: M004.
- Batch for anything other than duplicate pairs; an idempotency key.
- Token answers to the governor's own decisions: the governor's decision route stays session-only; see M004.
- The session caller's ungated merge, its 500-id batch and its revert: unchanged. The governance fuse keeps counting a person's reverts as today.
- A configurable per-base table, cap or limit.
- The nod gate on `remember`: unchanged.

Success criteria
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

Risks
- A merge is the one finalized agent write that changes facts; what a wrong merge sent out of the graph is not recalled by revert, and criterion 5 is verified on graph state only.
- The gate was written for the product's own deciders; pointing it at token calls, and adding the kept-pair and reverted-merge holds, may hold far more or far fewer pairs than expected.
- Races between a person, the governor and a token on one pair have partial guards today; a lost race must map to `already_decided`, not a generic error.
- A person bulk-approving proposals can reproduce blind-acceptance errors; approved proposals count as human precedent as accepted governor proposals do today.
- Batch audit rows can go missing silently on the existing path; the token path must treat a failed snapshot or audit write as a failed item.
- The cap of 50 and the limit of 200 per hour are not measured on this product; a running batch is not stopped by token revocation, which the cap bounds.
- This is the largest milestone; if the gate, the proposal store and the batch cannot ship together it may be re-sliced at the M002 boundary with batch and breaker split out.

Open questions
- None

### M004 — remaining-review-writes

Goal: An agent can call every remaining Review operation of the ruled authority table over the token API and MCP, each landing as a finalized change or as a proposal exactly as ruled, completing the program with its full documentation.
Depends on: [M001, M002, M003]
Surfaces: token-authenticated HTTP API, MCP endpoint, Review tab in the web app (Agent History and the item cards of each section)
Status: pending
Archive: null
Integrated: null

Scope: in
- Propose-only operations, each landing in the proposal store and applied only when a signed-in person approves: confirm fact, reject fact, close fact; resolve conflict (close old, retime new, keep both, reject new); alignment phrase decision and alignment rule decision; errata decision (approve or reject a held action); ontology proposal decision (reject or adopt); violation decision; defect decision; a proposed answer to a pending fact; a proposed answer to a governor decision, subject to the open question below.
- Finalized operations through the policy function: mapping decision, and the consistency check or inference run, both counted against the per-token rate limit.
- The alignment kind-word decision keeps finalizing for write tokens exactly as today; it gains a ledger row, attribution and an MCP tool, and its existing route behaviour is not narrowed.
- One MCP write tool per operation family for all of the above, through the shared operation layer, listed and callable only with scope `write` and an owner at least Editor, with REST codes in tool errors.
- The open proposal carried on the item in each section's token and MCP reads and shown on the item's card in the Review tab; approval, decline, supersession and the session bulk answer work for every proposal kind.
- Stable codes on every write: the scope and role refusals, `already_decided`, `already_proposed`, `agent_rate_limited`.
- Ledger rows and audit rows with owner and token for every operation, visible in Agent History and Activity with readable labels for token rows.
- Program close: ADR 0076 final against what shipped, and complete API and MCP documentation for every opened route and tool, including which operations finalize, which propose, and what a person must do to answer a proposal.

Scope: out
- A token finalizing any `pending_facts` decision, or any change to how `remember` waits for a nod: the nod stays with people (charter veto).
- A token approving or declining any proposal, its own or another agent's.
- New undo routes for confirm, reject, close, conflict, errata, ontology, violation or defect decisions: not in charter scope; these stay propose-only for that reason.
- Batch routes for sections other than Duplicates.
- Widening the Decisions section's four action families: accepted by the synthesis; Agent History and Activity carry the rest.
- Migrating `pending_facts` or the errata store into the ledger.
- Any change to a signed-in person's existing decisions in these sections.

Success criteria
1. Over the token API and over MCP, a write-scope token owned by an editor calls each propose-only operation on a seeded item: no fact, entity, review row, retrieval result or reasoning output changes, and one proposal appears for that target.
2. In the Review tab, a signed-in editor sees each such proposal in the Agent History section and on the item's card in its own section, approves it, and the graph changes exactly as the same decision made by that person would change it; a declined proposal changes nothing.
3. Over the token API and over MCP, a mapping decision and a consistency check run by a write token take effect at once, each with a ledger row of outcome `applied`.
4. A write token's kind-word decision behaves as it did before the program and now appears in Agent History and Activity as "owner via token name".
5. A pending fact proposed through `remember` is still asserted or rejected only by a signed-in person; a token call that tries to answer it never changes the pending row.
6. For every operation in the ruled authority table, a call with a write-scope token owned by an editor yields the outcome the table states, and the same call with a read-scope token or a below-editor owner is refused with a stable code, over the token API and over MCP.
7. Every action taken in criteria 1 to 4 appears in Agent History and in Activity naming the token owner and the token.
8. The API and MCP documentation list every opened Review route and tool with its ruling, and ADR 0076 matches the shipped behaviour.
9. All eight program success criteria of the charter hold on a build of this checkout, including an unchanged Review tab for a signed-in person.

Risks
- Twelve operation families in one milestone; they are uniform (propose, then a person applies the existing session function) but touch many store paths, and the milestone may need re-slicing at the M003 boundary.
- Approval fatigue: outside Duplicates there is no bulk path today, so the session bulk answer is the only relief on a large base, and it can reproduce blind acceptance.
- A proposal can go stale while it waits (a fact rejected, a conflict resolved, an errata action answered); each kind needs a correct `superseded` rule.
- Alignment phrase and rule approvals start asynchronous delivery jobs; a proposal's approval must surface that the change is queued, not applied.
- Bulk approval of proposed answers to pending facts lets a person nod many remembered sentences in one action, which is closer to the nod gate than any other proposal kind.
- The consistency check row has the thinnest evidence in the table; it is expensive to run and is bounded only by the rate limit.
- The ontology adoption reason rests on the M001 confirmation; a correction changes the ADR text, not the ruling.

Open questions
- The ruled table makes "answer a governor decision" propose only for a token, while the synthesis also keeps the governor's own decision route session-only and says existing handlers change extractor in place. Through which token route and MCP tool does an agent propose an answer to a governor decision, or is that proposal reachable only as an ordinary merge or keep proposal on the same pair?
