---
id: T009
title: Prove the whole ruled table over REST and MCP and correct the batch tool description
wave: 4
deps: [T008]
status: done
agent: build_T009
base: 1d5e6b75e66b32b0ee353d3fab596404a8467fbd
worktree: null
task_branch: null
files:
  - crates/utopia-server/src/api/review_ops.rs
  - crates/utopia-server/src/api/review_routes.rs
  - crates/utopia-server/src/api/review_tools.rs
  - crates/utopia-server/src/api/mod.rs
  - crates/utopia-server/src/api/review_token_table_tests.rs
---

# T009 — Prove the whole ruled table over REST and MCP and correct the batch tool description

## Context

T002 to T008 opened every remaining row of the owner's ruled authority table. This task proves the properties that must hold for every operation at once, in one table-driven test module, and repairs any operation that does not hold them. The policy lives in `crates/utopia-store/src/execution_gate.rs` (`AgentOperation` line 149, `agent_policy` line 226): twelve rows propose only, three finalize behind the server gate (merge, revert, batch) and four finalize. The M003 patterns to follow are in `crates/utopia-server/src/api/review_routes.rs` and `crates/utopia-server/src/api/review_ops.rs`: the token branch of `review_routes::decide` (lines 353 to 374), `review_ops::authorize_write` (lines 69 to 84), `review_ops::keep` (lines 361 to 388), `review_ops::guard` (lines 317 to 344) and the answer dispatch `review_routes::answer_proposal` (lines 850 to 918). The description of `review_batch` (`crates/utopia-server/src/api/review_tools.rs` lines 312 to 330) says an unknown action is refused with `batch_size`; the shipped behaviour is an uncoded 422 (`crates/utopia-server/src/api/review_token_batch_tests.rs` lines 585 to 607), and the owner ruled the description is corrected in this milestone (driver D128).

## Approach

- One table in the test module names, per operation: the policy row, the REST call, the MCP tool and arguments, a seeded target, an invalid body with the person's code, and a decided or missing variant where the operation has one. Each test walks the table. Use the helpers of `crates/utopia-server/src/api/review_token_write_support.rs`.
- N9 (judgment): every new token entry asks the compiled policy for its operation, no propose-only operation has a route, tool or parameter that applies it, and a read-scope or below-editor caller reaches no write over either surface. The table test asserts the outcome of every row against `agent_policy`, not against a list written in the test.
- N3 (judgment): the two answer routes take a session only; a token, the proposing token included, gets 401 from both, no listed tool name contains `proposal`, `approve`, `decline` or `answer`, and the M003 tests of this refusal are not edited.
- N2 (judgment): the table test checks, for one approval of every propose-only operation, that the audit row has the person as actor, names the proposal and carries no `token_id`.
- E19 is planned to its corrected reading (driver D131), which the orchestrator records in INTENT.md before this plan is built: over MCP a write tool is not listed for a read-scope or below-editor caller, and a call to one returns the coded tool error (`token_read_only`, `token_owner_below_editor`), as the M003 tools do. Over REST a write token whose `kb_ids` do not reach the base gets what `review_ops::authorize_write` answers today, 403 `token_kb_out_of_scope`; the two token reads answer 404. The kind-word REST route keeps its plain 403.
- `review_batch` description: say that an action other than `merge` or `keep` is refused as a whole with no code. Keep the words `batch_id`, `batch_size`, `review_revert` and `50`, which `crates/utopia-server/src/api/review_token_batch_tests.rs` (line 1260) pins. No behaviour of the batch changes.
- A defect found by the table is fixed in `crates/utopia-server/src/api/review_ops.rs`, `crates/utopia-server/src/api/review_routes.rs` or `crates/utopia-server/src/api/review_tools.rs` within the interface contracts; a fix that needs another file or another contract is a `NEEDS-ORCHESTRATOR` block.
- Declare `review_token_table_tests` in `crates/utopia-server/src/api/mod.rs`.
- If an existing test outside this task's files fails because of this change, do not edit it: record `NEEDS-ORCHESTRATOR` in the Log with the test name. The only existing-test edits this milestone allows are a lifted token refusal, an exact tool list and a label list (edge E22).
- Conventions (bind this task): Chinese explanatory comments; sentence-named tests; English user-facing strings, each refusal with a stable code; one behaviour-sentence commit subject; DCO sign-off is deferred to the upstream submission; nothing is sent to `deeplethe/utopia` or a deployed instance. Tests seed through store functions or direct inserts, never through model jobs. No migration is written and `CURRENT_SCHEMA_VERSION` stays 115; no `AgentOperation` variant, no `AppError` variant and no item struct field is added. Line numbers in this brief are those of commit `d5d2356`.

## Interface contract

- **Ledger names of this milestone (delivered by T001).** No migration; `token_agent_actions` keeps its columns. A propose-only row has `outcome` `proposed` and `gate_reason` `propose_only`; a finalized row has `outcome` `applied`. Operation and `target_kind`: `fact.confirm`, `fact.reject`, `fact.close` (`fact`); `conflict.resolve` (`conflict`); `violation.decide` (`axiom_violation`); `defect.decide` (`ontology_defect`); `errata.decide` (`errata_action`); `alignment.phrase` (`phrase_binding`); `alignment.rule` (`implication_rule`); `pending.nod` (`pending_fact`); `governor.answer` (`agent_decision`); `mapping.decide` (`concept_mapping`) — each with the row's own id as `target_id` and as `target_key`; `ontology.proposal` (`ontology_proposal`, `target_id` the id of the `ontology_proposals` row, `target_key` the section and the key joined by `:`); `alignment.kind_word` (`type_binding`, `target_id` the base id, `target_key` the normalized kind word); `consistency.check` and `inference.run` (`knowledge_base`, `target_id` and `target_key` the base id). There is one open proposal per base, operation and `target_key`. `payload` keys: `request` (the full request body), `target_label` (words naming the target), `action` (when the operation has one), `why` (the rationale, when given) and `fingerprint` (conflict and ontology proposal only). `action` codes: `confirm`, `reject`, `close` (the three fact operations); `close`, `retime_new`, `keep`, `reject_new` (conflict); `fact_retracted`, `fact_closed`, `axiom_relaxed`, `accepted` (violation); `fixed`, `accepted` (defect); `approve`, `reject` (errata and rule); `bind`, `none` (phrase and kind word); `adopt`, `reject` (ontology proposal); `confirm`, `reject` (pending fact); `merge`, `keep`, `revert` (governor answer); `confirmed`, `rejected` (mapping); the two runs have none.
- **Token propose over REST and the operation layer (delivered by T002, used by every propose-only family).** A handler opened to tokens takes `SessionOrToken` and keeps its `Json` request struct; `if let Some(token) = caller.pat` returns the `review_ops` answer on every path, and the session code below it is unchanged. In `crates/utopia-server/src/api/review_ops.rs`: `pub(crate) async fn target_open(state: &AppState, caller: &ReviewCaller, operation: &str, target_id: Uuid, request: &serde_json::Value) -> Result<(), AppError>` (`Missing` is 404, `Decided` is 409 `already_decided`) and `pub(crate) async fn land_proposal(state: &AppState, caller: &ReviewCaller, row: AgentOperation, operation: &'static str, target_kind: &'static str, target_id: Uuid, target_key: &str, request: serde_json::Value, action: Option<&str>, rationale: Option<&str>) -> Result<serde_json::Value, AppError>`, which lands only when `agent_policy(row, None)` is `AgentOutcome::Proposed`, takes the target's label and fingerprint from the store, and calls neither `guard` nor the breaker. Order in a family function: `authorize_write`; shape checks of the body with the person's codes; `target_open`; the state-dependent checks with the person's codes; `land_proposal`. Answer, HTTP 200: `{"ok":true,"outcome":"proposed","action_id":…,"reason":"propose_only","explanation":…}` plus `"code":"already_proposed"` only when the call met the open proposal, whose id `action_id` then is. Refusals: 403 `token_read_only`, 403 `token_kb_out_of_scope`, 403 `token_owner_below_editor` (checked in that order, as `authorize_write` does today), 404, 409 `already_decided`, and the person's 422 codes. A body that does not deserialize is answered by the `Json` extractor before the scope check, for every caller. A propose-only call is not counted toward the limit and is never answered `agent_rate_limited`.
- **Proposal answer for every operation (delivered by T002; each family task adds its arm).** The two session-only routes `POST /api/v1/kbs/{id}/review/agent-proposals/{proposal_id}` and `POST /api/v1/kbs/{id}/review/agent-proposals` keep `AuthUser`, their bodies and their M003 answers; `review_routes::answer_proposal` gains one arm per operation string and keeps answering 500 for an operation it does not know. Before the person's code runs, approval reads `target_state` with the stored `request` and `fingerprint`: `Missing` or `Decided` closes the proposal `superseded` and changes nothing else. Otherwise the arm runs the person's own code for that operation with the person as actor: each opened handler's session statements are moved unchanged into one function that takes the person and an optional proposal id; the session handler calls it with none and the arm with the id, which only adds `agent_proposal` (the ledger row id) to the audit detail. That audit row never carries `token_id`, and the token's rationale is never passed as the person's reason. An error of the person's code is returned with the person's code and leaves the proposal open; only a typed `NotFound` from a function that is one guarded statement or one transaction (confirm fact, reject fact, defect, conflict, ontology adoption) closes it `superseded`. Single answer, HTTP 200: `{"ok":true,"state":"approved"|"declined"|"superseded"}` plus, each only when set, `merge_id` (as today), `job_id` with `"queued":true` (an approved phrase or rule proposal) and `fact_id` (an approved confirm of a pending fact). A bulk outcome keeps its five keys and gains the same optional keys only when set. Decline and supersede audit rows keep their action names and carry `left` and `right` only when the payload has them.
- **MCP tools of this milestone (file `crates/utopia-server/src/api/review_tools.rs`).** Each new write tool is in `serves` and `writes`, is listed in `tools/list` only for a `write` token whose owner is at least Editor, and a caller without that right that calls one gets a tool error with the REST code (`token_read_only`, `token_owner_below_editor`), not JSON-RPC -32601. Arguments are the route's id and the fields of the handler's request struct; `structuredContent` is the REST body of the matching token call; a refused call is `isError: true` with `{"code","message"}` (`not_found` for a 404). Propose-only tools — `review_defect`, `review_fact`, `review_conflict`, `review_violation`, `review_errata`, `review_alignment`, `review_ontology`, `review_pending_fact`, `review_governor` — are annotated `{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":false}`; each description begins "Propose that a person", says that the call changes nothing, that a signed-in editor answers it in the Review tab, the result shape and `already_proposed` on a repeat, and ends "No tool answers a proposal." Finalize tools — `review_mapping`, `review_kind_word`, `review_check` — are annotated `{"readOnlyHint":false,"destructiveHint":true,"idempotentHint":false}`. The read tools `review_mapping_list` and `review_ontology_list` are annotated exactly `{"readOnlyHint":true}` and listed for every token that reaches the base. No listed name contains `proposal`, `approve`, `decline` or `answer`; `review_confirm`, `review_decide`, `review_pending` and `review_approve` stay unknown tools answered with JSON-RPC -32601. Each tool is delivered by the task that opens its route: `review_defect` by T002; `review_fact` and `review_conflict` by T004; `review_violation`, `review_errata` and `review_alignment` by T005; `review_ontology` and `review_ontology_list` by T006; `review_mapping`, `review_mapping_list`, `review_kind_word` and `review_check` by T007; `review_pending_fact` and `review_governor` by T008.
- **Test support for token writes (delivered by T002).** `crates/utopia-server/src/api/review_token_write_support.rs`, declared `#[cfg(test)] mod review_token_write_support;` in `crates/utopia-server/src/api/mod.rs` and so mounted as `api::review_token_write_support`, holds `pub(super)` helpers that later test modules import and do not edit: a REST call with a bearer token, an MCP `tools/list` and `tools/call`, the check that a tool result is a refusal with a given code, a snapshot of every `facts`, `entities` and `fact_evidence` row of the base with the count of queued jobs, the open proposals and the ledger rows of the base, and the audit rows of the base. It builds on `Fx` of `review_session_unchanged_tests` and `seed_every_section` of `review_totals_tests`.
- **Defect decision (delivered by T002).** `POST /api/v1/kbs/{id}/review/defects/{defect_id}` takes `SessionOrToken` with body `{"resolution":"fixed"|"accepted"}`. A token caller goes to `review_ops::propose_defect(state, caller, defect_id, resolution)` (`pub(crate) async`, `Result<serde_json::Value, AppError>`), policy row `AgentOperation::Defect`, operation `defect.decide`; an unknown resolution is 422 `bad_resolution` and lands nothing. MCP tool `review_defect` takes `id` and `resolution`. Approval runs `reasoning::decide_defect` as the person and writes the audit row `defect.decided` with the person as actor and `agent_proposal` in its detail. A session caller runs today's code and gets today's answer.
- **Fact and conflict proposals (delivered by T004).** `POST /api/v1/kbs/{id}/facts/{fact_id}/confirm`, the same path ending `/reject`, the same path ending `/close` (body `CloseFactBody`) and `POST /api/v1/kbs/{id}/conflicts/{conflict_id}` (body `ConflictBody`) take `SessionOrToken`. A token caller goes to `review_ops::propose_fact(state, caller, fact_id, action, request)` with `action` `confirm`, `reject` or `close` (policy rows `ConfirmFact`, `RejectFact`, `CloseFact`; operations `fact.confirm`, `fact.reject`, `fact.close`) or to `review_ops::propose_conflict(state, caller, conflict_id, request)` (policy row `ResolveConflict`, operation `conflict.resolve`); both are `pub(crate) async` and return `Result<serde_json::Value, AppError>`. The person's refusals are given at propose time with the person's codes: `bad_precision`, `close_at_required`, `interval_inverted`, and the store's refusal of an unknown conflict action. MCP tools `review_fact` (`id`, `action`, and the close fields `valid_to` and `valid_to_precision`) and `review_conflict` (`id` and the fields of `ConflictBody`). Approval runs the person's confirm, reject, close or conflict resolution with the person as actor; a close whose store call reports that it changed nothing is treated as stale and closes the proposal `superseded`, never `approved`.
- **Violation, errata and alignment proposals (delivered by T005).** `POST /api/v1/kbs/{id}/review/violations/{violation_id}` (body `DecideViolationReq`), `POST /api/v1/kbs/{id}/review/errata/{action_id}` (body `DecideErrataReq`), `POST /api/v1/kbs/{id}/review/alignment/phrases/{binding_id}` (body `DecideAlignmentPhraseReq`) and `POST /api/v1/kbs/{id}/review/alignment/rules/{rule_id}` (body `DecideAlignmentRuleReq`) take `SessionOrToken`. Token callers go to `review_ops::propose_violation(state, caller, violation_id, request)`, `review_ops::propose_errata(state, caller, action_id, approve)`, `review_ops::propose_alignment_phrase(state, caller, binding_id, request)` and `review_ops::propose_alignment_rule(state, caller, rule_id, approve)` (policy rows `Violation`, `Errata`, `AlignmentPhrase`, `AlignmentRule`), all `pub(crate) async` returning `Result<serde_json::Value, AppError>`. The two alignment routes answer a token 200 with the proposal body and keep 202 with `job_id` for a session. The person's refusals are given at propose time with the person's codes (`bad_resolution`, `fact_required`, `close_at_required`, `unknown_property` and the phrase handler's other body codes). MCP tools `review_violation`, `review_errata` and `review_alignment` (argument `kind` is `phrase` or `rule`, with `id` and that handler's fields); the description of `review_alignment` adds that an approval queues the change. Approving a phrase or rule proposal answers `{"ok":true,"state":"approved","job_id":…,"queued":true}` and its Agent History row reads `queued`. An error of the person's violation decision or errata answer after a committed step is returned and leaves the proposal open; it is never reported as `superseded`.
- **Ontology proposals: the read and the proposal (delivered by T006).** `GET /api/v1/kbs/{id}/ontology/proposals` takes `SessionOrToken`: a token needs an owner at least Viewer and `kb_ids` that reach the base (404 otherwise) and gets the body a session gets; each proposal row gains `agent_proposals` (the shape of the list items) only while a token proposal on it is open, and no other key. `POST /api/v1/kbs/{id}/ontology/proposals` (body `DecideProposalReq`, reject) and `POST /api/v1/kbs/{id}/ontology/proposals/adopt` (body `AdoptReq`) take `SessionOrToken`; a token caller goes to `review_ops::propose_ontology(state, caller, section, key, action, request)` with `action` `reject` or `adopt` (`pub(crate) async`, `Result<serde_json::Value, AppError>`; policy row `OntologyProposal`, operation `ontology.proposal`, one open proposal per ontology proposal whatever its action). A section and key with no stored proposal is 404; the reject route's 422 `adoption_endpoint` stays. `review_ops::ontology_proposals(state, caller)` is the token read. MCP tools `review_ontology_list` (no argument) and `review_ontology` (`section`, `key`, `action`, and that handler's other fields). Approving a rejection reads the proposal's status first, rejects it as the person and writes one audit row `ontology_proposal.rejected` (`target_kind` `ontology_proposal`, the person as actor, `agent_proposal` in the detail); approving an adoption runs `ontology_agent::adopt` as the person. The person's own reject route still writes no audit row and still answers ok for a missing proposal. On the Ontology page each stored proposal with `agent_proposals` shows `AgentProposalChips`.
- **Finalized token operations (delivered by T007).** `GET /api/v1/kbs/{id}/mappings` takes `SessionOrToken` (a token needs an owner at least Viewer and `kb_ids` that reach the base, 404 otherwise; `review_ops::mappings` is the token read; `mappings` stays `unknown_queue` on the shared list and on `review_list`). `POST /api/v1/kbs/{id}/review/mappings/{mapping_id}`, `POST /api/v1/kbs/{id}/consistency/check` and `POST /api/v1/kbs/{id}/inference/run` take `SessionOrToken`. Token callers go to `review_ops::decide_mapping(state, caller, mapping_id, status)` and `review_ops::run_check(state, caller, run)` with `run` `consistency` or `inference` (policy rows `Mapping` and `ConsistencyRun`). Order: `authorize_write`; `bad_status` or, for inference on a base with the setting off, `inference_off` (no row); the target read (an unknown mapping is 404, a status equal to the mapping's is 409 `already_decided` with no row); the policy row, which must give `AgentOutcome::Applied`; `guard` (HTTP 429 `agent_rate_limited`, nothing changed, no row); the store call. `token_agent_actions::decide_mapping(pool: &PgPool, w: &TokenWrite, mapping_id: Uuid, status: &str) -> AppResult<TokenAgentAction>` writes the mapping's new status, the `applied` ledger row and the audit row `mapping.decided` in one transaction; a run writes its `applied` row (`consistency.check` or `inference.run`) and its audit row (`consistency.checked` or `inference.materialized`) right after the run succeeds, and a failed row fails the call. Every such audit row has the owner as actor and `token_id`, `token_name` and `action_id` in its detail. The answer is the person's body. The kind-word route `POST /api/v1/kbs/{id}/review/alignment/kind-words/{kind_word}` keeps its two checks, its status, its body, `unknown_class`, `alignment_busy` and its plain 403; for a token caller `token_agent_actions::decide_kind_word(pool: &PgPool, w: &TokenWrite, kind_word: &str, type_id: Option<Uuid>, votes: &serde_json::Value) -> AppResult<Option<TokenAgentAction>>` writes the decision and one `applied` `alignment.kind_word` row in one transaction, the audit row `alignment.kind_word_decided` gains `token_name` and `action_id`, and `guard` is never called. MCP tools `review_mapping_list` (`status`, `q`, `limit`, `offset`), `review_mapping` (`id`, `status`), `review_check` (`run`) and `review_kind_word` (`kind_word`, `class`), the last through `review_ops::decide_kind_word(state, caller, kind_word, class)`, which checks `authorize_write` first so a refused caller gets the coded tool error.
- **The two separate token entries (delivered by T008).** `POST /api/v1/kbs/{id}/review/pending/{pending_id}/propose` with body `{"action":"confirm"|"reject"}` and `POST /api/v1/kbs/{id}/review/agent/{decision_id}/propose` with body `{"action":"merge"|"keep"|"revert","rationale":…}` take `SessionOrToken`, read the body as raw bytes and parse it after `authorize_write` (422 `invalid_body`); a caller without a personal token is refused with 403 `token_required` and changes nothing. They go to `review_ops::propose_pending(state, caller, pending_id, action)` (policy row `PendingNod`, operation `pending.nod`; an action other than `confirm` or `reject` is 422 `unknown_action`; an unknown or already nodded pending id is 404) and `review_ops::propose_governor(state, caller, decision_id, action, rationale)` (policy row `AnswerGovernor`, operation `governor.answer`; an action that does not fit the decision's status is 422 `agent_answer`; a decision already answered, or a `proposed` one whose review row is no longer pending, is 409 `already_decided`). The nod route `POST /api/v1/kbs/{id}/review/pending/{pending_id}`, `GET /api/v1/kbs/{id}/review/pending` and the governor's route `POST /api/v1/kbs/{id}/review/agent/{decision_id}` are not edited and keep answering every token 401. MCP tools `review_pending_fact` (`id`, `action`) and `review_governor` (`id`, `action`, `rationale`). Approving a pending-fact proposal records the nod as the person's: audit row `fact.nod_confirmed` (answer carries `fact_id`) or `fact.nod_rejected`, the person as actor, `agent_proposal` in the detail; declining leaves the pending row waiting. Approving a governor proposal runs the person's governor answer as that person with no rationale, no `token_id` key and `agent_proposal` in the detail.
- **Open proposals on list items (delivered by T003).** For session, token and MCP readers alike, every list item of the sections Awaiting your nod, Conflicts, Unconfirmed, Low confidence, Axioms, Ontology defects and Errata, every phrase and rule item of Alignment, and every governor row of Agent History carries `agent_proposals`: an array, oldest first, of `{"id","operation","reason","owner_name","token_name","created_at"}` plus `"action"` when the proposal has one, present only while at least one proposal is open, so an item without one has exactly the keys it has today. The key is attached at the JSON layer by one helper in `crates/utopia-server/src/api/review_ops.rs`, `pub(crate) async fn attach_agent_proposals(state: &AppState, kb_id: Uuid, target_kind: &str, items: &mut [serde_json::Value]) -> Result<(), AppError>`, which reads each item's `id`. Duplicates items and Merges rows keep the M003 key `agent_proposal` unchanged; kind-word items and mapping rows carry no proposal. The `counts` object of every list answer carries `agent_token_proposals` only while a token proposal is open.

## Intent coverage

- SC1
- SC2
- SC6
- SC7
- SC9

## Acceptance criteria

1. E1 (held-out; ruling: nothing changes on a propose-only call means every `facts`, `entities` and `fact_evidence` row of the base, the target's own row and the count of queued jobs are equal before and after the call): this holds for each of the twelve propose-only operations over REST and over MCP, and one proposal appears for the target.
2. E3 (held-out; ruling: a target id that is unknown or belongs to another base is not found; a target that has a decided state and is already decided answers `already_decided`; neither lands a proposal): every operation answers 404 for an unknown and for a foreign id, and each operation with a decided state answers 409 `already_decided`, with no ledger row.
3. E4 (held-out; ruling: the body of a propose-only call is validated when it is proposed, with the same refusals and codes the person's route gives for that body; an invalid body lands no proposal): each operation that has a body refusal returns the status and code a session gets for the same body, and `bad_status` is kept on the mapping decision.
4. E5 (held-out; ruling: proposals and refused calls do not count toward the 200-per-hour limit and a propose-only call is never answered `agent_rate_limited`): with 200 seeded `applied` rows every propose-only operation still answers `proposed`, and after 200 proposals a mapping decision is still applied.
5. E10 (held-out; ruling: one bulk answer takes 1 to 500 proposal ids of any mix of operations and returns one outcome per id in request order; an empty list or 501 ids is refused as a whole): a bulk approval of one proposal of every operation returns an outcome per id in order, and an empty list and 501 ids are refused with nothing changed.
6. E19 (held-out; ruling as corrected by driver D131: for each newly opened operation a read-scope token answers `token_read_only` and a below-editor owner answers `token_owner_below_editor` over REST; over MCP the write tools are not listed for those callers and a call to one returns that coded tool error; the kind-word REST route keeps its plain 403): every row of the ruled table gives a write token of an editor the outcome `agent_policy` states, and each refused caller the stable code, over both surfaces.
7. E20 (held-out; ruling: the token name shown is the snapshot stored with the action; a proposal, its approval, decline or supersession, a mapping decision, a run and a kind-word decision each have an Agent History row and an Activity row reading "owner via token name"): after the token is renamed and revoked each of those rows still carries the owner and the stored token name.
8. E22 (held-out; ruling: the existing session tests of these sections pass unchanged; the only edits to existing tests are assertions that a token is refused on a route this milestone opens, exact tool lists and label lists that gain entries, and every assertion for a route or tool that stays closed keeps its expectation): a session call of every opened route returns the status and body keys it returns at `d5d2356`, and the unedited session, proposal-answer, guard, batch, merge and revert modules pass.
9. No token gets anything but 401 from the two answer routes and no tool answers a proposal; the `review_batch` description states the uncoded refusal of an unknown action and keeps its pinned words.

## Verify

```bash
test -s crates/utopia-server/src/api/review_token_table_tests.rs && docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m004_t009 WITH (FORCE)" -c "CREATE DATABASE m004_t009" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m004_t009 && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- a_propose_only_call_changes_no_row_and_queues_no_job_for_every_operation an_unknown_foreign_or_decided_target_lands_no_proposal_for_every_operation an_invalid_body_gets_the_persons_code_and_lands_no_proposal proposals_and_refused_calls_never_count_toward_the_limit one_bulk_answer_takes_1_to_500_proposals_of_any_mix_of_operations every_operation_of_the_ruled_table_gives_its_outcome_or_a_stable_refusal every_action_reads_owner_via_token_name_in_agent_history_and_activity a_signed_in_persons_decisions_answer_as_before_in_every_opened_section no_token_and_no_tool_answers_a_proposal the_review_batch_description_says_what_an_unknown_action_gets 2>&1 | tee /dev/stderr | grep -c -E "(a_propose_only_call_changes_no_row_and_queues_no_job_for_every_operation|an_unknown_foreign_or_decided_target_lands_no_proposal_for_every_operation|an_invalid_body_gets_the_persons_code_and_lands_no_proposal|proposals_and_refused_calls_never_count_toward_the_limit|one_bulk_answer_takes_1_to_500_proposals_of_any_mix_of_operations|every_operation_of_the_ruled_table_gives_its_outcome_or_a_stable_refusal|every_action_reads_owner_via_token_name_in_agent_history_and_activity|a_signed_in_persons_decisions_answer_as_before_in_every_opened_section|no_token_and_no_tool_answers_a_proposal|the_review_batch_description_says_what_an_unknown_action_gets) \.\.\. ok" | grep -qx 10 && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- review_session_unchanged_tests:: review_proposal_answer_tests:: review_token_reads_tests:: review_mcp_reads_tests:: review_mcp_tests:: review_totals_tests:: review_token_api_tests:: review_token_merge_tests:: review_token_revert_tests:: review_token_batch_tests:: review_token_guard_tests:: review_routes::attribution_tests:: review_routes::conflict_tests:: review_routes::phrase_tests:: review_token_defect_tests:: review_token_fact_tests:: review_token_conflict_tests:: review_token_violation_tests:: review_token_errata_tests:: review_token_alignment_tests:: review_token_mapping_tests:: review_item_proposals_tests:: review_token_ontology_tests:: review_token_run_tests:: review_token_kind_word_tests:: review_token_pending_tests:: review_token_governor_tests:: 2>&1 | tee /dev/stderr | grep -c -E "test result: ok\. [1-9]" | grep -qvx 0
```
<!-- Runs in a fresh sidecar worktree at the landed commit with nothing installed. It creates and migrates its own database(s) on the compose Postgres (`utopia-db-1`, 127.0.0.1:1517). The orchestrator's environment supplies an empty `CARGO_TARGET_DIR` outside the worktree, PostgreSQL 16 client tools and the `tee` shim first on PATH. The leading `test -s` checks name the test files this task owns. -->

Heavy: yes

## Log

- 2026-11-20 — created by planner
- 2026-11-20 — coder build_T009: preflight clean (every path named in Context, Approach and the Interface contract exists at base `1d5e6b7` or is declared in `files`; SC1, SC2, SC6, SC7, SC9 exist in INTENT.md; landed signatures used unchanged; the Verify's only new path is this task's test file). Grep before editing: the only test that reads the `review_batch` description is `review_token_batch_tests.rs` (pins `batch_id`, `batch_size`, `review_revert`, `50`); no test asserts the replaced sentence; no exact tool list, route list or key set changes in this task.
- 2026-11-20 — coder build_T009: implemented.
  - `crates/utopia-server/src/api/review_token_table_tests.rs` (new): one table `Case` (22 REST entries covering the 19 `AgentOperation` rows: ontology proposal has reject and adopt, merge has queue and manual, the run row has consistency and inference). Each entry names its policy row, ledger operation and target kind, target table, REST call, person's route, MCP tool and arguments, seed (direct inserts; `pub(super)` seed helpers of the fact, conflict, violation and errata modules reused unedited), decided variant, unknown target and invalid bodies. `every_row()` holds an exhaustive `match` over `AgentOperation` (no wildcard), so a new variant without a table entry does not compile; the outcome test asserts the table covers every row. Expected outcomes are `agent_policy(row, None)`, never a list in the test. The ten tests the Verify names.
  - `crates/utopia-server/src/api/review_tools.rs`: `review_batch` description now says an empty list or more than 50 ids is refused with `batch_size`, and an action other than merge or keep is refused as a whole with no code, only a message; pinned words kept. No behaviour change.
  - `crates/utopia-server/src/api/mod.rs`: `#[cfg(test)] mod review_token_table_tests;`.
  - `review_ops.rs` and `review_routes.rs`: not changed. The table found no defect: every row held every property on both surfaces, so no landed behaviour was repaired.
- 2026-11-20 — coder build_T009: readings on the known token/person differences (behaviour left as landed; none is required to change by this contract).
  1. Conflict check order (T004): for each single-fault body (`close_at_precision` bad, unknown action `dismiss`, `retime_new` without `new_from`) the token and the session give the same status and code on the same open conflict; the test checks this. Order only differs for a body with two faults, which E4 ("the same refusals and codes the person's route gives for that body") does not reach with a single-fault reading; not changed. MCP trimming `action` is scope-out in INTENT ("MCP trimming of `action` ... left as they are").
  2. `fact_closed` on a violation whose left fact already ended answers `already_decided` (T005): read as a decided target (E3), not a body refusal (E4) — the same `target_state` reading makes approval close such a proposal `superseded`. Changing it would need `review_targets.rs` (outside `files`) or a second state check; not changed.
  3. `review_check` `unknown_run` (T007): MCP-only, stable code, REST cannot reach it; not changed.
  4. `mcp.tool_called` rows (M003): excluded from every audit count in the table tests, as `review_token_batch_tests` does.
  5. E3 excludes the kind word and the two runs: they have no target id (a kind word is upserted, T007 reading 5; a run's target is the base). Batch answers 200 with the member's code (`not_found`, `already_decided`), checked per item.
  6. E20: the approval's Activity row is the person's (actor the person, `agent_proposal`, no `token_id`, per N2); the token name for an approved proposal is read on the proposal's own Agent History and `agent.proposal` rows; decline and supersede rows carry `proposed_by_token`. All checked after the token was renamed and revoked.
  7. E10 also includes the three M003 proposal kinds (held queue merge, held manual merge, proposed revert of a person's merge) in the same bulk approval.
  8. E22's expected keys were read from `git show d5d2356:crates/utopia-server/src/api/review_routes.rs` (consistency includes `defects_new`; my first list missed it and the test failed once until corrected).
- 2026-11-20 — coder build_T009: commands, primary worktree, Git Bash, `CARGO_TARGET_DIR=C:/Users/Owner/.gsd-path/cargo-target/coder-m004`, own database `m004_t009_dev` (created, migrated, "数据库迁移完成"), `UTOPIA_TEST_REQUIRE_DB=1`:
  - `cargo test -p utopia-server -- review_token_table_tests::` → first run 8 passed, 2 failed (test-side: an ontology key over 40 characters, `bad_key`; then the missing `defects_new` key); fixed in the test; final `ok. 10 passed; 0 failed`.
  - The Verify's ten-name command → its `grep -c` counts 10 `... ok`.
  - The Verify's module-filter run (27 filters) → `test result: ok. 148 passed; 0 failed; 829 filtered out`. Each filter matches at least one mounted test (`cargo test -- --list`, e.g. `review_routes::conflict_tests` 1, `review_token_violation_tests` 1).
  - `cargo test -p utopia-server` (whole crate) → `974 passed; 1 failed; 2 ignored` (965 before + 10 new). The failure, `ingest_sources::timestamp_checkpoint_tests::recovery_ignores_old_success_and_retains_history_after_audit_pruning`, is in a module this task did not touch; rerun alone twice → `ok. 1 passed` both times.
  - `cargo fmt --all --check` → exit 0 (after one `cargo fmt --all`, which changed only the new test file).
  - `cargo clippy --workspace --all-targets -- -D warnings` → exit 0.
  - The full Verify command itself was not run (it names the orchestrator's database); each step was run as above against `m004_t009_dev`.
  - Left untested: the Review tab in a browser (final walkthrough); the store crate tests (no store file changed).
  - No deviation from the brief or the interface contract.
- 2026-11-20 — orchestrator Verify (sidecar gsd-path-verify/task-t009-verify): pass, exit 0; output tail:
  ```
  test api::review_totals_tests::the_alignment_low_confidence_and_agent_totals_follow_the_list_while_counts_keep_their_values ... ok
  test api::review_totals_tests::alignment_items_with_one_timestamp_are_each_listed_once_across_pages ... ok
  test api::review_token_revert_tests::review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall ... ok
  test api::review_totals_tests::a_session_list_response_differs_from_before_only_by_the_added_total_key ... ok
  test api::review_token_run_tests::a_token_consistency_check_and_inference_run_each_write_one_applied_row ... ok
  test api::review_token_revert_tests::a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided ... ok
  test api::review_token_reads_tests::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values ... ok
  test api::review_totals_tests::an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total ... ok
  test api::review_token_run_tests::the_call_after_the_200th_finalized_write_answers_429_and_changes_nothing ... ok
  test api::review_totals_tests::the_seed_puts_items_in_every_one_of_the_thirteen_sections ... ok
  test api::review_token_revert_tests::a_token_reverts_any_token_agents_merge_of_the_base_and_an_unknown_or_foreign_merge_id_is_not_found ... ok
  test api::review_token_reads_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read ... ok
  test api::review_token_revert_tests::after_a_token_merge_and_its_revert_every_fact_entity_and_evidence_row_is_as_before ... ok
  test api::review_totals_tests::the_total_of_every_list_queue_is_the_number_of_rows_its_pages_return ... ok
  test api::review_token_merge_tests::of_a_person_the_governor_and_a_token_deciding_one_pair_exactly_one_is_applied_and_the_losing_token_gets_already_decided ... ok
  test api::review_token_violation_tests::a_token_violation_decision_waits_as_a_proposal_and_a_person_applies_it ... ok
  test api::review_token_reads_tests::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items ... ok
  
  test result: ok. 148 passed; 0 failed; 0 ignored; 0 measured; 829 filtered out; finished in 31.83s
  ```
- 2026-11-20 — orchestrator regression Verify of T002 (sidecar gsd-path-verify/task-t009-verify): pass, exit 0; output tail:
  ```
  test api::review_token_revert_tests::a_token_revert_of_a_persons_or_the_governors_merge_lands_a_proposal_and_reverts_nothing ... ok
  test api::review_totals_tests::alignment_items_with_one_timestamp_are_each_listed_once_across_pages ... ok
  test api::review_totals_tests::a_token_reading_duplicates_receives_the_same_total_as_a_session ... ok
  test api::review_totals_tests::the_alignment_low_confidence_and_agent_totals_follow_the_list_while_counts_keep_their_values ... ok
  test api::review_totals_tests::an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total ... ok
  test api::review_token_revert_tests::review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall ... ok
  test api::review_token_revert_tests::a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided ... ok
  test api::review_token_merge_tests::each_of_the_six_hold_reasons_lands_a_proposal_that_names_it_for_the_queue_merge_and_the_manual_merge ... ok
  test api::review_token_reads_tests::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values ... ok
  test api::review_totals_tests::the_seed_puts_items_in_every_one_of_the_thirteen_sections ... ok
  test api::review_totals_tests::the_total_of_every_list_queue_is_the_number_of_rows_its_pages_return ... ok
  test api::review_token_revert_tests::a_token_reverts_any_token_agents_merge_of_the_base_and_an_unknown_or_foreign_merge_id_is_not_found ... ok
  test api::review_token_revert_tests::after_a_token_merge_and_its_revert_every_fact_entity_and_evidence_row_is_as_before ... ok
  test api::review_token_reads_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read ... ok
  test api::review_token_merge_tests::of_a_person_the_governor_and_a_token_deciding_one_pair_exactly_one_is_applied_and_the_losing_token_gets_already_decided ... ok
  test api::review_token_violation_tests::a_token_violation_decision_waits_as_a_proposal_and_a_person_applies_it ... ok
  test api::review_token_reads_tests::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items ... ok
  
  test result: ok. 124 passed; 0 failed; 0 ignored; 0 measured; 853 filtered out; finished in 28.47s
  ```
- 2026-11-20 — orchestrator regression Verify of T003 (sidecar gsd-path-verify/task-t009-verify): pass, exit 0; output tail:
  ```
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:31008) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:18480) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:35060) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:4304) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:21272) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:40912) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:28792) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  [33m
  (!) Some chunks are larger than 500 kB after minification. Consider:
  - Using dynamic import() to code-split the application
  - Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
  - Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.[39m
  ```
- 2026-11-20 — orchestrator regression Verify of T004 (sidecar gsd-path-verify/task-t009-verify): pass, exit 0; output tail:
  ```
  test api::review_totals_tests::a_session_list_response_differs_from_before_only_by_the_added_total_key ... ok
  test api::review_token_reads_tests::a_token_receives_each_sections_items_in_the_session_order ... ok
  test api::review_totals_tests::the_alignment_low_confidence_and_agent_totals_follow_the_list_while_counts_keep_their_values ... ok
  test api::review_totals_tests::alignment_items_with_one_timestamp_are_each_listed_once_across_pages ... ok
  test api::review_token_revert_tests::review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall ... ok
  test api::review_token_revert_tests::a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided ... ok
  test api::review_totals_tests::an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total ... ok
  test api::review_token_reads_tests::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values ... ok
  test api::review_token_merge_tests::each_of_the_six_hold_reasons_lands_a_proposal_that_names_it_for_the_queue_merge_and_the_manual_merge ... ok
  test api::review_totals_tests::the_seed_puts_items_in_every_one_of_the_thirteen_sections ... ok
  test api::review_token_revert_tests::a_token_reverts_any_token_agents_merge_of_the_base_and_an_unknown_or_foreign_merge_id_is_not_found ... ok
  test api::review_totals_tests::the_total_of_every_list_queue_is_the_number_of_rows_its_pages_return ... ok
  test api::review_token_revert_tests::after_a_token_merge_and_its_revert_every_fact_entity_and_evidence_row_is_as_before ... ok
  test api::review_token_reads_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read ... ok
  test api::review_token_merge_tests::of_a_person_the_governor_and_a_token_deciding_one_pair_exactly_one_is_applied_and_the_losing_token_gets_already_decided ... ok
  test api::review_token_violation_tests::a_token_violation_decision_waits_as_a_proposal_and_a_person_applies_it ... ok
  test api::review_token_reads_tests::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items ... ok
  
  test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 850 filtered out; finished in 29.68s
  ```
- 2026-11-20 — orchestrator regression Verify of T005 (sidecar gsd-path-verify/task-t009-verify): pass, exit 0; output tail:
  ```
  test api::review_totals_tests::a_session_list_response_differs_from_before_only_by_the_added_total_key ... ok
  test api::review_totals_tests::alignment_items_with_one_timestamp_are_each_listed_once_across_pages ... ok
  test api::review_totals_tests::the_alignment_low_confidence_and_agent_totals_follow_the_list_while_counts_keep_their_values ... ok
  test api::review_token_reads_tests::a_token_receives_each_sections_items_in_the_session_order ... ok
  test api::review_totals_tests::an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total ... ok
  test api::review_token_revert_tests::review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall ... ok
  test api::review_token_revert_tests::a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided ... ok
  test api::review_totals_tests::the_seed_puts_items_in_every_one_of_the_thirteen_sections ... ok
  test api::review_token_reads_tests::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values ... ok
  test api::review_token_merge_tests::each_of_the_six_hold_reasons_lands_a_proposal_that_names_it_for_the_queue_merge_and_the_manual_merge ... ok
  test api::review_token_revert_tests::a_token_reverts_any_token_agents_merge_of_the_base_and_an_unknown_or_foreign_merge_id_is_not_found ... ok
  test api::review_totals_tests::the_total_of_every_list_queue_is_the_number_of_rows_its_pages_return ... ok
  test api::review_token_revert_tests::after_a_token_merge_and_its_revert_every_fact_entity_and_evidence_row_is_as_before ... ok
  test api::review_token_reads_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read ... ok
  test api::review_token_merge_tests::of_a_person_the_governor_and_a_token_deciding_one_pair_exactly_one_is_applied_and_the_losing_token_gets_already_decided ... ok
  test api::review_token_violation_tests::a_token_violation_decision_waits_as_a_proposal_and_a_person_applies_it ... ok
  test api::review_token_reads_tests::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items ... ok
  
  test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 850 filtered out; finished in 29.85s
  ```
- 2026-11-20 — orchestrator regression Verify of T006 (sidecar gsd-path-verify/task-t009-verify): pass, exit 0; output tail:
  ```
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:34848) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:29452) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:35256) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:28792) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:1976) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:40912) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:10544) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  [33m
  (!) Some chunks are larger than 500 kB after minification. Consider:
  - Using dynamic import() to code-split the application
  - Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
  - Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.[39m
  ```
- 2026-11-20 — orchestrator regression Verify of T007 (sidecar gsd-path-verify/task-t009-verify): pass, exit 0; output tail:
  ```
  test api::review_token_reads_tests::a_token_receives_each_sections_items_in_the_session_order ... ok
  test api::review_totals_tests::the_alignment_low_confidence_and_agent_totals_follow_the_list_while_counts_keep_their_values ... ok
  test api::review_totals_tests::a_session_list_response_differs_from_before_only_by_the_added_total_key ... ok
  test api::review_token_revert_tests::a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided ... ok
  test api::review_token_run_tests::a_token_consistency_check_and_inference_run_each_write_one_applied_row ... ok
  test api::review_token_revert_tests::review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall ... ok
  test api::review_token_reads_tests::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values ... ok
  test api::review_totals_tests::an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total ... ok
  test api::review_token_revert_tests::a_token_reverts_any_token_agents_merge_of_the_base_and_an_unknown_or_foreign_merge_id_is_not_found ... ok
  test api::review_token_run_tests::the_call_after_the_200th_finalized_write_answers_429_and_changes_nothing ... ok
  test api::review_token_revert_tests::after_a_token_merge_and_its_revert_every_fact_entity_and_evidence_row_is_as_before ... ok
  test api::review_totals_tests::the_seed_puts_items_in_every_one_of_the_thirteen_sections ... ok
  test api::review_totals_tests::the_total_of_every_list_queue_is_the_number_of_rows_its_pages_return ... ok
  test api::review_token_reads_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read ... ok
  test api::review_token_merge_tests::of_a_person_the_governor_and_a_token_deciding_one_pair_exactly_one_is_applied_and_the_losing_token_gets_already_decided ... ok
  test api::review_token_violation_tests::a_token_violation_decision_waits_as_a_proposal_and_a_person_applies_it ... ok
  test api::review_token_reads_tests::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items ... ok
  
  test result: ok. 140 passed; 0 failed; 0 ignored; 0 measured; 837 filtered out; finished in 31.26s
  ```
