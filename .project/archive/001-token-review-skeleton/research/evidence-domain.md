# Evidence — domain

<!-- Written by one researcher role. Consumed by the decider. -->

Dimension: domain
Questions assigned: Which operations may an agent finalize and which may it only propose? Gather the precedents (ADR 0015 nod, ADR 0025 governor, ADR 0027 merge gate, ADR 0043 governed queues, ADR 0044 errata gate) and each operation's reversibility, including what a wrong action sends out of the graph. Decide proposes a per-operation table; the owner rules on it (Delegated, driver D003). This ruling is a `NEEDS-USER` item at decide.; Largest assumption, challenged: that an agent reading untrusted base content can be allowed to finalize anything safely. If it cannot, the program still delivers reads plus proposals for every operation; research should say what that leaves a person to click on a large base.

Scope note: all repository references are to the checkout at `3d307f6` under `D:\utopia`. Code was read, not run. Web sources were opened on 2026-11-18.

## Finding: The product's stated rule is "act alone only on what can be undone inside the graph; hold what leaves it"

- **Claim**: ADR 0027 gates an automatic merge on three things it would send out of the graph (a functional-predicate contradiction, a live derivation on either side, either side named in an assistant answer) and holds the pair for a person "whatever the confidence"; it states that `revert_merge` "restores the graph, not the world the graph was read into", and that keeps are not gated because "the next merge is the undo".
- **Source**: `docs/decisions/0027-an-automatic-merge-is-gated-by-what-it-can-undo.md:11-27`; implemented in `crates/utopia-store/src/execution_gate.rs:108-119` (`hold`), `:123-151` (`hold_for_merge`, which adds an identity-source hold), `:157-193` (`impact_of`)
- **Confidence**: high
- **Why it matters here**: This is the product's own criterion for finalize versus propose; the charter's per-operation table can be read against it operation by operation.

## Finding: ADR 0027 names an agent's calls as this gate's next caller

- **Claim**: ADR 0027 decision 3 says the roadmap's execution gate, "checking an agent's calls against ontology rules and symbolic logic", "is this gate pointed at someone else's writes rather than our own, and joins this module rather than growing beside it"; the module header repeats it.
- **Source**: `docs/decisions/0027-an-automatic-merge-is-gated-by-what-it-can-undo.md:29-31`; `crates/utopia-store/src/execution_gate.rs:13-14`
- **Confidence**: high
- **Why it matters here**: The product already recorded where a server-enforced check on external agent writes belongs; the charter's "enforced by the server" policy has a designated home.

## Finding: The person's merge path is not gated; only the two machine deciders call the gate

- **Claim**: `hold_for_merge` is called from exactly two places, the batch adjudicator and the governor. The session routes `decide`, `batch`, `agent_answer` and `manual_merge` call `decide_review` / `merge_entities` directly with no impact check.
- **Source**: grep `hold_for_merge\(` over `crates/` → `crates/utopia-server/src/adjudication.rs:396`, `crates/utopia-server/src/governance.rs:575` only; `crates/utopia-server/src/api/review_routes.rs:283-339`, `:486-558`, `:656-702`; `crates/utopia-store/src/resolution.rs:1991-2064`
- **Confidence**: high
- **Why it matters here**: Opening the existing routes to a token as they stand would give an external agent the person's ungated merge, which is more than the product's own agent has.

## Finding: An action taken with a token is recorded as the person's and would be read as human precedent

- **Claim**: Every Review write records `audit_events.actor_id = user.id`. The governor's precedent queries select `review.merge`, `review.keep` and `merge.manual` rows `WHERE actor_id IS NOT NULL` and do not read `detail.token_id`. ADR 0025 decision 1 says only people write precedent because the agent's own rows "would cite itself and grow more confident with every round"; agreeing precedent lowers the governor's bar from 0.85 to 0.75, and a pair "people already decided" is applied without the model.
- **Source**: `crates/utopia-store/src/governance.rs:89-108`, `:812-818`; `crates/utopia-store/src/audit.rs:32-36` (`via_token` only adds `detail.token_id`); `docs/decisions/0025-governance-reads-the-ledger-before-it-decides.md:19`, `:46`; `docs/decisions/0014-identity-from-the-person-scope-from-the-token.md:18-22`
- **Confidence**: high
- **Why it matters here**: A finalized agent merge or keep made under a token would, with today's queries, steer later automatic merges by the built-in governor; the charter's attribution criterion (owner plus agent name) and ADR 0025 decision 1 pull in different directions unless token rows are told apart.

## Finding: A token-made revert would count toward the governance fuse

- **Claim**: `revert_merge` (route) calls `governance::after_revert`, which marks the agent's row `reverted` and runs `fuse`; two reverts of the built-in agent's merges within seven days turn governance off for the base. The count does not distinguish who reverted.
- **Source**: `crates/utopia-server/src/api/review_routes.rs:608-644`; `crates/utopia-server/src/governance.rs:911-976`; `docs/decisions/0025-governance-reads-the-ledger-before-it-decides.md:68-72`
- **Confidence**: high
- **Why it matters here**: Revert is the one operation the charter requires an agent to perform (success criterion 7); two agent reverts of governor merges would switch off a base's governance as if a person had judged the governor wrong.

## Finding: Nods are excluded from every agent by two decision records

- **Claim**: ADR 0015 decision 6 says with the gate "an external agent can only propose, never assert"; ADR 0025 open questions and ADR 0043 decision 1 both keep `pending_facts` out of the governor: "an agent nodding for them would turn a remark into an assertion".
- **Source**: `docs/decisions/0015-recording-a-sentence-is-not-asserting-a-fact.md:53-55`; `docs/decisions/0025-governance-reads-the-ledger-before-it-decides.md:114`; `docs/decisions/0043-every-review-queue-is-governed.md:20`; `docs/design/governance.md:98-99`
- **Confidence**: high
- **Why it matters here**: "Decide pending nod" by an agent is the operation the charter's second veto names; the product's records already rule it out for the product's own agent.

## Finding: Confirming a nod writes a live fact and removes the pending row; rejecting it writes a permanent block with no removal path

- **Claim**: `pending::confirm` inserts the fact through the extraction path with evidence and temporal reconciliation, then deletes the pending row. `pending::reject` inserts into `rejected_facts` and deletes the pending row; `rejected_facts` is only inserted into and read (to mark a later extraction as previously rejected), and no code deletes from it.
- **Source**: `crates/utopia-store/src/pending.rs:289-349`, `:563-590`, `:142-148`; grep `rejected_facts` over `crates/` (non-test) → 6 hits, one `INSERT`, one `SELECT`, no `DELETE`
- **Confidence**: high
- **Why it matters here**: Both nod answers change state that no route undoes directly (a wrong confirm needs a separate fact reject; a wrong reject has no undo).

## Finding: Merge is the only Review write with an exact, routed undo

- **Claim**: `merge_entities` logs moved facts, invalidated duplicates, copied evidence and the target's prior profile and type in `entity_merges`; `revert_merge` moves facts back (including rows rewritten after the merge), un-invalidates the duplicates, deletes copied evidence, restores the target snapshot, revives the source, and refuses a second revert with a Conflict. It is exposed at `POST /kbs/{id}/merges/{merge_id}/revert`.
- **Source**: `crates/utopia-store/src/resolution.rs:2139-2179`, `:2487-2629`; `crates/utopia-server/src/api/mod.rs:606-607`; `docs/decisions/0019-the-second-clock-can-be-rewound.md:37`
- **Confidence**: high
- **Why it matters here**: Charter success criterion 7 (every agent merge revertible, graph returns to its prior state) is supported by existing store code for the graph itself.

## Finding: What a wrong merge sends out of the graph is not recalled by revert

- **Claim**: While a wrong merge is live it can open a `functional` violation at the next consistency check, cause derived facts with either side as subject or object to be rewritten, and enter chat answers that name the entity; ADR 0027 states none of these is recalled by `revert_merge`. Exports are named as a fourth exit and deliberately not gated ("an export is of the base, not of the pair").
- **Source**: `docs/decisions/0027-an-automatic-merge-is-gated-by-what-it-can-undo.md:7`, `:11`, `:19-21`, `:51`; `crates/utopia-store/src/execution_gate.rs:4-6`
- **Confidence**: high
- **Why it matters here**: "Revertible" in the charter covers the graph only; the owner's ruling on merge needs the distinction between graph state and what was read from it.

## Finding: Keep (duplicates) changes only the review row

- **Claim**: `decide_review` with `keep` updates `resolution_reviews` to `kept` with `decided_by` and `rationale` and touches no fact or entity; a later manual merge of the same two entities remains possible.
- **Source**: `crates/utopia-store/src/resolution.rs:2049-2060`; `docs/decisions/0027-an-automatic-merge-is-gated-by-what-it-can-undo.md:25-27`; `docs/decisions/0025-governance-reads-the-ledger-before-it-decides.md:50`
- **Confidence**: high
- **Why it matters here**: Keep is the product's own example of an action cheap enough to leave ungated; its remaining cost is the precedent it writes (see the precedent finding).

## Finding: Confirm fact overwrites confidence with 1.0 and has no undo route

- **Claim**: `graph::confirm_fact` runs `UPDATE facts SET confidence = 1.0`; the prior value survives only in the `fact.confirm` audit snapshot taken before the write. No route or store function restores it. The "unconfirmed" queue criterion does not read confidence, so this write does not remove a stale-evidence fact from that queue.
- **Source**: `crates/utopia-store/src/graph.rs:2147-2162`; `crates/utopia-server/src/api/review_routes.rs:28-51`, `:560-582`; `crates/utopia-store/src/review.rs:49-60`, `:78-80`
- **Confidence**: high
- **Why it matters here**: A confirmed fact stays live either way, so nothing new leaves the graph, but a wrong confirm removes the item from the Low confidence queue and cannot be put back through the API.

## Finding: Reject fact invalidates the row and recomputes timelines; no route restores it

- **Claim**: `graph::reject_fact` calls `temporal::retract` (sets `invalidated_at`, recomputes the fact's timelines so a predecessor closed at its start reopens) and, for an open statement, queues a typed-graph recompute. The row is kept (append-only), but the only code that clears `invalidated_at` is merge revert and document restore; the `temporal::restore` and `temporal::undo_rewrite` functions that ADR 0043 decision 4 lists as undos do not exist in this checkout.
- **Source**: `crates/utopia-store/src/graph.rs:2164-2196`; `crates/utopia-store/src/temporal.rs:852-877`; grep `invalidated_at = NULL` over `crates/` (non-test) → `documents.rs:1409`, `resolution.rs:2597` only; grep `pub (async )?fn (restore|undo|reopen)` → no fact-level match; `docs/decisions/0043-every-review-queue-is-governed.md:37-48`, `:76`
- **Confidence**: high
- **Why it matters here**: A wrong reject removes a true fact from retrieval, reasoning and export, and may reopen a predecessor; undoing it needs code the program would have to add (or re-land from closed PR #699).

## Finding: Close fact and conflict resolutions rewrite rows inside one transaction, with no routed undo

- **Claim**: `close_superseded` invalidates the row and inserts a corrected row with a stated end "the engine no longer recomputes". `resolve_review_conflict` handles `close`, `retime_new`, `keep`, `reject_new` in one transaction with the audit row, re-checking that the conflict is still open on the locked pair; `reject_new` retracts the new fact and settles its sibling conflicts. A resolved conflict is not reopened by any function in this checkout.
- **Source**: `crates/utopia-store/src/temporal.rs:879-915`; `crates/utopia-store/src/temporal/conflicts.rs:49-196`; `crates/utopia-server/src/api/review_routes.rs:186-272`
- **Confidence**: medium
- **Why it matters here**: These are the actions that decide which value the graph answers with at a date; ADR 0043 planned undos for each, and none is on `dev`.
- **Limits**: High for what the code does; medium for "no routed undo", because the graph page's interval editor was not read and may correct a closed interval by a further rewrite. The finding is kept at the lower level.

## Finding: ADR 0043's conflict and fact governance is accepted but not built; automatic application is explicitly waiting on a measurement

- **Claim**: ADR 0043 is "Accepted 2026-09-14 · no code on `dev` (#699 closed)". Its 2026-10-20 note says the human conflict path landed, "The governor still handles only duplicates. Proposals follow in a separate PR; automatic conflict application and undo require the real-model lease measurement requested in #1131's review." Its decision 3 also requires server-checked evidence (a close date must appear in the evidence; a stale fact is confirmed only by a quote found in the current document version) and decision 4 requires a recorded undo per action.
- **Source**: `docs/decisions/0043-every-review-queue-is-governed.md:3`, `:28-50`, `:76-93`; `docs/design/governance.md:103-110`
- **Confidence**: high
- **Why it matters here**: For confirm, reject and resolve, the product's own position today is: its agent may not yet apply them automatically, and when it does, each needs a server-side evidence check and a stored undo.

## Finding: The errata gate holds retractions and revisions of unflagged facts for a person because the agent removed correct facts

- **Claim**: The errata agent's action is `held` when the 0027 gate fires (a derivation rests on the fact, its subject was named in an answer, or the write would give a functional predicate two values) or when it wants to retract or revise a fact no structural flag reported; ADR 0044 measured that about a quarter of what the agent retracted was correct, and the code comment says nine in ten retractions in the first real run were of facts the text did state. `decide_held` executes the held action on approval; no undo of an applied errata action was found.
- **Source**: `crates/utopia-store/src/errata.rs:1-12`, `:516-550`, `:779-834`; `docs/decisions/0044-the-ontology-is-a-view-over-what-documents-say.md:38`, `:104-106`
- **Confidence**: medium
- **Why it matters here**: Approving an errata item is itself the human check on another agent's output; an external agent approving it would replace the person the gate was built to reach.
- **Limits**: High for the gate and what it holds; medium for "no undo of an applied errata action", because only `errata.rs` was searched. The finding is kept at the lower level.

## Finding: Violation and defect decisions are one-way status changes, and two of them have wider effects

- **Claim**: `reasoning::decide` and `decide_defect` set `status = 'resolved'` only `WHERE status = 'open'`, so a decided row cannot be decided again. `fact_retracted` really retracts a fact and reruns the consistency check; `fact_closed` rewrites an interval; for a `derived_contradiction`, any resolution (including `accepted`) is followed by `materialize`, which lets the blocked derivation land. ADR 0043 says `axiom_relaxed` "changes the ontology" and `fixed` "means a person changed it; both need a gate before an agent applies them".
- **Source**: `crates/utopia-store/src/reasoning.rs:943-999`, `:2786-2812`; `crates/utopia-server/src/api/review_routes.rs:807-948`, `:1013-1041`; `docs/decisions/0043-every-review-queue-is-governed.md:68`
- **Confidence**: high
- **Why it matters here**: A wrong `accepted` on a contradiction puts a derived fact into the graph and silences the queue item permanently; these rows have no reopen path to build a revert on.

## Finding: Alignment decisions act per signature, not per fact, and a person's decision cannot be overridden by the product's aligner

- **Claim**: A phrase decision is written as `decided_by = 'person'` and commits with a `materialize_typed` job that recomputes typed facts for every statement under that signature; `phrase_bindings::decide` refuses an agent or ontology write over a person's row. A rule decision sets `decided_by = 'person'` and queues reading or materialisation of implied facts. Both can be decided again by a later person call (the rule `UPDATE` has no status guard; the phrase write is an upsert).
- **Source**: `crates/utopia-server/src/api/review_routes.rs:1214-1356`; `crates/utopia-store/src/phrase_bindings.rs:319-336`, `:379-424`; `crates/utopia-store/src/implication_rules.rs:165-206`; `docs/decisions/0044-the-ontology-is-a-view-over-what-documents-say.md:60-69`
- **Confidence**: high
- **Why it matters here**: One alignment call changes many typed facts at once and, recorded as a person's, becomes immune to the aligner's later correction; it is re-decidable, but typed facts read in between are not recalled.

## Finding: One alignment write already takes a write token and finalizes

- **Claim**: `decide_alignment_kind_word` takes `SessionOrToken`, requires write scope and Editor, writes the binding as a human decision, applies the class to the entities under that kind word, and records `alignment.kind_word_decided` with `detail.token_id`. ADR 0014 decision 6 opened it for a backend that sets up a base it mirrors into, and adds that routes behind `AuthUser` "stay session-only until someone has a reason to open one and says it here".
- **Source**: `crates/utopia-server/src/api/review_routes.rs:1365-1426`; `docs/decisions/0014-identity-from-the-person-scope-from-the-token.md:57-86`
- **Confidence**: high
- **Why it matters here**: There is an existing precedent of a token finalizing a Review decision, justified by a trusted backend's setup job, not by an agent reading base content; the per-operation table has to place this already-open route.

## Finding: Ontology proposals split into a reject route and an adopt route; no undo of an adoption was found

- **Claim**: `POST /kbs/{id}/ontology/proposals` only accepts `rejected` (status change, row kept, reason readable by the next agent round); adoption goes through `/ontology/proposals/adopt` → `ontology_agent::adopt`, which creates ontology elements. A case-insensitive search of `ontology_agent.rs` for undo/revert found nothing.
- **Source**: `crates/utopia-server/src/api/ontology_routes.rs:601-643`; `crates/utopia-server/src/api/question_routes.rs:254-266`; grep `undo|revert|unadopt` in `crates/utopia-server/src/ontology_agent.rs` → no matches
- **Confidence**: low
- **Why it matters here**: Adoption changes the contract every later alignment and consistency check reads; ADR 0043 already treats ontology-changing actions as needing a gate before an agent applies them.
- **Limits**: High for the two routes and what each accepts; low for "adoption has no undo", because the store's ontology module was not read. The finding is kept at the lower level.

## Finding: Mapping decisions are re-decidable and feed the chat prompt

- **Claim**: `mappings::decide` updates `status` with no guard on the prior status, so a confirmed mapping can be set to rejected and back; confirmed mappings reach the chat prompt through retrieval.
- **Source**: `crates/utopia-store/src/mappings.rs:175-201`; `crates/utopia-server/src/api/review_routes.rs:754-781`; `docs/design/chat-and-mcp.md:72-73`
- **Confidence**: high
- **Why it matters here**: A wrong confirm is undone inside the product by one more call, but answers given while it was confirmed are out of the graph.

## Finding: The batch route applies one action to up to 500 duplicate pairs, each through the single-item path

- **Claim**: `batch` accepts 1–500 review ids and one action (`merge` or `keep`), calls `decide_review` per id with independent outcomes, writes one audit row per decided pair with a `batch` field holding the batch size, and triggers one governor round. It covers `resolution_reviews` only; no batch route exists for facts, conflicts, nods, errata, alignment, violations or defects.
- **Source**: `crates/utopia-server/src/api/review_routes.rs:471-558`; `crates/utopia-store/src/resolution.rs:1906-1929`; `crates/utopia-server/src/api/mod.rs:532-615`
- **Confidence**: high
- **Why it matters here**: Batch multiplies whatever the merge ruling is by up to 500 per call, each merge separately revertible; it is also the only existing bulk path a person has for clearing proposals.

## Finding: The product's own measurements show an agent merging on its own is wrong at a low, non-zero rate, and blind acceptance propagates

- **Claim**: On 589 labelled pairs the shipped governor decided 97.7% on its own, agreed with labels on 96.7%, made 6 wrong merges and left 12 for people; ADR 0025 notes two of four wrong merges in one run "follow a precedent a simulated person left by accepting proposals blindly", and states "a wrong merge costs more than a wrong keep".
- **Source**: `docs/decisions/0025-governance-reads-the-ledger-before-it-decides.md:23-29`, `:82-93`; `docs/design/governance.md:75-77`
- **Confidence**: high
- **Why it matters here**: It bounds both options: a gated agent finalizing merges leaves about 2% of pairs to people on this bench, and a person approving proposals without reading is a measured source of wrong merges.
- **Limits**: The figures are the product's own bench: one corpus and one model family.

## Finding: The product already names the external agent as a confused deputy and answered it with a propose-only write

- **Claim**: ADR 0014 decision 2 describes an MCP client as "someone else's agent under someone else's system prompt, reading untrusted content from the base" that may obey "a document saying 'run this SQL' or 'remember X'" with the person's permissions; ADR 0015 decision 6 removes the objection for `remember` because the agent "can only propose". In code the only MCP write tool is `remember`, exposed when the token has write scope and the owner is Editor or above.
- **Source**: `docs/decisions/0014-identity-from-the-person-scope-from-the-token.md:24-31`; `docs/decisions/0015-recording-a-sentence-is-not-asserting-a-fact.md:53-55`; `crates/utopia-server/src/api/mcp.rs:63-68`, `:107-111`; `docs/design/chat-and-mcp.md:113-116`
- **Confidence**: high
- **Why it matters here**: The program's threat model is not new to the product; the only precedent for an external agent's write resolves it by proposal, which is what the charter's second veto protects.

## Finding: OWASP's LLM guidance calls for human approval of high-impact actions and server-side authorization, and says prevention of prompt injection is not assured

- **Claim**: OWASP LLM06:2025 (Excessive Agency) lists "Utilise human-in-the-loop control to require a human to approve high-impact actions before they are taken", "Implement authorization in downstream systems rather than relying on an LLM to decide if an action is allowed or not", minimising extension functionality and permissions, executing in the user's context, and, as damage limiters only, logging and rate limiting. OWASP LLM01:2025 says "it is unclear if there are fool-proof methods of prevention for prompt injection" and recommends "human-in-the-loop controls for privileged operations".
- **Source**: https://genai.owasp.org/llmrisk/llm062025-excessive-agency/ ; https://genai.owasp.org/llmrisk/llm01-prompt-injection/
- **Confidence**: high
- **Why it matters here**: It supports gating by impact rather than a blanket rule, and puts enforcement in the Utopia server, as the charter's policy item requires.
- **Limits**: Primary source; the quotes were returned by a fetch-and-extract tool, not read as raw HTML.

## Finding: The MCP specification puts the human-in-the-loop duty on the client, which the server cannot verify

- **Claim**: The MCP tools specification (2025-06-18) says "there SHOULD always be a human in the loop with the ability to deny tool invocations" and that applications SHOULD "present confirmation prompts to the user for operations"; these are SHOULDs addressed to clients. Server MUSTs are: validate tool inputs, implement access controls, rate limit tool invocations, sanitize tool outputs. Clients MUST treat tool annotations as untrusted unless from trusted servers.
- **Source**: https://modelcontextprotocol.io/specification/2025-06-18/server/tools
- **Confidence**: high
- **Why it matters here**: Utopia cannot assume the connecting client asks its user before a Review write (headless agents will not), so any person-in-the-loop guarantee has to be Utopia's own queue; rate limiting tool calls is a server MUST the Review tools would inherit.

## Finding: MCP's own "confused deputy" guidance is about OAuth proxying, not about content read through tools

- **Claim**: The MCP Security Best Practices page defines its confused-deputy attack as an MCP proxy server with a static client ID skipping third-party consent, and covers token passthrough, SSRF, session hijacking, local server compromise, authorization URL validation and scope minimization. It contains no section on prompt injection through tool results or resource content. Its scope-minimization section recommends "minimal initial scope" and warns against "wildcard or omnibus scopes".
- **Source**: https://modelcontextprotocol.io/specification/2025-06-18/basic/security_best_practices (the fetched page links to the 2025-11-25 authorization spec, so it is the current text)
- **Confidence**: high
- **Why it matters here**: The charter's risk (an agent talked into a write by base content) is not addressed by MCP's guidance under that name; the omnibus-scope warning bears on a single `write` scope covering every Review operation.

## Finding: Published agent-security guidance says an agent that reads untrusted input and can change state should not act autonomously

- **Claim**: Meta's "Agents Rule of Two" (2025-10-31) says an agent should satisfy no more than two of: processes untrustworthy inputs, has access to sensitive systems or private data, can change state or communicate externally; with all three it "should not be permitted to operate autonomously and at a minimum requires supervision — via human-in-the-loop approval or another reliable means of validation". "Design Patterns for Securing LLM Agents against Prompt Injections" (arXiv 2506.08837) states that "once an LLM agent has ingested untrusted input, it must be constrained" so that input cannot trigger consequential actions.
- **Source**: https://ai.meta.com/blog/practical-ai-agent-security/ ; https://arxiv.org/html/2506.08837
- **Confidence**: medium
- **Why it matters here**: A Review agent reads extracted base content, holds the owner's access and writes to the graph, so it has all three properties; both sources allow "another reliable means of validation", which is the role ADR 0027's and ADR 0043's server checks play.
- **Limits**: A vendor engineering post and a preprint; both are the authors' own text, and the quotes were returned by a fetch-and-extract tool.

## Finding: The same sources warn that human approval degrades at volume

- **Claim**: The design-patterns paper treats user confirmation as effective in principle but warns of "tired or overloaded human verifiers approving unsafe actions" and that it slows automation.
- **Source**: https://arxiv.org/html/2506.08837 (first 100,000 of 108,649 characters read)
- **Confidence**: medium
- **Why it matters here**: Propose-only for every operation on a large base turns each agent action into a human click; the product's own bench shows blind acceptance producing wrong merges (ADR 0025 decision 10).

## Finding: Per-operation reversibility at 3d307f6

- **Claim**: At `3d307f6` only merge has an exact routed undo; keep, revert merge, mapping decisions and alignment decisions are undone by a further decision; confirm fact, reject fact, close fact, resolve conflict, nod answers, errata decisions, ontology proposal decisions, violation decisions and defect decisions have no routed undo; the table under this finding gives, per operation, what it changes, the undo that exists today, and what a wrong action sends out of the graph.
- **Source**: `crates/utopia-server/src/api/review_routes.rs:186-1460`; `crates/utopia-store/src/resolution.rs:1991-2064`, `:2139-2179`, `:2487-2629`; `crates/utopia-store/src/graph.rs:2147-2196`; `crates/utopia-store/src/temporal.rs:852-915`; `crates/utopia-store/src/temporal/conflicts.rs:49-196`; `crates/utopia-store/src/pending.rs:289-349`, `:563-590`; `crates/utopia-store/src/reasoning.rs:943-999`, `:2786-2812`; `crates/utopia-store/src/errata.rs:516-550`, `:779-834`; `crates/utopia-store/src/mappings.rs:175-201`; `crates/utopia-store/src/phrase_bindings.rs:319-424`; `crates/utopia-store/src/implication_rules.rs:165-206`; `crates/utopia-server/src/api/ontology_routes.rs:601-643`; `docs/decisions/0027-an-automatic-merge-is-gated-by-what-it-can-undo.md:11-27`
- **Confidence**: medium
- **Why it matters here**: This is the reversibility column the charter asks research to supply for the owner's per-operation finalize-versus-propose ruling.
- **Limits**: Each row summarises a finding above and carries that finding's confidence: high for most rows; medium for "no undo" on close fact, resolve conflict and errata; low for "no undo" on ontology adoption. The finding is kept at medium. Two cross-cutting facts bear on every row: the person's routes carry no impact gate (only the machine deciders call it), and token actions are stored with the person as actor, so the governor would read them as human precedent.

| Operation | What it changes | Undo that exists today | What a wrong action sends out |
|---|---|---|---|
| Merge (decide, agent answer, manual, batch) | Moves facts to the target, invalidates duplicates, marks source merged | Exact: `revert_merge`, routed | Functional violation, rewritten derivations, answers naming the entity, exports (ADR 0027); human-actor precedent |
| Keep | Review row only | A later merge | Nothing from the graph; human-actor precedent |
| Revert merge | Restores the pre-merge graph | Merge again | Counts toward the governance fuse; a `merge.revert` precedent blocks the governor on either name |
| Batch | Up to 500 merges or keeps, each as above | Per merge | As merge, multiplied |
| Confirm fact | `confidence = 1.0` | None (prior value only in the audit snapshot) | Nothing new leaves; item drops out of Low confidence |
| Reject fact | Invalidates the fact, recomputes timelines, typed recompute | None routed (row kept) | A true fact disappears from retrieval, reasoning, export; predecessor may reopen |
| Close fact | Rewrites the row with a fixed end | None routed | Answers at a date change |
| Resolve conflict | Close old, retime new, keep both, or reject new; conflict resolved | None routed; ADR 0043 undos not on `dev` | Which value answers at a date; `reject_new` as reject fact |
| Decide pending nod | Confirm: live fact via extraction path. Reject: `rejected_facts` row | Confirm: reject the fact. Reject: none | Confirm: an unreviewed remark becomes an assertion (ADR 0015) |
| Decide review item / agent proposal | Merge or keep as above, and settles the proposal as accepted or overridden | As merge / keep | As merge / keep; recorded as a person's answer to the governor |
| Alignment phrase / rule | Binding or rule per signature as `person`; typed or implied facts recomputed | Decide again | Many typed facts at once; aligner can no longer correct it |
| Alignment kind word | Class binding applied to entities (already token-enabled) | Decide again | Entity classes, hence phrase signatures |
| Errata decision | Approve executes retract/revise/add; reject records | None found | As reject fact or a new typed fact |
| Ontology proposal | Reject: status. Adopt (separate route): ontology elements | Reject: none found. Adopt: none found (low confidence) | Adopt changes the contract alignment and checks read |
| Violation decision | One-way resolve; may retract or close a fact; `accepted` on a derived contradiction lets the derivation land | None | Derived fact enters the graph; queue item gone for good |
| Defect decision | One-way resolve (`fixed` / `accepted`) | None | Nothing from the graph; defect no longer shown |
| Mapping decision | Status confirmed / rejected | Decide again | Confirmed mapping enters chat prompts |

## Assigned questions — answers

- Which operations may an agent finalize and which may it only propose? Gather the precedents (ADR 0015 nod, ADR 0025 governor, ADR 0027 merge gate, ADR 0043 governed queues, ADR 0044 errata gate) and each operation's reversibility, including what a wrong action sends out of the graph. Decide proposes a per-operation table; the owner rules on it (Delegated, driver D003). This ruling is a `NEEDS-USER` item at decide. → Evidence only; no ruling made here. Precedents: nods stay with people for every agent (ADR 0015 d6, 0025 open questions, 0043 d1); the product's agent may finalize a merge only past a confidence/precedent gate and the impact gate, and a keep on confidence alone (ADR 0025 d4, 0027 d1–d2); for facts and conflicts the product's agent is designed to apply only with server-checked evidence and a stored undo, and that is not built and awaits a measurement (ADR 0043 d3–d4 and its 2026-10-20 note); errata retractions/revisions the structure did not flag, and anything the impact gate catches, are held for a person (ADR 0044 d7, `errata.rs:516-550`); ontology-changing resolutions "need a gate before an agent applies them" (ADR 0043 open questions); one alignment write (kind word) is already token-finalized for a trusted setup backend (ADR 0014 d6). Reversibility per operation, and what a wrong action sends out of the graph, is the table in the finding "Per-operation reversibility at 3d307f6"; two cross-cutting facts bear on every row and are in the findings "The person's merge path is not gated; only the two machine deciders call the gate" and "An action taken with a token is recorded as the person's and would be read as human precedent".
- Largest assumption, challenged: that an agent reading untrusted base content can be allowed to finalize anything safely. If it cannot, the program still delivers reads plus proposals for every operation; research should say what that leaves a person to click on a large base. → No source read supports "safely" without qualification: OWASP LLM01 says fool-proof prevention of prompt injection is unclear, and Meta's Rule of Two and arXiv 2506.08837 say an agent that reads untrusted input and can change state needs human approval "or another reliable means of validation" (URLs in the findings "OWASP's LLM guidance calls for human approval of high-impact actions and server-side authorization, and says prevention of prompt injection is not assured" and "Published agent-security guidance says an agent that reads untrusted input and can change state should not act autonomously"). The product's own records agree on the threat (ADR 0014 d2) and answer it two ways: proposal (ADR 0015) and server-side gates on what can be undone or checked (ADR 0027, 0043 d3–d4, 0044 d7). By the product's criterion the operations with the least exposure are keep (changes no fact), revert merge (restores a prior state) and mapping decisions (re-decidable), each with the side effects listed in the table of the finding "Per-operation reversibility at 3d307f6"; merge has an exact graph undo but known exits; every other operation has no routed undo today. If everything is propose-only, a person answers one item per agent proposal: the existing Agent queue takes one call per decision (`review_routes.rs:353-469`), the only bulk path is `review/batch` for duplicate pairs (500 per call, merge or keep), and no bulk path exists for facts, conflicts, errata, alignment, violations or defects. The product's measured reference point is duplicates only: its gated governor left 12 of 589 pairs for people, and blind acceptance of proposals produced wrong merges (ADR 0025 d10). How many items a person would face on the owner's base was not measured here; no reliable source found for that number.

## Dead ends

<!-- What was checked and didn't pan out, so nobody re-treads it. -->
- MCP Security Best Practices as a source on agent write authority under prompt injection — its confused-deputy section is about OAuth proxy consent; it has no guidance on injected content in tool results.
- Simon Willison's "lethal trifecta" (https://simonwillison.net/2025/Jun/16/the-lethal-trifecta/) — covers data exfiltration (private data, untrusted content, external communication), not unauthorized state changes; not used as a finding.
- arXiv abstract page for 2506.08837 — the fetch tool would not return the abstract verbatim; the HTML full text was used instead, and its last 8,649 characters were not read.
- `temporal::restore`, `temporal::undo_rewrite`, `temporal::set_confidence` named by ADR 0043 — not present in this checkout (PR #699 closed); no fact-level undo to build on.
- A reopen path for resolved conflicts, violations or defects — none found in `temporal/conflicts.rs` or `reasoning.rs`.
- An undo for ontology adoption — searched `crates/utopia-server/src/ontology_agent.rs` only; the store's ontology module was not read, so this stays low confidence.
- How Activity and Agent History render token actions, and what the web Review tab offers on the Unconfirmed queue — assigned to `capability-map`; not read here.
- The graph page's interval editor as an undo for close/retime — not read.
- A published count of review items per section on a large base, or of clicks a propose-only policy costs — no source found; the only measured figure is the product's duplicate bench.
