# 0076 · A token agent acts on Review under a ruled table

- **Status**: In progress · accepted 2026-11-18 · the keep slice built 2026-11-19 (migration 0128): a personal token reads Duplicates and Overview and keeps one duplicate pair through the policy function, over the token API and the MCP tools `review_list`, `review_item` and `review_keep`, with the `token_agent_actions` ledger, the precedent exclusion, and "owner via token name" in Agent History and Activity · open: every row of the table except Keep stays closed to tokens (merge, manual merge, revert, batch, every propose-only operation and the proposal store); the per-token rate limit and breaker; token reads of the other eleven sections and of Agent History; a token keep is counted under its owner in Decisions and Overview (ruled "unchanged for now")
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

2. **MCP gains three tools over the same implementation.** `review_list` takes a
   `section` parameter and applies the existing 1 to 200 page clamp; `review_item`
   returns one item's evidence; `review_keep` keeps one duplicate pair. The read
   tools are listed for any token that reaches the base with an owner at least
   Viewer. The write tool is listed and callable only with scope `write` and an
   owner at least Editor, the rule `remember` already follows. In this cut
   `review_list` serves Duplicates and Overview; any other section answers the
   existing `unknown_queue`.

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

7. **The ledger is `token_agent_actions`.** One row per token-agent Review action.
   This cut creates the columns it writes: base, operation, target kind and id,
   owner, token, token name snapshot, outcome, request payload, time. The settled
   design has more — later state (`approved`, `declined`, `superseded`, `undone`),
   gate reason, batch id, undo reference such as a merge id, the answering person —
   and each of those columns arrives with the cut that first writes it. Agent
   History returns the governor's `agent_decisions` rows unchanged plus these rows,
   told apart by a `source` field.

8. **An agent action is recorded as owner plus token.** `audit_events.actor_id`
   stays the owner (0014 decision 1). Every token Review write adds
   `detail.token_id` and a `detail.token_name` snapshot taken at insert time,
   because the audit table is append-only and a renamed or revoked token must still
   read as it was. It is shown as "owner via token name", the wording the nod card
   already uses for `remember`. The stored HTTP `User-Agent` is returned to readers
   marked as declared and is never read for authorization. MCP `clientInfo` is not
   parsed. **A failed snapshot or audit write fails the action**: attribution is
   part of the write, not a best-effort trailer.

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

11. **A merge revert restores graph state only.** Derivations concluded from the
    merged entity, violations and alerts opened on it, triples already exported and
    answers already given are not recalled (0027). Tool descriptions say so, and a
    revert is verified on graph state and nothing else.

12. **What this cut leaves closed.** To a token: merge, manual merge, revert and
    batch (answered `token_operation_not_open`); every propose-only row and the
    proposal store that would hold them; listing of the other eleven sections,
    evidence for their items, the proof routes and true section totals; the
    per-token rate limit and breaker. Unchanged for everyone: the nod on `remember`
    and every `pending_facts` decision, which no token finalizes in any option of
    the ruling; the person's ungated merge and 500-id batch; the labels of existing
    machine actors in Activity.

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
(0127 is claimed by upstream #1496). The gaps are not reused (0055 to 0059 and 0068
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
- **The precedent exclusion has not been exercised end to end.** If a token row
  still steers the governor, no agent finalize is safe and the gated rows change
  shape.
