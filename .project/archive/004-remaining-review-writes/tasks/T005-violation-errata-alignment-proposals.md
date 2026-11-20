---
id: T005
title: Open violation, errata and alignment phrase and rule decisions to a token as proposals
wave: 2
deps: [T004]
status: done
agent: build_T005
base: dd579e3bef2b7f0bda0385562f5cf311fc7a15bd
worktree: null
task_branch: null
files:
  - crates/utopia-server/src/api/review_ops.rs
  - crates/utopia-server/src/api/review_routes.rs
  - crates/utopia-server/src/api/review_tools.rs
  - crates/utopia-server/src/api/review_token_violation_tests.rs
  - crates/utopia-server/src/api/review_token_errata_tests.rs
  - crates/utopia-server/src/api/review_token_alignment_tests.rs
  - crates/utopia-server/src/api/review_mcp_reads_tests.rs
---

# T005 — Open violation, errata and alignment phrase and rule decisions to a token as proposals

## Context

This task adds four more propose-only operations on the T002 path. The M003 patterns to follow are in `crates/utopia-server/src/api/review_routes.rs` and `crates/utopia-server/src/api/review_ops.rs`: the token branch of `review_routes::decide` (lines 353 to 374), `review_ops::authorize_write` (lines 69 to 84), `review_ops::keep` (lines 361 to 388), `review_ops::guard` (lines 317 to 344) and the answer dispatch `review_routes::answer_proposal` (lines 850 to 918). The session handlers are `decide_violation` (`crates/utopia-server/src/api/review_routes.rs` line 1528, body `DecideViolationReq` line 1505), `decide_errata` (line 2155), `decide_alignment_rule` (line 1935) and `decide_alignment_phrase` (line 1973, body `DecideAlignmentPhraseReq` line 1912). The two alignment handlers answer a person 202 with a `job_id`, because the decision and its delivery job are one commit and the change itself is applied later by the job. A phrase binding and a rule have no decided state. The violation decision and the errata answer are not one transaction.

## Approach

- For each handler: `SessionOrToken`, the token branch, the session statements moved unchanged into one function taking the person and an optional proposal id, and one `answer_proposal` arm.
- The two alignment routes return different statuses for the two callers: 200 with the proposal body for a token, 202 with `job_id` for a session. Keep the handler's return type able to do both without changing the session answer.
- N6 (judgment): an approved phrase or rule proposal is never worded as applied while its job is only queued. The answer carries `job_id` and `queued` true, the Agent History row reads `queued` (delivered by T001 and T003), and the description of `review_alignment` says that an approval queues the change.
- Body checks at propose time with the person's codes: `bad_resolution`, `fact_required`, `close_at_required` for a violation; `unknown_property` and the phrase handler's other body codes. A check that reads the ontology or the violation's facts runs after `target_open`.
- An error from the person's violation decision or errata answer at approval is returned as it is and the proposal stays open; do not map it to `superseded`.
- Replace `a_token_is_still_refused_on_the_violation_route`, `..._errata_route` and `..._alignment_rule_route` with the new behaviour.
- N2 (judgment): each approval has the person as actor and names the proposal. N3 (judgment): no new entry answers a proposal.
- MCP: follow the M003 write tools in `crates/utopia-server/src/api/review_tools.rs` (`serves` line 39, `writes` line 44, `definitions` line 70, `annotations` line 358, `listed` line 378, `bad_argument` line 403, `call` line 487). `crates/utopia-server/src/api/mcp.rs` calls only `listed`, `serves` and `call` and is not edited. Add each new tool name to the expected lists and to the `writes` expression of `the_mcp_endpoint_offers_exactly_the_review_tools_the_ruled_table_opens` in `crates/utopia-server/src/api/review_mcp_reads_tests.rs` (lists at lines 1050 to 1073, expression at lines 1087 to 1090); every other assertion of that test, the unknown names at lines 1180 to 1199 included, keeps its expectation.
- If an existing test outside this task's files fails because of this change, do not edit it: record `NEEDS-ORCHESTRATOR` in the Log with the test name. The only existing-test edits this milestone allows are a lifted token refusal, an exact tool list and a label list (edge E22).
- Conventions (bind this task): Chinese explanatory comments; sentence-named tests; English user-facing strings, each refusal with a stable code; one behaviour-sentence commit subject; DCO sign-off is deferred to the upstream submission; nothing is sent to `deeplethe/utopia` or a deployed instance. Tests seed through store functions or direct inserts, never through model jobs. No migration is written and `CURRENT_SCHEMA_VERSION` stays 115; no `AgentOperation` variant, no `AppError` variant and no item struct field is added. Line numbers in this brief are those of commit `d5d2356`.

## Interface contract

- **Typed target state (delivered by T001).** New module `crates/utopia-store/src/review_targets.rs`: `pub enum TargetState { Open, Decided, Missing }`; `pub async fn target_state(pool: &PgPool, kb_id: Uuid, operation: &str, target_id: Uuid, request: &serde_json::Value, fingerprint: Option<&serde_json::Value>) -> AppResult<TargetState>`; `pub async fn fingerprint(pool: &PgPool, kb_id: Uuid, operation: &str, target_id: Uuid) -> AppResult<Option<serde_json::Value>>`; `pub async fn target_label(pool: &PgPool, kb_id: Uuid, operation: &str, target_id: Uuid, request: &serde_json::Value) -> AppResult<String>`; `pub async fn ontology_proposal_id(pool: &PgPool, kb_id: Uuid, section: &str, key: &str) -> AppResult<Option<Uuid>>`. Every read is scoped by base, so an id of another base is `Missing`, and no error text is matched. Rules: `fact.confirm` — a missing or invalidated fact is `Missing` and there is no `Decided`; `fact.reject` and `fact.close` — an invalidated fact is `Decided`, and for `fact.close` a live fact that already has an end is `Missing`; `conflict.resolve` — `Open` while the conflict's status is open and both facts are live, `Decided` otherwise, and `Decided` when a fingerprint is given and the conflict's two fact ids differ from it; `violation.decide` — `Open` while the violation's status is open and, for the resolutions `fact_retracted` and `fact_closed`, the fact the request names is live and has no end, `Decided` otherwise; `defect.decide` — `Open` while the defect's status is open; `errata.decide` — `Open` while the action's status is held; `ontology.proposal` — `Open` while the proposal's status is open and the ontology agent proposed it, and `Decided` when a fingerprint is given and the stored payload differs from it; `alignment.phrase`, `alignment.rule` and `pending.nod` — `Open` while the row exists and `Missing` otherwise, with no `Decided`; `governor.answer` — `Open` when the decision is `proposed` and its review row is still pending, or the decision is `applied`, and `Decided` otherwise; `mapping.decide` — `Decided` when the mapping's status equals the requested status and `Open` otherwise. `fingerprint` returns the conflict's two fact ids or the ontology proposal's stored payload, and nothing for any other operation.
- **Agent History row and the badge count (delivered by T001).** A token ledger row in Agent History keeps today's keys and gains, each only when it has a value: `target_label` and `action` (from the payload) and `queued` (`true` on an `approved` `alignment.phrase` or `alignment.rule` row), so the row of a keep or a merge has exactly the keys it has today. `review::counts` gains `agent_token_proposals`, the number of open token proposals of the base, serialized only when it is above zero; `agent` and every other count keep their values.
- **Token propose over REST and the operation layer (delivered by T002, used by every propose-only family).** A handler opened to tokens takes `SessionOrToken` and keeps its `Json` request struct; `if let Some(token) = caller.pat` returns the `review_ops` answer on every path, and the session code below it is unchanged. In `crates/utopia-server/src/api/review_ops.rs`: `pub(crate) async fn target_open(state: &AppState, caller: &ReviewCaller, operation: &str, target_id: Uuid, request: &serde_json::Value) -> Result<(), AppError>` (`Missing` is 404, `Decided` is 409 `already_decided`) and `pub(crate) async fn land_proposal(state: &AppState, caller: &ReviewCaller, row: AgentOperation, operation: &'static str, target_kind: &'static str, target_id: Uuid, target_key: &str, request: serde_json::Value, action: Option<&str>, rationale: Option<&str>) -> Result<serde_json::Value, AppError>`, which lands only when `agent_policy(row, None)` is `AgentOutcome::Proposed`, takes the target's label and fingerprint from the store, and calls neither `guard` nor the breaker. Order in a family function: `authorize_write`; shape checks of the body with the person's codes; `target_open`; the state-dependent checks with the person's codes; `land_proposal`. Answer, HTTP 200: `{"ok":true,"outcome":"proposed","action_id":…,"reason":"propose_only","explanation":…}` plus `"code":"already_proposed"` only when the call met the open proposal, whose id `action_id` then is. Refusals: 403 `token_read_only`, 403 `token_kb_out_of_scope`, 403 `token_owner_below_editor` (checked in that order, as `authorize_write` does today), 404, 409 `already_decided`, and the person's 422 codes. A body that does not deserialize is answered by the `Json` extractor before the scope check, for every caller. A propose-only call is not counted toward the limit and is never answered `agent_rate_limited`.
- **Proposal answer for every operation (delivered by T002; each family task adds its arm).** The two session-only routes `POST /api/v1/kbs/{id}/review/agent-proposals/{proposal_id}` and `POST /api/v1/kbs/{id}/review/agent-proposals` keep `AuthUser`, their bodies and their M003 answers; `review_routes::answer_proposal` gains one arm per operation string and keeps answering 500 for an operation it does not know. Before the person's code runs, approval reads `target_state` with the stored `request` and `fingerprint`: `Missing` or `Decided` closes the proposal `superseded` and changes nothing else. Otherwise the arm runs the person's own code for that operation with the person as actor: each opened handler's session statements are moved unchanged into one function that takes the person and an optional proposal id; the session handler calls it with none and the arm with the id, which only adds `agent_proposal` (the ledger row id) to the audit detail. That audit row never carries `token_id`, and the token's rationale is never passed as the person's reason. An error of the person's code is returned with the person's code and leaves the proposal open; only a typed `NotFound` from a function that is one guarded statement or one transaction (confirm fact, reject fact, defect, conflict, ontology adoption) closes it `superseded`. Single answer, HTTP 200: `{"ok":true,"state":"approved"|"declined"|"superseded"}` plus, each only when set, `merge_id` (as today), `job_id` with `"queued":true` (an approved phrase or rule proposal) and `fact_id` (an approved confirm of a pending fact). A bulk outcome keeps its five keys and gains the same optional keys only when set. Decline and supersede audit rows keep their action names and carry `left` and `right` only when the payload has them.
- **MCP tools of this milestone (file `crates/utopia-server/src/api/review_tools.rs`).** Each new write tool is in `serves` and `writes`, is listed in `tools/list` only for a `write` token whose owner is at least Editor, and a caller without that right that calls one gets a tool error with the REST code (`token_read_only`, `token_owner_below_editor`), not JSON-RPC -32601. Arguments are the route's id and the fields of the handler's request struct; `structuredContent` is the REST body of the matching token call; a refused call is `isError: true` with `{"code","message"}` (`not_found` for a 404). Propose-only tools — `review_defect`, `review_fact`, `review_conflict`, `review_violation`, `review_errata`, `review_alignment`, `review_ontology`, `review_pending_fact`, `review_governor` — are annotated `{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":false}`; each description begins "Propose that a person", says that the call changes nothing, that a signed-in editor answers it in the Review tab, the result shape and `already_proposed` on a repeat, and ends "No tool answers a proposal." Finalize tools — `review_mapping`, `review_kind_word`, `review_check` — are annotated `{"readOnlyHint":false,"destructiveHint":true,"idempotentHint":false}`. The read tools `review_mapping_list` and `review_ontology_list` are annotated exactly `{"readOnlyHint":true}` and listed for every token that reaches the base. No listed name contains `proposal`, `approve`, `decline` or `answer`; `review_confirm`, `review_decide`, `review_pending` and `review_approve` stay unknown tools answered with JSON-RPC -32601. Each tool is delivered by the task that opens its route: `review_defect` by T002; `review_fact` and `review_conflict` by T004; `review_violation`, `review_errata` and `review_alignment` by T005; `review_ontology` and `review_ontology_list` by T006; `review_mapping`, `review_mapping_list`, `review_kind_word` and `review_check` by T007; `review_pending_fact` and `review_governor` by T008.
- **Test support for token writes (delivered by T002).** `crates/utopia-server/src/api/review_token_write_support.rs`, declared `#[cfg(test)] mod review_token_write_support;` in `crates/utopia-server/src/api/mod.rs` and so mounted as `api::review_token_write_support`, holds `pub(super)` helpers that later test modules import and do not edit: a REST call with a bearer token, an MCP `tools/list` and `tools/call`, the check that a tool result is a refusal with a given code, a snapshot of every `facts`, `entities` and `fact_evidence` row of the base with the count of queued jobs, the open proposals and the ledger rows of the base, and the audit rows of the base. It builds on `Fx` of `review_session_unchanged_tests` and `seed_every_section` of `review_totals_tests`.
- **Violation, errata and alignment proposals (delivered by T005).** `POST /api/v1/kbs/{id}/review/violations/{violation_id}` (body `DecideViolationReq`), `POST /api/v1/kbs/{id}/review/errata/{action_id}` (body `DecideErrataReq`), `POST /api/v1/kbs/{id}/review/alignment/phrases/{binding_id}` (body `DecideAlignmentPhraseReq`) and `POST /api/v1/kbs/{id}/review/alignment/rules/{rule_id}` (body `DecideAlignmentRuleReq`) take `SessionOrToken`. Token callers go to `review_ops::propose_violation(state, caller, violation_id, request)`, `review_ops::propose_errata(state, caller, action_id, approve)`, `review_ops::propose_alignment_phrase(state, caller, binding_id, request)` and `review_ops::propose_alignment_rule(state, caller, rule_id, approve)` (policy rows `Violation`, `Errata`, `AlignmentPhrase`, `AlignmentRule`), all `pub(crate) async` returning `Result<serde_json::Value, AppError>`. The two alignment routes answer a token 200 with the proposal body and keep 202 with `job_id` for a session. The person's refusals are given at propose time with the person's codes (`bad_resolution`, `fact_required`, `close_at_required`, `unknown_property` and the phrase handler's other body codes). MCP tools `review_violation`, `review_errata` and `review_alignment` (argument `kind` is `phrase` or `rule`, with `id` and that handler's fields); the description of `review_alignment` adds that an approval queues the change. Approving a phrase or rule proposal answers `{"ok":true,"state":"approved","job_id":…,"queued":true}` and its Agent History row reads `queued`. An error of the person's violation decision or errata answer after a committed step is returned and leaves the proposal open; it is never reported as `superseded`.

## Intent coverage

- SC1
- SC2

## Acceptance criteria

1. Over REST and over `review_violation`, `review_errata` and `review_alignment`, each decision by a write token of an editor answers `proposed` with HTTP 200, changes no fact, entity, target row or queued job, and leaves one open proposal.
2. A signed-in editor approves a violation proposal and an errata proposal and the base changes exactly as that person's own decision changes it, with the person as actor and the proposal named; a declined proposal changes nothing.
3. E9 (held-out; ruling: approving an alignment phrase or rule proposal answers with the queued job's id, and the proposal's row says the change is queued, not that it is applied): the approval answer carries `job_id` and `queued` true and the Agent History row of that proposal carries `queued` true.
4. A session phrase or rule decision still answers 202 with its `job_id` and today's audit row.
5. A violation proposal on a violation decided meanwhile and an errata proposal on an action no longer held answer `superseded`; a phrase or rule proposal applies while its row exists.
6. An invalid body gets the person's code and lands nothing, and the three tools are listed and refused as the write tools are.

## Verify

```bash
test -s crates/utopia-server/src/api/review_token_violation_tests.rs && test -s crates/utopia-server/src/api/review_token_errata_tests.rs && test -s crates/utopia-server/src/api/review_token_alignment_tests.rs && docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m004_t005 WITH (FORCE)" -c "CREATE DATABASE m004_t005" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m004_t005 && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- a_token_violation_decision_waits_as_a_proposal_and_a_person_applies_it a_token_errata_answer_waits_as_a_proposal_and_a_person_applies_it a_token_phrase_and_rule_decision_wait_as_proposals_answered_200 approving_an_alignment_proposal_answers_the_job_id_and_its_row_reads_queued a_session_alignment_decision_still_answers_202_with_its_job_id a_violation_or_errata_proposal_on_a_decided_target_is_superseded the_mcp_violation_errata_and_alignment_tools_propose_and_change_nothing the_mcp_endpoint_offers_exactly_the_review_tools_the_ruled_table_opens 2>&1 | tee /dev/stderr | grep -c -E "(a_token_violation_decision_waits_as_a_proposal_and_a_person_applies_it|a_token_errata_answer_waits_as_a_proposal_and_a_person_applies_it|a_token_phrase_and_rule_decision_wait_as_proposals_answered_200|approving_an_alignment_proposal_answers_the_job_id_and_its_row_reads_queued|a_session_alignment_decision_still_answers_202_with_its_job_id|a_violation_or_errata_proposal_on_a_decided_target_is_superseded|the_mcp_violation_errata_and_alignment_tools_propose_and_change_nothing|the_mcp_endpoint_offers_exactly_the_review_tools_the_ruled_table_opens) \.\.\. ok" | grep -qx 8 && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- review_session_unchanged_tests:: review_proposal_answer_tests:: review_token_reads_tests:: review_mcp_reads_tests:: review_mcp_tests:: review_totals_tests:: review_token_api_tests:: review_token_merge_tests:: review_token_revert_tests:: review_token_batch_tests:: review_token_guard_tests:: review_routes::attribution_tests:: review_routes::conflict_tests:: review_routes::phrase_tests:: review_token_defect_tests:: review_token_fact_tests:: review_token_conflict_tests:: review_token_violation_tests:: review_token_errata_tests:: review_token_alignment_tests:: review_token_mapping_tests:: review_item_proposals_tests:: 2>&1 | tee /dev/stderr | grep -c -E "test result: ok\. [1-9]" | grep -qvx 0
```
<!-- Runs in a fresh sidecar worktree at the landed commit with nothing installed. It creates and migrates its own database(s) on the compose Postgres (`utopia-db-1`, 127.0.0.1:1517). The orchestrator's environment supplies an empty `CARGO_TARGET_DIR` outside the worktree, PostgreSQL 16 client tools and the `tee` shim first on PATH. The leading `test -s` checks name the test files this task owns. -->

Heavy: yes

## Log

- 2026-11-20 — created by planner
- 2026-11-20 — coder build_T005, preflight: base dd579e3, serial round in the primary worktree. Early check: all four handlers write their audit rows in `review_routes.rs` (no `audit::record` in store `reasoning`, `errata`, `implication_rules`, `phrase_bindings`), and both `decide_with_delivery` functions return the job id, so naming the proposal (E7/N2) and reporting the queued job (E9/N6) fit inside `files`. No NEEDS-ORCHESTRATOR.
- 2026-11-20 — built:
  - `review_routes.rs`: `decide_violation`, `decide_errata`, `decide_alignment_phrase`, `decide_alignment_rule` take `SessionOrToken`; the token branch returns before any session code (200 with the proposal). The session bodies are moved unchanged into `violation_decision`, `errata_decision`, `alignment_phrase_decision`, `alignment_rule_decision` (`proposal: Option<Uuid>`; `Some` adds `agent_proposal` to each audit detail it writes, incl. the violation's `fact.reject` / `fact.close` rows). Phrase body validation moved unchanged into `phrase_choice`, shared with the token branch. `Decision` / `decision_of` / `approve_decision` gain the four operations; phrase and rule approvals return `Applied { job_id }`, so the answer carries `job_id` and `queued: true`.
  - `review_ops.rs`: `propose_violation`, `propose_errata`, `propose_alignment_phrase`, `propose_alignment_rule` (authorize_write, body, `target_open`, state-dependent checks, `land_proposal`). Actions: violation = the resolution; errata and rule = `approve` / `reject`; phrase = `bind` / `none`.
  - `review_tools.rs`: tools `review_violation`, `review_errata`, `review_alignment` (`kind` = `phrase` | `rule`), listed, annotated and refused as the other propose-only tools.
  - `review_mcp_reads_tests.rs`: the three names added to the editor write token's expected list and to the non-read-only set; nothing else.
  - Tests (three placeholder tests replaced by seven): violation 1, errata 2, alignment 4. Seeded by direct inserts and store `reasoning::run`; no model job.
- 2026-11-20 — readings taken:
  - Token-branch violation refusals carry the person's codes (`bad_resolution`, `fact_required`, `close_at_required`, `bad_precision`, `interval_inverted`) with new English messages; the session keeps its existing text. Phrase refusals are the person's code and message verbatim (same function).
  - Phrase body checks run after `target_open` (they read the ontology), so an unknown id answers 404 before a body error, as the session route does. `bad_precision` for a violation is checked only on the derived `fact_closed` path, as the person's route does.
  - A `fact_retracted` / `fact_closed` proposal on a fact already retracted or ended answers `already_decided` (T001 target state), where the session answers `not_open`.
  - At approval, any error from the person's violation, errata, phrase or rule code (NotFound included) is returned as it is and the proposal stays open; `superseded` comes only from the target-state check. Reason: violation and errata decisions are not one transaction.
  - Tool argument errors with no REST equivalent (unknown `kind`, non-boolean or missing `approve`) carry no code.
  - Landing a proposal and each MCP tool call write their own audit rows (existing behaviour); "changes nothing" in the tests is asserted on the graph snapshot, the target row, queued jobs and the decision audit actions.
- 2026-11-20 — commands (own database `m004_t005_dev`, `UTOPIA_TEST_REQUIRE_DB=1`):
  - `cargo run -p utopia-store --example migrate` — ok.
  - `cargo test -p utopia-server -- <the 8 Verify names>` — 8 passed, 0 failed.
  - `cargo test -p utopia-server -- <the 22 Verify module filters>` — 125 passed, 0 failed. Every filter matches at least one mounted test (`--list`: violation 1, errata 2, alignment 4; smallest others 1).
  - `cargo test -p utopia-server` — 942 passed, 0 failed, 2 ignored (938 before; +7 new, −3 placeholders).
  - `cargo fmt --all --check` — exit 0 (after `cargo fmt --all`, which touched only this task's files).
  - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0.
  - The literal Verify command (database `m004_t005`) was not run by the coder, per the brief.
- 2026-11-20 — deviations: none from `files` or the interface contract. Not tested: concurrent approvals of one alignment proposal; an errata `revise` / `add` approval (only `retract` is seeded); the web surface (none in this task).
- 2026-11-20 — orchestrator Verify (sidecar gsd-path-verify/task-t005-verify): pass, exit 0; output tail:
  ```
  test api::review_totals_tests::a_session_list_response_differs_from_before_only_by_the_added_total_key ... ok
  test api::review_totals_tests::alignment_items_with_one_timestamp_are_each_listed_once_across_pages ... ok
  test api::review_totals_tests::a_token_reading_duplicates_receives_the_same_total_as_a_session ... ok
  test api::review_token_reads_tests::a_token_receives_each_sections_items_in_the_session_order ... ok
  test api::review_token_revert_tests::a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided ... ok
  test api::review_token_merge_tests::each_of_the_six_hold_reasons_lands_a_proposal_that_names_it_for_the_queue_merge_and_the_manual_merge ... ok
  test api::review_token_revert_tests::review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall ... ok
  test api::review_totals_tests::an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total ... ok
  test api::review_totals_tests::the_seed_puts_items_in_every_one_of_the_thirteen_sections ... ok
  test api::review_token_reads_tests::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values ... ok
  test api::review_token_revert_tests::a_token_reverts_any_token_agents_merge_of_the_base_and_an_unknown_or_foreign_merge_id_is_not_found ... ok
  test api::review_totals_tests::the_total_of_every_list_queue_is_the_number_of_rows_its_pages_return ... ok
  test api::review_token_revert_tests::after_a_token_merge_and_its_revert_every_fact_entity_and_evidence_row_is_as_before ... ok
  test api::review_token_reads_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read ... ok
  test api::review_token_merge_tests::of_a_person_the_governor_and_a_token_deciding_one_pair_exactly_one_is_applied_and_the_losing_token_gets_already_decided ... ok
  test api::review_token_violation_tests::a_token_violation_decision_waits_as_a_proposal_and_a_person_applies_it ... ok
  test api::review_token_reads_tests::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items ... ok
  
  test result: ok. 125 passed; 0 failed; 0 ignored; 0 measured; 819 filtered out; finished in 28.30s
  ```
- 2026-11-20 — orchestrator regression Verify of T002 (sidecar gsd-path-verify/task-t005-verify): pass, exit 0; output tail:
  ```
  test api::review_totals_tests::the_alignment_low_confidence_and_agent_totals_follow_the_list_while_counts_keep_their_values ... ok
  test api::review_totals_tests::a_session_list_response_differs_from_before_only_by_the_added_total_key ... ok
  test api::review_totals_tests::alignment_items_with_one_timestamp_are_each_listed_once_across_pages ... ok
  test api::review_token_revert_tests::review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall ... ok
  test api::review_token_revert_tests::a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided ... ok
  test api::review_token_reads_tests::a_token_receives_each_sections_items_in_the_session_order ... ok
  test api::review_totals_tests::an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total ... ok
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
  
  test result: ok. 122 passed; 0 failed; 0 ignored; 0 measured; 822 filtered out; finished in 27.97s
  ```
- 2026-11-20 — orchestrator regression Verify of T003 (sidecar gsd-path-verify/task-t005-verify): pass, exit 0; output tail:
  ```
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:24572) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:19260) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:14184) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:6844) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:19248) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:33592) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:40164) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  [33m
  (!) Some chunks are larger than 500 kB after minification. Consider:
  - Using dynamic import() to code-split the application
  - Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
  - Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.[39m
  ```
