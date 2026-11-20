# 0076 · A token agent acts on Review under a ruled table

- **Status**: In progress · accepted 2026-11-18 · the keep slice built 2026-11-19 (migration 0128): a personal token reads Duplicates and Overview and keeps one duplicate pair through the policy function, over the token API and the MCP tools `review_list`, `review_item` and `review_keep`, with the `token_agent_actions` ledger, the precedent exclusion, and "owner via token name" in Agent History and Activity · the read slice built 2026-11-19 (no migration): a personal token lists all thirteen Review sections with a `total`, reads one listed item of any section and both proof routes · the gated-write slice built 2026-11-19 (migration 0129): a write token of an editor merges a duplicate pair or two entities behind the merge gate, reverts a merge a token agent made and batches up to 50 duplicate pairs with a unit revert, over the token API and the MCP tools `review_merge`, `review_revert` and `review_batch`; a held or propose-only call changes nothing and waits in the ledger as a proposal that only a signed-in editor answers; finalized token writes are limited to 200 per rolling hour and a per-token breaker makes a token propose only · the remaining Review writes built 2026-11-20 (no migration): every row of the table is now reachable by a write token of an editor under its ruling, over the token API and twenty MCP tools; the twelve propose-only rows change nothing and land as proposals that only a signed-in editor approves or declines, and an approval runs that person's own code as that person's decision; the mapping decision and the consistency check or inference run finalize under the limit; the kind-word decision keeps its route and gains a ledger row · open: a token keep is counted under its owner in Decisions and Overview (ruled "unchanged for now")
- **Written**: 2026-11-18
- **Related**: [0014](0014-identity-from-the-person-scope-from-the-token.md) (a token acts as its owner; decisions 6 and 7 opened the first token write and the token reads); [0027](0027-an-automatic-merge-is-gated-by-what-it-can-undo.md) (the execution gate; decision 3 names an agent's calls as its next caller); [0025](0025-governance-reads-the-ledger-before-it-decides.md) (the governor and its precedent); [0015](0015-recording-a-sentence-is-not-asserting-a-fact.md) (the nod); [0043](0043-every-review-queue-is-governed.md) (a gate before an agent applies changes); [0046](0046-the-app-surface-is-mcp.md) (the MCP surface)

## Problem

The Review tab is the only way to maintain a graph, and its routes take a browser
session only. A personal token reads entities, the ontology, the export and search
(0014 decision 7) and gets 401 on Review; the MCP server has eleven read tools and
`remember`, and nothing that touches review. An agent that can read the graph
cannot act on what it finds, and on a large machine-extracted base the clean-up
cannot be done one click at a time.

Opening the handlers as they stand is not the answer. The person's merge path calls
no gate: an outside agent would hold a wider merge power than the product's own
governor, up to 500 merges in one request, on the strength of text it has just read.
0015 makes `remember` wait for a person's nod because a document can talk an agent
into recording something; the same risk applies to an agent confirming or merging
facts.

So the question this record settles is not "may a token call Review" but **which
Review operations a token agent may finalize, which it may only propose, and where
that is enforced**. The owner ruled it on 2026-11-18: "Option 1: the recommended
per-operation table". This record carries that table and the design around it. It
is written before the first route or schema change.

## Decisions

1. **The existing Review handlers change extractor; there is no second route tree.**
   The handlers behind `GET /api/v1/kbs/{kb_id}/review?queue=duplicates`,
   `GET /api/v1/kbs/{kb_id}/review/summary` and `POST /api/v1/kbs/{kb_id}/review/{review_id}`
   (action `keep`) take `SessionOrToken` instead of `AuthUser`. Same URLs, same
   handlers, so a token sees the items a signed-in editor sees. A session caller's
   requests and responses are unchanged: the session branch of every handler runs
   the code it runs today.

   **Revision 2026-11-19 (the read slice).** The paragraph above named three
   handlers, and the list handler served a token `queue=duplicates` only. The same
   extractor change now also covers the Decisions history
   (`GET /api/v1/kbs/{kb_id}/review/history`) and the two proof routes
   (`GET /api/v1/kbs/{kb_id}/violations/{violation_id}/proof` and
   `GET /api/v1/kbs/{kb_id}/derived/{derived_id}/proof`), and the list handler
   serves a token all eleven queue sections. What changed it: an agent has to find
   an item before it can act on it, and the read side carries none of the risk the
   table rules on. No write handler changed; decision 13 records the read shapes.

2. **MCP gains three tools over the same implementation.** `review_list` takes a
   `section` parameter and applies the existing 1 to 200 page clamp; `review_item`
   returns one item's evidence; `review_keep` keeps one duplicate pair. The read
   tools are listed for any token that reaches the base with an owner at least
   Viewer. The write tool is listed and callable only with scope `write` and an
   owner at least Editor, the rule `remember` already follows. In this cut
   `review_list` serves Duplicates and Overview; any other section answers the
   existing `unknown_queue`.

   **Revision 2026-11-19 (the read slice).** The last sentence above said
   `review_list` served Duplicates and Overview and refused every other section,
   and `review_item` read duplicate pairs by id alone. `review_list` now serves
   all thirteen sections: the eleven queue sections with `limit` and `offset`,
   `decisions` with the history route's own `page` and `per`, and `overview` with
   no paging argument. `review_item` takes `section` and `id`; `section` left out
   means `duplicates`, so a call written for the first cut reads as before. A
   name outside the thirteen still answers `unknown_queue`. Still three tools:
   no tool was added, and `review_keep` is unchanged.

   **Revision 2026-11-19 (the gated-write slice).** The heading above says three
   tools, and the revision before this one said "still three tools". There are
   now six: `review_merge`, `review_revert` and `review_batch` were added, one
   tool per operation family, listed and callable under the rule `review_keep`
   follows (scope `write`, owner at least Editor). Each calls the same operation
   function as its route (decision 5), and no tool answers a proposal. Decision
   14 records their shapes.

3. **The server enforces the authority table. Nothing else does.** Not the MCP
   client, not `ToolAnnotations` (`readOnlyHint`, `destructiveHint`,
   `idempotentHint` are set as hints only), not elicitation: the MCP specification
   puts confirmation on the client as a SHOULD, and a server cannot verify that a
   client asked anyone. The table is compiled in. It is not a per-base setting.

4. **The authority table, as the owner ruled it.** "Gate" means the server check
   named in the row; a call the gate holds becomes a proposal. The names in
   parentheses are sections of the program's research notes
   (`.project/research/evidence-*.md`), kept as the ruling cited them.

   | Operation | Ruling | Reason (evidence) |
   |---|---|---|
   | Confirm fact | propose only | Overwrites confidence with 1.0, no undo route, and the item leaves the Low confidence queue; accepting a pending item is a reviewer's act elsewhere (evidence-domain.md § Confirm fact overwrites confidence with 1.0 and has no undo route; evidence-similar.md § Per-operation precedents across comparable systems) |
   | Reject fact | propose only | Invalidates the fact and recomputes timelines; no route restores it, so a true fact leaves retrieval, reasoning and export (evidence-domain.md § Reject fact invalidates the row and recomputes timelines; no route restores it) |
   | Close fact | propose only | Rewrites the row with a fixed end; no routed undo; changes which value answers at a date (evidence-domain.md § Close fact and conflict resolutions rewrite rows inside one transaction, with no routed undo) |
   | Merge (queue decision and manual merge) | finalize behind the server gate | The only write with an exact routed undo. Gate: `hold_for_merge` (functional contradiction, live derivation, entity named in an answer, identity source), plus a hold when a person kept the pair or a merge of it was reverted (evidence-domain.md § Merge is the only Review write with an exact, routed undo; evidence-domain.md § What a wrong merge sends out of the graph is not recalled by revert; evidence-similar.md § MDM products split merge by rule class: automatic rules finalize, the rest queue for a steward) |
   | Keep | finalize | Changes only the review row; a later merge is the undo; ADR 0027 leaves keeps ungated. Safe only with the precedent exclusion decided above (evidence-domain.md § Keep (duplicates) changes only the review row) |
   | Revert merge | finalize behind the server gate | Restores a prior state and is redoable. Gate: the merge was made by a token agent (ledger row) and has not been reverted before; a revert of a person's or the governor's merge is a proposal, which keeps the governance fuse a person's judgement (evidence-similar.md § A fully autonomous bot that finalizes is allowed where its action is itself an undo and its error rate is capped; evidence-domain.md § A token-made revert would count toward the governance fuse) |
   | Resolve conflict (close old, retime new, keep both, reject new) | propose only | No routed undo and no reopen; the product's own automatic conflict handling is not built and waits on a measurement; no outside system lets a bot settle a sourced contradiction (evidence-domain.md § ADR 0043's conflict and fact governance is accepted but not built; automatic application is explicitly waiting on a measurement; evidence-similar.md § Wikipedia keeps context-sensitive judgement away from unsupervised bots) |
   | Nod on pending facts (confirm or reject) | propose only | Fixed by the charter veto in every option. Confirm turns a remark into an assertion; reject writes a permanent block with no removal path (evidence-domain.md § Nods are excluded from every agent by two decision records; evidence-domain.md § Confirming a nod writes a live fact and removes the pending row; rejecting it writes a permanent block with no removal path) |
   | Batch (duplicate pairs) | finalize behind the server gate | Inherits each member's ruling: merge members pass the merge gate, keep members finalize. Capped at 50 ids with a batch id and unit revert (evidence-similar.md § Batches on Wikidata are undoable as a unit, by anyone, with partial failure; evidence-pitfalls.md § `review/batch` is 1–500 ids, one action, not atomic, with per-item audit rows and no batch undo) |
   | Answer a governor decision (accept, override, revert) | propose only | The route is the human check on the server's own agent; a token answer would be one model approving another and would be stored as a person's answer (evidence-pitfalls.md § `review/agent/{decision_id}` answers the governor through the person's path, in three shapes) |
   | Alignment phrase decision | propose only | Acts per signature, recomputes many typed facts at once, and as a person's decision can no longer be corrected by the aligner (evidence-domain.md § Alignment decisions act per signature, not per fact, and a person's decision cannot be overridden by the product's aligner) |
   | Alignment rule decision | propose only | Same finding: queues reading or materialisation of implied facts under a person's name |
   | Alignment kind-word decision | finalize | Already open to write tokens under ADR 0014 decision 6 for a backend that sets up a mirrored base; re-decidable. Left unchanged; narrowing it would change an existing token route (evidence-domain.md § One alignment write already takes a write token and finalizes) |
   | Errata decision (approve or reject a held action) | propose only | Approval is the human check on another agent whose retractions were often of correct facts; no undo of an applied errata action was found (evidence-domain.md § The errata gate holds retractions and revisions of unflagged facts for a person because the agent removed correct facts) |
   | Ontology proposal decision (reject or adopt) | propose only | Adoption changes the contract every alignment and consistency check reads; no undo was found and ADR 0043 asks for a gate before an agent applies ontology changes (evidence-domain.md § Ontology proposals split into a reject route and an adopt route; no undo of an adoption was found) |
   | Mapping decision | finalize | Re-decidable by one more call with no status guard; exposure is the chat prompts served while a wrong confirm stands (evidence-domain.md § Mapping decisions are re-decidable and feed the chat prompt) |
   | Violation decision | propose only | One-way status change with no reopen; `accepted` on a derived contradiction lets the blocked derivation land; `fact_retracted` and `fact_closed` act as reject and close (evidence-domain.md § Violation and defect decisions are one-way status changes, and two of them have wider effects) |
   | Defect decision | propose only | One-way status change with no reopen; `fixed` asserts a person changed the ontology (same finding) |
   | Run consistency check / inference | finalize | Decides no item: it recomputes violations from the current graph, and the server already runs inference on a timer. Counted against the per-token rate limit. This row has the thinnest evidence of the table (evidence-capability-map.md § Only two of the thirteen sections have their own read route; eleven share `GET /kbs/{id}/review`, section table row 7; evidence-codebase.md Map, Architecture) |

   The Keep row's "precedent exclusion decided above" is decision 9 of this record.
   The reason on the ontology row rested on low-confidence evidence and was checked
   against the code the same day; see "Ontology adoption finding" below.

5. **One implementation per operation.** Each Review operation is one server
   function that takes the resolved caller (owner, token, base). The REST token
   branch and the MCP tool both call it, so authorization, policy, gate, ledger and
   audit exist once. The policy function lives with the `execution_gate` module:
   0027 decision 3 said the gate that checks an agent's calls would be this one and
   not a second one. It holds the table of decision 4 and answers `applied`, `held`
   or `proposed`. In this cut only the Keep row is reachable by a token.

   **Revision 2026-11-19 (the gated-write slice).** The last sentence above said
   only the Keep row was reachable by a token. The Merge, Revert merge and Batch
   rows now are, through `review_ops::merge_pair`, `merge_entities`,
   `revert_merge`, `batch` and `revert_batch`. The gate is asked inside the
   store functions those call (`hold_for_token_merge`, then `agent_policy`,
   before the merge transaction opens), so no caller of the operation layer can
   reach a token merge without it. The session branch of each handler is
   unchanged and still calls no gate.

6. **Checks and codes.** A read needs an owner at least Viewer and a token that
   reaches the base; a token whose `kb_ids` exclude the base gets 404, the same
   answer as for a base that does not exist. A write checks scope first and the
   owner's role second, the order `decide_alignment_kind_word` already uses
   (`require_write`, then the Editor check). The codes are stable, and a refused
   MCP tool call carries the same code in its structured error:

   | Code | When |
   |---|---|
   | `token_read_only` | a read-scope token calls a write (existing) |
   | `token_kb_out_of_scope` | a write token whose `kb_ids` exclude the base calls a write (existing) |
   | `token_owner_below_editor` | the token's owner is below Editor on the base (new; a plain 403 before) |
   | `already_decided` | a lost race or a repeated call: the item was decided before this call (new) |
   | `token_operation_not_open` | a token calls a Review write this build does not open to tokens (new) |

   A token that is both read-scope and owned by someone below Editor gets
   `token_read_only`, because scope is checked first.

   **Revision 2026-11-19 (checked against the code).** The first sentence above
   said of every read that a token whose `kb_ids` exclude the base gets 404. That
   is the token API's answer only. The MCP endpoint checks the token's base list
   itself, before any tool runs, and answers such a token with HTTP 403; the
   answer carries no tool list, item, count or evidence. The endpoint's answer is
   older than this record and is left unchanged: both answers give the token
   nothing from the base, and changing the endpoint's would change it for every
   MCP tool, not only the Review ones. A dead or revoked token gets 401 on both.

7. **The ledger is `token_agent_actions`.** One row per token-agent Review action.
   This cut creates the columns it writes: base, operation, target kind and id,
   owner, token, token name snapshot, outcome, request payload, time. The settled
   design has more — later state (`approved`, `declined`, `superseded`, `undone`),
   gate reason, batch id, undo reference such as a merge id, the answering person —
   and each of those columns arrives with the cut that first writes it. Agent
   History returns the governor's `agent_decisions` rows unchanged plus these rows,
   told apart by a `source` field.

   **Revision 2026-11-19 (the gated-write slice, migration 0129).** The paragraph
   above said the later columns would each arrive with the cut that first writes
   it. Migration 0129 adds them in one file, all nullable: `gate_reason`,
   `batch_id`, `merge_id`, `target_key`, `state`, `state_at` and `state_by`
   (the answering person). `merge_id` is the undo reference: on an `applied`
   merge row the merge it made, on a `merge.revert` row the merge it targets, on
   an approved merge proposal the merge the person's approval made.
   `target_key` is a text key for "the same target": the review id for a queue
   merge, the two entity ids in ascending order joined by `|` for a manual
   merge, the merge id for a revert. The same migration adds the partial unique
   index `token_agent_actions_open_proposal_idx` on `(kb_id, operation,
   target_key)` over rows with outcome `held` or `proposed` and no `state`. That
   index, not application code, is what keeps one open proposal per base,
   operation and target when two calls arrive at once. `approved`, `declined`
   and `superseded` are stored by the answer route. **`undone` is read from the
   merge, not stored**: an `applied` merge row reads `state` `undone`, with
   `state_at` the revert time, whenever `entity_merges.reverted_at` of its
   `merge_id` is set. A person's revert through the unchanged session handler
   therefore shows on the ledger with no write. The column's check constraint
   admits `undone`, but no statement writes it. In Agent History a token row
   gains `reason`, `batch_id`, `merge_id`, `state` and `state_at`, each present
   only when it has a value, so the row of a single keep is what it was.

8. **An agent action is recorded as owner plus token.** `audit_events.actor_id`
   stays the owner (0014 decision 1). Every token Review write adds
   `detail.token_id` and a `detail.token_name` snapshot taken at insert time,
   because the audit table is append-only and a renamed or revoked token must still
   read as it was. It is shown as "owner via token name", the wording the nod card
   already uses for `remember`. The stored HTTP `User-Agent` is returned to readers
   marked as declared and is never read for authorization. MCP `clientInfo` is not
   parsed. **A failed snapshot or audit write fails the action**: attribution is
   part of the write, not a best-effort trailer.

   **Revision 2026-11-20 (who proposed what a person answered).** The paragraph
   above names the token on the token's own rows only. A person's answer to a
   token proposal (the person's decision row written by an approval, and the
   `agent.proposal.declined` and `agent.proposal.superseded` rows) is the
   person's act and carries no `token_id` or `token_name`, so Activity showed it
   without the token that proposed it. Every such row already names its proposal
   in `detail.agent_proposal`. `GET /kbs/{kb_id}/audit` now reads that proposal's
   ledger row in the same base when it lists the event and returns two more
   keys: `proposed_by_token`, the token name snapshot stored at propose time
   (unchanged by a later rename or revoke), and `proposed_by_owner`, the display
   name of the token's owner (absent when that account is gone). The person
   stays the actor (`actor_id`, `actor_name`); the stored audit detail is not
   changed and no migration is needed. An event that answers no token proposal,
   and every row of `GET /kbs/{kb_id}/review/history`, has the keys it had
   before. Activity shows such a row as the person and the action, followed by
   "proposed by owner via token name".

9. **A token decision is never human precedent.** The governor's precedent queries
   and `settled_by_people` exclude audit rows whose detail carries a token id and
   review rows marked by the new nullable `resolution_reviews.decided_token`.
   `after_human_decision` is not called for a token decision, so a token action
   starts no govern round and settles no governor proposal. The governor's own
   decision route, `review/agent/{decision_id}`, stays session-only.

10. **Two limits are named constants, for the cuts that enforce them.** A token
    batch is capped at 50 ids (the session cap stays 500). Finalized token writes
    are limited to 200 per rolling hour, per token and base, counted from the
    ledger. Neither number comes from a measurement on this product (50 is the low
    end of Wikidata's 50 to 250 trial range); both are accepted as named constants.

    **Revision 2026-11-19 (the gated-write slice).** The paragraph above named
    the two constants "for the cuts that enforce them" and said nothing of the
    breaker, which the status line listed as open. Both are now enforced, in the
    shared operation layer and nowhere else. The constants are
    `review_ops::TOKEN_BATCH_MAX` (50) and `review_ops::TOKEN_WRITES_PER_HOUR`
    (200). One function, `review_ops::guard`, runs before every token call that
    would finalize (`keep`, `merge_pair`, `merge_entities`, `revert_merge`),
    after the write checks and before the store function; a batch member, a unit
    revert item and each MCP write tool go through those four functions, so no
    entry goes around it.

    - **The limit.** `guard` counts the `applied` ledger rows of that token on
      that base in the last rolling hour. At 200 or more the call answers HTTP
      429 with the new code `agent_rate_limited`, changes nothing and writes no
      row; the 200th write applies and the next is refused. Keep, merge and
      revert count; proposals and refused calls do not. Inside a batch each
      member past the limit carries the code as its own outcome. The limit is
      checked before the gate, so a call past the limit answers 429 even if it
      would only have proposed. The count is a read before the write, not part
      of its transaction, so calls of one token in flight at the same moment can
      overshoot by the number in flight.
    - **The breaker.** `guard` then asks whether, within
      `governance::FUSE_WINDOW_DAYS` (7), at least `governance::FUSE_REVERTS`
      (2) merges made by that token on that base were reverted by a person.
      While that holds, the token's merge and revert land as proposals with
      reason `breaker`; its keep still finalizes; when the window has passed it
      finalizes again. Reverts made by a token and reverts of another token's
      merges do not count. A person's revert is recognised from the ledger (the
      merge is reverted and no applied token revert row names it) and also from
      the person's own `merge.revert` audit row, so it still counts when a
      token reverted the same merge at the same moment. The breaker borrows the
      governor's two constants and is separate from the governance fuse.

11. **A merge revert restores graph state only.** Derivations concluded from the
    merged entity, violations and alerts opened on it, triples already exported and
    answers already given are not recalled (0027). Tool descriptions say so, and a
    revert is verified on graph state and nothing else.

    **Revision 2026-11-19 (the gated-write slice; the comparison as corrected).**
    The paragraph above said a revert is verified on graph state and did not say
    what "graph state" is compared. It is this: every `facts` row of the base,
    every `entities` row except `updated_at`, and every `fact_evidence` row are
    equal before the merge and after the revert; for `typed_fact_sources`, every
    row present before the merge is present after the revert. That last clause
    is a correction made during the build, when the first test of an exact
    comparison failed. The existing revert was not changed, and it leaves two
    differences, on a person's merge as on a token's:

    - **The untyped target.** A target that had no type before the merge keeps
      the type it took from the source. The comparison is therefore made on
      typed entities.
    - **Copied statement links.** When both sides hold the same typed fact, the
      merge keeps one and copies the duplicate's statement links
      (`typed_fact_sources` rows) onto the surviving fact. The revert removes
      the copied `fact_evidence` rows it recorded and nothing in
      `typed_fact_sources`, so the surviving fact keeps those statement links.
      They are the only rows the comparison allows to be added.

    Review rows and recorded signature violations are outside the comparison,
    and a revert restores neither. So a revert is not a full undo, and nothing
    here describes it as one: it does not recall derivations, violations,
    exports or answers drawn while the merge was live, and it leaves the two
    differences above. The description of `review_revert` and the API and MCP
    documentation say the same. Recording the copied links at merge time and
    removing them in the revert is a candidate change for upstream; it is not
    built here, because it would change the revert a signed-in person uses.

12. **What this cut leaves closed.** To a token: merge, manual merge, revert and
    batch (how each is answered: first revision below); every propose-only row and the
    proposal store that would hold them; listing of the other eleven sections,
    evidence for their items, the proof routes and true section totals; the
    per-token rate limit and breaker. Unchanged for everyone: the nod on `remember`
    and every `pending_facts` decision, which no token finalizes in any option of
    the ruling; the person's ungated merge and 500-id batch; the labels of existing
    machine actors in Activity.

    **Revision 2026-11-19 (checked against the code).** The first clause above
    said, in a parenthesis after the four names, that merge, manual merge, revert
    and batch were all answered with the code `token_operation_not_open`; the
    parenthesis now points here. Only `merge` sent to the decision route is.
    Manual merge, revert and batch are session-only routes: their handlers take
    a signed-in session and nothing else, so they answer a token with 401 until
    M003 opens them.

    **Revision 2026-11-19 (the read slice).** The list above also named, as
    closed, the listing of the other eleven sections, evidence for their items,
    the proof routes and true section totals. The read slice opened all four
    (decisions 1, 2 and 13). Still closed to a token: merge, manual merge, revert
    and batch; every propose-only row and the proposal store; the per-token rate
    limit and breaker. The "unchanged for everyone" part stands as written.

    **Revision 2026-11-19 (the gated-write slice).** The revision above still
    listed, as closed to a token: merge, manual merge, revert and batch; the
    proposal store; the per-token rate limit and breaker. The gated-write slice
    built all of them (decisions 7, 10 and 14). Open to a write token of an
    editor now: keep, merge and manual merge behind the gate, revert of a token
    agent's merge, batch on duplicate pairs and its unit revert. Still closed to
    every token: answering a proposal, its own included (the `agent-proposals`
    routes take a session only); the governor's decision route; every
    propose-only row of the table, which also has no proposal to land as, since
    the store holds merge and revert proposals only; the mapping decision and
    the consistency check, which the table rules "finalize" but no cut has
    opened; batch for anything other than duplicate pairs. The "unchanged for
    everyone" part stands as written: the nod, the person's ungated merge, its
    500-id batch and its revert run the code they ran before.

13. **The read shapes of the read slice.** Added 2026-11-19. The reads go through
    the handlers and the operation layer of decisions 1 and 5; these are the
    points where a shape had to be ruled.

    - **`total` is a new field; `counts` is unchanged.** Every list response
      carries `total`, the number of rows that section's list returns over all
      pages, for a session and a token alike. It is the only key a session's
      list response gained. `counts` keeps its keys and values because the
      Review tab's badges read it, and for Alignment (the badge omits proposed
      implication rules), Low confidence and Agent History the badge named
      after the section is not the number of listed rows, so a client pages by
      `total`.
    - **The Low confidence list is unchanged, and its total counts the listed
      rows**, derived facts included, although the badge leaves them out.
    - **Decisions keeps `page` and `per`** and its `{events, total}`. It is not
      folded into the list route, and `review_list` passes the two arguments
      through for section `decisions`.
    - **The Alignment list has an identifier tie-break.** It is ordered by
      `decided_at` and then by the item's own identifier (the id of a phrase or
      rule, the word of a kind word). Rows written by one aligner run share a
      time, and without the tie-break a client paging by `limit` and `offset`
      could see an item twice or not at all.
    - **`review_item` identifies an item by `section` and `id`.** The id is the
      item's `id` from the list; an Alignment kind word has none and is named by
      its word, compared exactly as stored; an Agent History row is named by its
      row id whichever source it has. The item is found in the section's own
      list, so what comes back is the list's object and passes the list's reader
      check. There is one not-found answer, with no evidence in it, for an
      unknown id, an id of another base and an item no longer waiting in that
      section; `overview` lists no items and gives the same answer.
    - **An Axioms item carries a `proof` key**: the proof steps for kind
      `derived_contradiction`, null for any other kind, which is not an error.
      No other section gains a key.
    - **`mappings` is refused for a token** with `unknown_queue`. The list route
      serves it to a session, but it is not a Review section.
    - **The MCP endpoint's 403 is unchanged** (decision 6).
    - **No Review write is added.** No write handler gained a token branch and
      no MCP write tool was added; reading "Awaiting your nod" gives a token no
      way to nod.

14. **The shapes of the gated-write slice.** Added 2026-11-19. The writes go
    through the handlers and the operation layer of decisions 1 and 5 and are
    recorded as decisions 7 and 8 say; these are the points where a shape had to
    be ruled. The request and response bodies are in the API and MCP
    documentation (`web/src/docs/mcp.md`).

    - **Routes.** Four existing handlers change extractor in place and keep
      their session branch: the decision route
      (`POST /api/v1/kbs/{kb_id}/review/{review_id}`, now `keep` or `merge`),
      the manual merge (`POST /api/v1/kbs/{kb_id}/entities/merge`), the revert
      (`POST /api/v1/kbs/{kb_id}/merges/{merge_id}/revert`) and the batch
      (`POST /api/v1/kbs/{kb_id}/review/batch`). Three routes are new: the unit
      revert `POST /api/v1/kbs/{kb_id}/review/agent-batches/{batch_id}/revert`
      and the two proposal answer routes under
      `/api/v1/kbs/{kb_id}/review/agent-proposals`.
    - **Six holds, two of them looked up by entity ids.** The token merge gate
      is `hold_for_merge` (identity source, functional contradiction, live
      derivation, entity named in an answer) followed by two holds only a token
      merge gets, with the reason codes `identity`, `contradiction`, `derived`,
      `answered`, `kept_by_person` and `reverted_merge`. "A pair a person kept"
      is: the latest decided review of those two entities, in either order, was
      decided keep by a person and not through a token. "A pair with a reverted
      merge" is: any merge between those two entities, in either direction, has
      been reverted. Both are looked up by the two entity ids, because a manual
      merge has no review row; the queue merge uses the same lookup. The
      product's own deciders keep calling `hold_for_merge` and never see the
      two new holds.
    - **A held call is an answer, not an error.** A merge the gate holds, and a
      call that may only propose, answer HTTP 200 with outcome `held` or
      `proposed`, a stable `reason` and an `explanation` sentence that is not
      part of the contract. Nothing in the graph changes; the ledger row holds
      the full request and an `agent.proposal` audit row is written with it.
      The propose-only reasons are `merge_by_person`, `merge_by_governor` and
      `breaker`.
    - **Proposal target keys.** One open proposal per base, operation and
      target (decision 7). A queue merge's target is its review id; a manual
      merge's is the unordered pair of entity ids, so both directions are one
      proposal and it keeps the direction of the first call; a revert's is the
      merge id.
    - **`already_proposed` rides on a 200 answer.** A repeat, or a second call
      at the same moment, writes no row and returns the open proposal's id with
      `"code": "already_proposed"` beside the usual keys. It is not a refusal.
    - **An applied merge that failed afterwards is still applied.** The merge,
      the review row, the ledger row and the audit row commit in one
      transaction. The steps after the commit can fail; the answer is then
      still `applied`, with `follow_up_error`, and never a failure.
    - **A token reverts any token agent's merge.** A merge the ledger shows a
      token agent made on that base, through whichever token, and that is not
      reverted, is reverted outright. A merge a person made (one made by
      approving a proposal included) or the governor made is a proposal, so a
      token cannot finalize such a revert or move the governance fuse. An
      unparseable body on the token revert is 422 `invalid_body`.
    - **The precedent rule for merges made and reverted by tokens.** Decision 9
      stands and is extended. A live token merge counts in no governor
      precedent read. A merge made by a token and reverted by a token is not
      counted as a reverted merge either: no person took a position at any
      point. A person's revert of a token merge counts as that person's revert,
      as any revert does. No token action lowers the governor's bar, settles a
      pair as decided by people, or starts a govern round.
    - **Batch and unit revert.** A token batch takes 1 to 50 ids and one
      action, `merge` or `keep`; the server issues one `batch_id` and runs each
      member through the single-item function, so a member gets nothing a
      single call would not. Outcomes come back per item in request order; a
      refused member is that item's code, not a failed batch. Zero or more than
      50 ids is 422 `batch_size`, and an unknown action is the session batch's
      own uncoded 422; both refuse the whole batch. The unit revert reverts
      each merge the batch applied, best effort, one outcome per merge; an
      already reverted merge is that item's `already_decided`, keeps and
      proposals are left. Its callers are a write token of an Editor (each
      merge through the token revert) and a signed-in Editor (each merge
      through the person's revert); there is no Review tab button. A batch that
      made no merge answers 200 with no outcomes; 404 is only for a batch id no
      ledger row of the base carries.
    - **The answer routes take a session only.** Single
      (`…/agent-proposals/{proposal_id}`) and bulk (`…/agent-proposals`, 1 to
      500 ids) need a signed-in Editor; every personal token gets 401, and no
      MCP tool answers a proposal (a call to an invented `review_` name is
      JSON-RPC -32601). Approval runs the person's own merge or revert with the
      person as actor and no gate, and its audit row names the proposal
      (`agent_proposal`), so the merge is the person's decision and counts as
      human precedent. A target that can no longer be decided closes the
      proposal `superseded` and applies nothing. An answered proposal answers
      `already_decided`. Also built: bulk per-item `not_found`; 422
      `unknown_action` for an action other than approve or decline; 403 for a
      session below Editor; an uncoded 409 while a running governor round holds
      the pair, in which case the proposal stays open. Approving a manual-merge
      proposal leaves the pair's own pending review open, as the session manual
      merge does.
    - **Read shapes.** A Duplicates item gains `agent_proposal` and a
      merges-list row gains `agent` and `agent_proposal`, each serialized only
      when it has a value, so every other row is byte for byte what it was, for
      session, token and MCP readers. The Merges section shows a token agent's
      merge as "owner via token name" from `agent`.
    - **MCP errors.** A refused tool call carries the REST code when the route
      has one (`not_found` for a 404). Three cases carry a message only: a pair
      held by a running governor round, a malformed `review_merge` or
      `review_revert` call (both argument forms, half a form, or none), and an
      unknown `review_batch` action.
    - **On every token write route** a write token whose base list excludes the
      base is refused 403 `token_kb_out_of_scope`, between the scope check and
      the role check (decision 6).

    **Known limits this cut leaves.**

    - A person's approval is not one transaction with its merge. The merge or
      revert commits, then its audit row is written, then the proposal is
      closed. A crash between them leaves the merge, as the person's, and an
      open proposal; a later answer closes that proposal `superseded` without a
      merge id, so the link from proposal to merge is lost for that case.
    - A token revert is not one transaction with its ledger and audit rows. The
      unchanged `resolution::revert_merge` opens and commits its own
      transaction; the two rows are written after it, with retries. If they
      cannot be written the merge is reverted with no `merge.revert` row: its
      ledger row reads `undone` and a second revert answers `already_decided`.
    - A person's and a token's revert of one merge sent at the same moment can
      both run the unchanged revert. Answers to one proposal, and unit reverts
      of one batch, are serialised within one server process only.
    - Recording and removing the copied statement links in the revert
      (decision 11) is left as a candidate change for upstream.

## Ontology adoption finding

The ontology row's reason, "no undo was found", came from low-confidence evidence.
Checked on 2026-11-18 against the code: **confirmed. An adopted ontology proposal
cannot be undone.** The ruling was the owner's and is unchanged; the reason stands
as written.

What was read:

- `save_agent_proposals`, `open_proposal`, `open_proposals` and `reject_proposal`
  in the store's ontology module. A proposal row is inserted `open`; a refresh
  rewrites it only `WHERE status = 'open'`; `reject_proposal` moves `open` to
  `rejected`; both readers return `open` rows only.
- The adopt route. It is `adopt` in `question_routes` (`POST
  /kbs/{id}/ontology/proposals/adopt`), which calls `ontology_agent::adopt`; the
  proposal route in `ontology_routes`, `decide_proposal`, only rejects and answers
  `adoption_endpoint` to an `adopted` status. `adopt_on` and `finish_adoption`
  commit in one transaction: the new class or property (`create_entity_type`,
  `create_relation_type`) or, for `map_to`, a person's binding decision per shape;
  the regression cases with origin `adoption`; the queued `embed_ontology` and
  alignment jobs; and the status `adopted`. No statement anywhere moves a row out
  of `adopted` or `rejected` — three statements write `ontology_proposals`, and
  those are the three above. `signatures_already_proposed` then keeps the agent
  from proposing the same shape again.
- `unadopt_types` in the store's resolution module, behind `type_resolution_undo`.
  It undoes something else: one batch of entity retypes (`entity_retypes`), putting
  those entities back in their former class and restoring `proposed_type`. It does
  not read or write `ontology_proposals`, removes no class or property, and
  touches no binding or regression case. Its own comment says the type is not
  deleted.
- `adopt_iri_onto_key`. Also something else: an OWL import claiming an IRI onto an
  existing class key, written only when the IRI is empty. It has no inverse
  either, and it is not the adoption of a proposal.

What a person can do afterwards is repair, not undo: delete the created class
(`delete_entity_type` refuses while any entity uses it) or property, and re-decide
bindings one at a time in Review. The proposal stays `adopted`, the alignment that
ran against the new element has already rewritten typed facts, and the regression
cases remain. That is the situation the ruling's reason describes.

## Numbers

This record is 0076 and the ledger's migration is 0128: each the lowest number
above every merged number and every number claimed by an open upstream pull request
(0127 is claimed by upstream #1496). The ledger's second migration is 0129 (the
gated-write slice, decision 7), taken by the same rule; it is named here by the
revision of 2026-11-20 (the remaining Review writes), which itself adds no
migration, so 0128 and 0129 are this record's only migrations. The gaps are not reused (0055 to 0059 and 0068
here; 0081 to 0089, 0107, 0108, 0112 and 0124 among migrations): nothing says a
skipped number may be taken, and the stated rule is the latest number plus one.

`scripts/check-upstream-numbers.sh <adr-number> <migration-number>` re-checks both
before each landing. It only reads `deeplethe/utopia`: the files of
`docs/decisions/` and `migrations/` on `dev`, and the files open pull requests add
there. Exit 0 is free, 1 names the colliding file, 2 means upstream could not be
read — a number that was not checked is not treated as free. Migration SQL is
written idempotent, so a renumbering for an upstream submission replays safely on a
database that ran the old number.

## Dead ends

- **A separate token-only route tree.** Two trees must be kept equal by hand, and
  "the same items a signed-in editor sees" is safest when both callers run the same
  handler. The one-extractor pattern already exists in this file and in the token
  reads.
- **Opening the existing write handlers to tokens as they stand.** The person's
  merge path calls no gate; this is the straight-through agent write the program
  vetoes.
- **A per-base authority setting.** 0014 decision 6 keeps base settings
  session-only, and nothing found asks for the extra surface.
- **MCP tools implemented beside `remember`, or calling the REST routes over
  HTTP.** Two implementations drift. One list tool per section lost too: the
  per-section dispatch already exists as one `match` on the queue. Writes get one
  tool per operation family, not one omnibus write tool.
- **MCP `clientInfo`, or a declared agent-name field, as the agent's name.** The
  server is stateless on the implemented revision and cannot tie a `tools/call` to
  an `initialize`; the field is optional and unverified; the REST API has no
  equivalent. The token's name is the only name the server can vouch for.
- **Widening `agent_decisions` with actor and token columns** (as 0043 planned).
  Its constraints admit only duplicate-pair merge and keep, and the governor's
  precedent and fuse queries read it, so every one of them would need a token
  exclusion.
- **Recording token actions with a null actor**, as the governor's rows are, so
  that today's queries skip them. It breaks 0014 decision 1 and Activity would
  label the row "AI adjudicator".
- **Reusing the three per-kind stores** (`pending_facts`, `agent_decisions`,
  `errata_actions`) as the proposal store. Most propose-only operations have
  nowhere to land in them, and each new store would need its own uniqueness rule,
  queue and answer route.
- **Opening the existing batch to tokens with its 500 cap.** One injected
  instruction becomes 500 merges with no handle to find or undo the set.
- **Propose only, for every operation** (option 2 of the ruling). Fully viable and
  what the program falls back to. Not chosen because the product's gated governor
  decided 97.7 percent of 589 labelled pairs on its own, approval by tired
  reviewers is a documented failure, and no bulk human path exists outside
  duplicates.
- **Finalize only what has an exact routed undo** (the third option put to the
  owner). Not chosen; the ruling was option 1.
- **Filling the gap numbers.** See Numbers.

## Open questions

- **A token keep is counted under its owner in Decisions and Overview.** A token
  keep is stored as a `review.keep` audit row with the owner as actor, so the
  Decisions section lists it and the Overview's decided counts include it under
  that person. The owner ruled on 2026-11-18: "leave Decisions and Overview
  unchanged for now". This cut does not change those two reads. It is ruled for
  now, not settled: Agent History and Activity say "owner via token name", and
  these two do not yet.
- **The two constants.** 50 and 200 are not measured on this product.
- **Whether bulk approval of agent proposals reproduces blind acceptance.** 0025
  measured it for the governor; an approved proposal counts as human precedent
  exactly as an accepted governor proposal does.
- **The precedent exclusion is covered by store and HTTP tests, not by a govern
  round with a model.** (Reworded by the revision of 2026-11-20, the remaining
  Review writes.) The store tests show that a token keep changes no precedent read
  (`precedents_for`, the ledger search, `decided_before`, `settled_by_people` and
  the gate's bar), settles no governor proposal and queues no govern job, and that
  a live token merge, and a merge made and reverted by tokens, count in no
  governor precedent read. The HTTP tests show that a token keep or merge over the
  API starts no govern round, that a token merge and a token-reverted token merge
  move no governor read while a person's revert of one counts as any revert does,
  and that a token's proposed answer to a governor decision changes neither the
  decision, the pair nor the fuse and starts no round. What is not shown: no full
  govern round with a model was run after a token write. If a token row still
  steered the governor in such a round, no agent finalize would be safe and the
  gated rows would change shape.

## Status history

**Revision 2026-11-19 (the read slice; checked against the code).** Until that
date the `open:` part of the status line read:

> every row of the table except Keep stays closed to tokens (merge, manual merge,
> revert, batch, every propose-only operation and the proposal store); the
> per-token rate limit and breaker; token reads of the other eleven sections and
> of Agent History; a token keep is counted under its owner in Decisions and
> Overview (ruled "unchanged for now")

Two things in it no longer held. "Every row except Keep" was wrong when written:
the table's own row for the alignment kind-word decision says it is already open
to write tokens under ADR 0014 decision 6, and `decide_alignment_kind_word`
takes a write token. And the token reads of the other eleven sections and of
Agent History were built by the read slice, so they left the open list.

**Revision 2026-11-19 (the gated-write slice, migration 0129).** Until that
slice the `open:` part of the status line read:

> this record has opened only the Keep row of the table to tokens, and the
> alignment kind-word decision was already open to write tokens under ADR 0014
> decision 6; every other row stays closed to tokens (merge, manual merge,
> revert, batch, mapping decision, the consistency check, every propose-only
> operation and the proposal store); the per-token rate limit and breaker; a
> token keep is counted under its owner in Decisions and Overview (ruled
> "unchanged for now")

The slice built merge, manual merge, revert, batch, the proposal store for
merge and revert proposals, the rate limit and the breaker, so they left the
open list, and the status line gained a clause naming the slice and its
migration. What stays open is what the line now says: the mapping decision, the
consistency check and every propose-only operation are closed to tokens, and the
ruling on Decisions and Overview is unchanged. Decisions 2, 5, 7, 10, 11 and 12
carry the dated revisions for this slice, and decision 14 is new.

**Revision 2026-11-20 (the remaining Review writes, no migration).** Until that
date the `open:` part of the status line read:

> this record has opened the Keep, Merge, Revert merge and Batch rows of the
> table to tokens, and the alignment kind-word decision was already open to
> write tokens under ADR 0014 decision 6; every other row stays closed to tokens
> (mapping decision, the consistency check, and every propose-only operation,
> for which the proposal store holds nothing yet: it holds merge and revert
> proposals only); a token keep is counted under its owner in Decisions and
> Overview (ruled "unchanged for now")

The slice opened every remaining row of the authority table under its ruling,
so those rows left the open list; the status line gained a clause naming the
slice, and the ruling on Decisions and Overview is still open. The table itself
is unchanged. What shipped, checked against the code:

- **The twelve propose-only rows** (confirm, reject and close a fact, resolve a
  conflict, the pending nod, answer the governor, the Alignment phrase and rule,
  errata, the ontology proposal, the violation, the defect) are reached by a
  write token of an Editor on the routes a signed-in person uses, except the two
  below. A token's call changes nothing: no fact, entity, evidence, review row,
  ontology row or job. It runs the scope, base-list and owner-role checks, then
  the person's own body checks with the person's codes, then the target (404 for
  an unknown or foreign target, 409 `already_decided` for a decided one), then
  the checks that need the target's state, and lands one ledger row with outcome
  `proposed` and reason `propose_only` plus an `agent.proposal` audit row. The
  answer is 200 with `outcome`, `action_id`, `reason` and `explanation`, and
  `"code": "already_proposed"` when an open proposal of the same base, operation
  and target already exists. A propose-only call never counts toward the limit
  and is never refused by it or by the breaker.
- **Answering stays a person's.** A signed-in Editor approves or declines a
  proposal on the two answer routes of decision 7, singly or in bulk, in the
  Agent History section of the Review tab; the item's own card (the Ontology
  page for an ontology proposal) shows the open proposal as `agent_proposals`.
  Approval runs that person's own code for the operation, as that person, with
  `agent_proposal` in the audit detail and no `token_id`; the token's rationale
  is never passed as the person's reason. An approved Alignment phrase or rule
  is queued as a job (`job_id`, `queued`), not applied at once; an approved
  pending confirm returns `fact_id`. A refusal from the person's code is returned
  and the proposal stays open.
- **The target key needs no migration.** Each proposal names its target by the
  item's own id, except an ontology proposal, named by section and key joined by
  `:` (one open proposal per ontology proposal, whichever action), and an
  alignment kind word, named by the normalized word against the base. Confirm,
  reject and close of one fact are three operations, so three proposals may be
  open on one fact. The 0128 and 0129 columns hold all of it, so the slice added
  no migration and the schema version is unchanged.
- **A stale target is read, not guessed.** Before an approval runs the person's
  code it reads the target's state through one typed reader
  (`review_targets::target_state`, with the stored request and, for a conflict
  or ontology proposal, the fingerprint taken at propose time). A missing or
  decided target closes the proposal `superseded` and applies nothing. No error
  text is matched to decide that.
- **Two token entries of their own.** The nod and the governor's answer keep
  their person routes, which still answer every token 401: opening the nod route
  would let a token touch what the charter keeps for people, and opening the
  governor's decision route would add a second person merge path. A token
  proposes on `POST /kbs/{kb_id}/review/pending/{pending_id}/propose` and
  `POST /kbs/{kb_id}/review/agent/{decision_id}/propose` instead. A signed-in
  session gets 403 `token_required` there, and an unparseable body gets 422
  `invalid_body` after the scope checks.
- **Three rows finalize.** The mapping decision and the consistency check or
  inference run are applied at once, behind the limit and the breaker of
  decision 12, with one `applied` ledger row and one audit row naming the token;
  inference on a base with materialized inference off is 422 `inference_off`.
  The kind-word decision keeps its route, codes and plain 403 for a
  below-Editor owner, and now writes an `applied` ledger row in the same
  transaction as the decision; it counts toward the limit and is never refused
  by it. The ontology proposals and the mappings gained token reads.
- **The MCP tools.** Fourteen were added: `review_fact`, `review_conflict`,
  `review_violation`, `review_defect`, `review_errata`, `review_alignment`,
  `review_ontology`, `review_pending_fact`, `review_governor` (propose only),
  `review_mapping`, `review_check`, `review_kind_word` (finalize), and the reads
  `review_ontology_list` and `review_mapping_list`, so twenty Review tools are
  served, each calling the same function as its route. Corrected reading: a
  write tool called by a token without the right is not a JSON-RPC -32601; it
  is a tool error carrying `token_read_only` or `token_owner_below_editor`, and
  the tool is left out of `tools/list` for that token. The -32601 answer is only
  for a `review_` name the server does not serve. Over REST, a write token whose
  base list excludes the base gets 403 `token_kb_out_of_scope`; the MCP endpoint
  refuses such a token with HTTP 403 before any tool runs.
- **Known limits.** A consistency check or inference run commits its own changes
  before its ledger and audit rows; if that write fails the call errors while
  the run's changes stay. The kind-word audit row is written best effort after
  the commit. A proposal the person's code refuses at approval stays open until
  a person declines it; nothing sweeps stale proposals. Every Review MCP call,
  a refused one included, writes the `mcp.tool_called` audit row.

The `## Numbers` paragraph and the last open question were corrected in place by
this revision: the first now names migration 0129 beside 0128, and the second
says what the store and HTTP tests cover and that no full govern round with a
model was run after a token write. The full route and tool reference, with each
refusal code, is `web/src/docs/mcp.md`.
