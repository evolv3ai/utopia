---
id: T008
title: Give a token its own entries to propose an answer to a pending fact and to a governor decision
wave: 4
deps: [T007]
status: done
agent: build_T008
base: 3af3b48003fcb83908a4ae0529583f3fee7fa5b4
worktree: null
task_branch: null
files:
  - crates/utopia-server/src/api/review_ops.rs
  - crates/utopia-server/src/api/review_routes.rs
  - crates/utopia-server/src/api/review_tools.rs
  - crates/utopia-server/src/api/mod.rs
  - crates/utopia-server/src/api/review_token_pending_tests.rs
  - crates/utopia-server/src/api/review_token_governor_tests.rs
  - crates/utopia-server/src/api/review_mcp_reads_tests.rs
---

# T008 — Give a token its own entries to propose an answer to a pending fact and to a governor decision

## Context

Two propose-only rows do not enter through the person's route. The charter forbids a token from asserting or rejecting a pending fact by any path, and the governor's own route is a second way to merge, keep or revert a pair as a person; the milestone synthesis therefore gives each a separate token-only route and leaves the person's routes session-only. The M003 patterns to follow are in `crates/utopia-server/src/api/review_routes.rs` and `crates/utopia-server/src/api/review_ops.rs`: the token branch of `review_routes::decide` (lines 353 to 374), `review_ops::authorize_write` (lines 69 to 84), `review_ops::keep` (lines 361 to 388), `review_ops::guard` (lines 317 to 344) and the answer dispatch `review_routes::answer_proposal` (lines 850 to 918). The person's handlers are `decide_pending` (`crates/utopia-server/src/api/review_routes.rs` line 1800: `pending::confirm` or `pending::reject`, audit `fact.nod_confirmed` or `fact.nod_rejected`, the `resolve_time` job and three emits on confirm), `pending_for_chunk` (line 1778) and `agent_answer` (lines 439 to 555, three body shapes, error code `agent_answer`); their routes are in `crates/utopia-server/src/api/mod.rs` lines 539 to 563. `a_token_cannot_nod_and_every_review_write_outside_the_ruled_table_answers_401` in `crates/utopia-server/src/api/review_token_reads_tests.rs` pins 401 for a token on all three and must keep passing unedited.

## Approach

- Add the two routes of the interface contract in `crates/utopia-server/src/api/mod.rs` and two handlers in `crates/utopia-server/src/api/review_routes.rs` that read raw bytes, so a refused caller is answered before the body is parsed. A session caller gets 403 `token_required`.
- Do not edit `decide_pending`, `pending_for_chunk` or `agent_answer`, their signatures or their routes. The approval arm of a pending-fact proposal repeats the statements of `decide_pending` for the chosen action with the approving person as actor, plus `agent_proposal` in the audit detail. The approval arm of a governor proposal may move the statements of `agent_answer` unchanged into a function that takes the person and an optional proposal id, called by the session handler with none.
- N5 (judgment): a proposed answer to a pending fact always says that a token proposed it. The list item of the pending fact carries the proposal with `owner_name` and `token_name` (T003), the nod card shows that chip and keeps its line naming who remembered the sentence; this task's test checks the item's keys for a `pending.nod` proposal and that the pending fact's own fields are all still present.
- Charter veto (N4): no path of this task writes or rejects a fact for a token. The test compares the `pending_facts` row as a whole before and after every token call, over REST and MCP.
- Governor (E6): a proposal changes neither `agent_decisions`, the review pair nor the governance fuse and enqueues nothing. `governor.answer` is `Open` only by the rule of the typed target state; an action that does not fit the decision's status is refused at propose time with `agent_answer`.
- Declare `review_token_pending_tests` and `review_token_governor_tests` in `crates/utopia-server/src/api/mod.rs`.
- N2 (judgment): each approval has the person as actor and names the proposal. N3 (judgment): the new routes propose only; no token entry answers a proposal.
- MCP: follow the M003 write tools in `crates/utopia-server/src/api/review_tools.rs` (`serves` line 39, `writes` line 44, `definitions` line 70, `annotations` line 358, `listed` line 378, `bad_argument` line 403, `call` line 487). `crates/utopia-server/src/api/mcp.rs` calls only `listed`, `serves` and `call` and is not edited. Add each new tool name to the expected lists and to the `writes` expression of `the_mcp_endpoint_offers_exactly_the_review_tools_the_ruled_table_opens` in `crates/utopia-server/src/api/review_mcp_reads_tests.rs` (lists at lines 1050 to 1073, expression at lines 1087 to 1090); every other assertion of that test, the unknown names at lines 1180 to 1199 included, keeps its expectation.
- If an existing test outside this task's files fails because of this change, do not edit it: record `NEEDS-ORCHESTRATOR` in the Log with the test name. The only existing-test edits this milestone allows are a lifted token refusal, an exact tool list and a label list (edge E22).
- Conventions (bind this task): Chinese explanatory comments; sentence-named tests; English user-facing strings, each refusal with a stable code; one behaviour-sentence commit subject; DCO sign-off is deferred to the upstream submission; nothing is sent to `deeplethe/utopia` or a deployed instance. Tests seed through store functions or direct inserts, never through model jobs. No migration is written and `CURRENT_SCHEMA_VERSION` stays 115; no `AgentOperation` variant, no `AppError` variant and no item struct field is added. Line numbers in this brief are those of commit `d5d2356`.

## Interface contract

- **Typed target state (delivered by T001).** New module `crates/utopia-store/src/review_targets.rs`: `pub enum TargetState { Open, Decided, Missing }`; `pub async fn target_state(pool: &PgPool, kb_id: Uuid, operation: &str, target_id: Uuid, request: &serde_json::Value, fingerprint: Option<&serde_json::Value>) -> AppResult<TargetState>`; `pub async fn fingerprint(pool: &PgPool, kb_id: Uuid, operation: &str, target_id: Uuid) -> AppResult<Option<serde_json::Value>>`; `pub async fn target_label(pool: &PgPool, kb_id: Uuid, operation: &str, target_id: Uuid, request: &serde_json::Value) -> AppResult<String>`; `pub async fn ontology_proposal_id(pool: &PgPool, kb_id: Uuid, section: &str, key: &str) -> AppResult<Option<Uuid>>`. Every read is scoped by base, so an id of another base is `Missing`, and no error text is matched. Rules: `fact.confirm` — a missing or invalidated fact is `Missing` and there is no `Decided`; `fact.reject` and `fact.close` — an invalidated fact is `Decided`, and for `fact.close` a live fact that already has an end is `Missing`; `conflict.resolve` — `Open` while the conflict's status is open and both facts are live, `Decided` otherwise, and `Decided` when a fingerprint is given and the conflict's two fact ids differ from it; `violation.decide` — `Open` while the violation's status is open and, for the resolutions `fact_retracted` and `fact_closed`, the fact the request names is live and has no end, `Decided` otherwise; `defect.decide` — `Open` while the defect's status is open; `errata.decide` — `Open` while the action's status is held; `ontology.proposal` — `Open` while the proposal's status is open and the ontology agent proposed it, and `Decided` when a fingerprint is given and the stored payload differs from it; `alignment.phrase`, `alignment.rule` and `pending.nod` — `Open` while the row exists and `Missing` otherwise, with no `Decided`; `governor.answer` — `Open` when the decision is `proposed` and its review row is still pending, or the decision is `applied`, and `Decided` otherwise; `mapping.decide` — `Decided` when the mapping's status equals the requested status and `Open` otherwise. `fingerprint` returns the conflict's two fact ids or the ontology proposal's stored payload, and nothing for any other operation.
- **Token propose over REST and the operation layer (delivered by T002, used by every propose-only family).** A handler opened to tokens takes `SessionOrToken` and keeps its `Json` request struct; `if let Some(token) = caller.pat` returns the `review_ops` answer on every path, and the session code below it is unchanged. In `crates/utopia-server/src/api/review_ops.rs`: `pub(crate) async fn target_open(state: &AppState, caller: &ReviewCaller, operation: &str, target_id: Uuid, request: &serde_json::Value) -> Result<(), AppError>` (`Missing` is 404, `Decided` is 409 `already_decided`) and `pub(crate) async fn land_proposal(state: &AppState, caller: &ReviewCaller, row: AgentOperation, operation: &'static str, target_kind: &'static str, target_id: Uuid, target_key: &str, request: serde_json::Value, action: Option<&str>, rationale: Option<&str>) -> Result<serde_json::Value, AppError>`, which lands only when `agent_policy(row, None)` is `AgentOutcome::Proposed`, takes the target's label and fingerprint from the store, and calls neither `guard` nor the breaker. Order in a family function: `authorize_write`; shape checks of the body with the person's codes; `target_open`; the state-dependent checks with the person's codes; `land_proposal`. Answer, HTTP 200: `{"ok":true,"outcome":"proposed","action_id":…,"reason":"propose_only","explanation":…}` plus `"code":"already_proposed"` only when the call met the open proposal, whose id `action_id` then is. Refusals: 403 `token_read_only`, 403 `token_kb_out_of_scope`, 403 `token_owner_below_editor` (checked in that order, as `authorize_write` does today), 404, 409 `already_decided`, and the person's 422 codes. A body that does not deserialize is answered by the `Json` extractor before the scope check, for every caller. A propose-only call is not counted toward the limit and is never answered `agent_rate_limited`.
- **Proposal answer for every operation (delivered by T002; each family task adds its arm).** The two session-only routes `POST /api/v1/kbs/{id}/review/agent-proposals/{proposal_id}` and `POST /api/v1/kbs/{id}/review/agent-proposals` keep `AuthUser`, their bodies and their M003 answers; `review_routes::answer_proposal` gains one arm per operation string and keeps answering 500 for an operation it does not know. Before the person's code runs, approval reads `target_state` with the stored `request` and `fingerprint`: `Missing` or `Decided` closes the proposal `superseded` and changes nothing else. Otherwise the arm runs the person's own code for that operation with the person as actor: each opened handler's session statements are moved unchanged into one function that takes the person and an optional proposal id; the session handler calls it with none and the arm with the id, which only adds `agent_proposal` (the ledger row id) to the audit detail. That audit row never carries `token_id`, and the token's rationale is never passed as the person's reason. An error of the person's code is returned with the person's code and leaves the proposal open; only a typed `NotFound` from a function that is one guarded statement or one transaction (confirm fact, reject fact, defect, conflict, ontology adoption) closes it `superseded`. Single answer, HTTP 200: `{"ok":true,"state":"approved"|"declined"|"superseded"}` plus, each only when set, `merge_id` (as today), `job_id` with `"queued":true` (an approved phrase or rule proposal) and `fact_id` (an approved confirm of a pending fact). A bulk outcome keeps its five keys and gains the same optional keys only when set. Decline and supersede audit rows keep their action names and carry `left` and `right` only when the payload has them.
- **MCP tools of this milestone (file `crates/utopia-server/src/api/review_tools.rs`).** Each new write tool is in `serves` and `writes`, is listed in `tools/list` only for a `write` token whose owner is at least Editor, and a caller without that right that calls one gets a tool error with the REST code (`token_read_only`, `token_owner_below_editor`), not JSON-RPC -32601. Arguments are the route's id and the fields of the handler's request struct; `structuredContent` is the REST body of the matching token call; a refused call is `isError: true` with `{"code","message"}` (`not_found` for a 404). Propose-only tools — `review_defect`, `review_fact`, `review_conflict`, `review_violation`, `review_errata`, `review_alignment`, `review_ontology`, `review_pending_fact`, `review_governor` — are annotated `{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":false}`; each description begins "Propose that a person", says that the call changes nothing, that a signed-in editor answers it in the Review tab, the result shape and `already_proposed` on a repeat, and ends "No tool answers a proposal." Finalize tools — `review_mapping`, `review_kind_word`, `review_check` — are annotated `{"readOnlyHint":false,"destructiveHint":true,"idempotentHint":false}`. The read tools `review_mapping_list` and `review_ontology_list` are annotated exactly `{"readOnlyHint":true}` and listed for every token that reaches the base. No listed name contains `proposal`, `approve`, `decline` or `answer`; `review_confirm`, `review_decide`, `review_pending` and `review_approve` stay unknown tools answered with JSON-RPC -32601. Each tool is delivered by the task that opens its route: `review_defect` by T002; `review_fact` and `review_conflict` by T004; `review_violation`, `review_errata` and `review_alignment` by T005; `review_ontology` and `review_ontology_list` by T006; `review_mapping`, `review_mapping_list`, `review_kind_word` and `review_check` by T007; `review_pending_fact` and `review_governor` by T008.
- **Test support for token writes (delivered by T002).** `crates/utopia-server/src/api/review_token_write_support.rs`, declared `#[cfg(test)] mod review_token_write_support;` in `crates/utopia-server/src/api/mod.rs` and so mounted as `api::review_token_write_support`, holds `pub(super)` helpers that later test modules import and do not edit: a REST call with a bearer token, an MCP `tools/list` and `tools/call`, the check that a tool result is a refusal with a given code, a snapshot of every `facts`, `entities` and `fact_evidence` row of the base with the count of queued jobs, the open proposals and the ledger rows of the base, and the audit rows of the base. It builds on `Fx` of `review_session_unchanged_tests` and `seed_every_section` of `review_totals_tests`.
- **Open proposals on list items (delivered by T003).** For session, token and MCP readers alike, every list item of the sections Awaiting your nod, Conflicts, Unconfirmed, Low confidence, Axioms, Ontology defects and Errata, every phrase and rule item of Alignment, and every governor row of Agent History carries `agent_proposals`: an array, oldest first, of `{"id","operation","reason","owner_name","token_name","created_at"}` plus `"action"` when the proposal has one, present only while at least one proposal is open, so an item without one has exactly the keys it has today. The key is attached at the JSON layer by one helper in `crates/utopia-server/src/api/review_ops.rs`, `pub(crate) async fn attach_agent_proposals(state: &AppState, kb_id: Uuid, target_kind: &str, items: &mut [serde_json::Value]) -> Result<(), AppError>`, which reads each item's `id`. Duplicates items and Merges rows keep the M003 key `agent_proposal` unchanged; kind-word items and mapping rows carry no proposal. The `counts` object of every list answer carries `agent_token_proposals` only while a token proposal is open.
- **The two separate token entries (delivered by T008).** `POST /api/v1/kbs/{id}/review/pending/{pending_id}/propose` with body `{"action":"confirm"|"reject"}` and `POST /api/v1/kbs/{id}/review/agent/{decision_id}/propose` with body `{"action":"merge"|"keep"|"revert","rationale":…}` take `SessionOrToken`, read the body as raw bytes and parse it after `authorize_write` (422 `invalid_body`); a caller without a personal token is refused with 403 `token_required` and changes nothing. They go to `review_ops::propose_pending(state, caller, pending_id, action)` (policy row `PendingNod`, operation `pending.nod`; an action other than `confirm` or `reject` is 422 `unknown_action`; an unknown or already nodded pending id is 404) and `review_ops::propose_governor(state, caller, decision_id, action, rationale)` (policy row `AnswerGovernor`, operation `governor.answer`; an action that does not fit the decision's status is 422 `agent_answer`; a decision already answered, or a `proposed` one whose review row is no longer pending, is 409 `already_decided`). The nod route `POST /api/v1/kbs/{id}/review/pending/{pending_id}`, `GET /api/v1/kbs/{id}/review/pending` and the governor's route `POST /api/v1/kbs/{id}/review/agent/{decision_id}` are not edited and keep answering every token 401. MCP tools `review_pending_fact` (`id`, `action`) and `review_governor` (`id`, `action`, `rationale`). Approving a pending-fact proposal records the nod as the person's: audit row `fact.nod_confirmed` (answer carries `fact_id`) or `fact.nod_rejected`, the person as actor, `agent_proposal` in the detail; declining leaves the pending row waiting. Approving a governor proposal runs the person's governor answer as that person with no rationale, no `token_id` key and `agent_proposal` in the detail.

## Intent coverage

- SC1
- SC2
- SC5

## Acceptance criteria

1. E17 (held-out; ruling: after any token call about a pending fact, over REST or MCP, the `pending_facts` row is byte-equal and no fact was written or rejected; a pending id that is unknown or already nodded is not found; the person's nod route still answers a token as it did before): a confirm and a reject proposal over the propose route and over `review_pending_fact` leave the row equal and write no fact, an unknown or nodded id is 404, and the nod route and the pending-chunk read still answer every token 401.
2. E18 (held-out; ruling: when a signed-in editor approves a proposed answer to a pending fact, the nod is recorded as that person's, `fact.nod_confirmed` or `fact.nod_rejected` with the person as actor and the proposal named; declining leaves the pending row waiting): an approved confirm writes the fact as the person's nod does and answers `fact_id`, an approved reject records the rejection, and a declined proposal leaves the row and the queue as they were.
3. E6 (held-out; ruling: a token's proposed answer to a governor decision changes neither the governor's decision row nor the pair nor the governance fuse and starts no govern round; when a person approves it, the answer runs as that person's answer does today): the decision row, the pair, the fuse and the queued jobs are equal after the proposal, and the approved proposal has the effect and the audit row of that person's own answer, with the proposal named and no rationale of the token.
4. A session caller of either propose route gets 403 `token_required`; an unreadable body is 422 `invalid_body` after the scope check; an action that does not fit is 422 with the person's code; a governor decision already answered is 409 `already_decided`.
5. The item of a pending fact with an open proposal names the owner and the token and keeps every field it has without one.
6. `review_pending_fact` and `review_governor` are listed and refused as the write tools are, and `a_token_cannot_nod_and_every_review_write_outside_the_ruled_table_answers_401` passes unedited.

## Verify

```bash
test -s crates/utopia-server/src/api/review_token_pending_tests.rs && test -s crates/utopia-server/src/api/review_token_governor_tests.rs && docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m004_t008 WITH (FORCE)" -c "CREATE DATABASE m004_t008" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m004_t008 && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- a_token_proposal_about_a_pending_fact_leaves_the_pending_row_byte_equal an_unknown_or_already_nodded_pending_id_is_not_found approving_a_pending_fact_proposal_records_the_nod_as_that_persons declining_a_pending_fact_proposal_leaves_the_row_waiting a_caller_without_a_token_is_refused_on_the_two_propose_routes a_token_proposed_governor_answer_changes_no_decision_pair_or_fuse approving_a_governor_proposal_runs_that_persons_answer the_mcp_pending_fact_and_governor_tools_propose_and_change_nothing a_token_cannot_nod_and_every_review_write_outside_the_ruled_table_answers_401 the_mcp_endpoint_offers_exactly_the_review_tools_the_ruled_table_opens 2>&1 | tee /dev/stderr | grep -c -E "(a_token_proposal_about_a_pending_fact_leaves_the_pending_row_byte_equal|an_unknown_or_already_nodded_pending_id_is_not_found|approving_a_pending_fact_proposal_records_the_nod_as_that_persons|declining_a_pending_fact_proposal_leaves_the_row_waiting|a_caller_without_a_token_is_refused_on_the_two_propose_routes|a_token_proposed_governor_answer_changes_no_decision_pair_or_fuse|approving_a_governor_proposal_runs_that_persons_answer|the_mcp_pending_fact_and_governor_tools_propose_and_change_nothing|a_token_cannot_nod_and_every_review_write_outside_the_ruled_table_answers_401|the_mcp_endpoint_offers_exactly_the_review_tools_the_ruled_table_opens) \.\.\. ok" | grep -qx 10 && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- review_session_unchanged_tests:: review_proposal_answer_tests:: review_token_reads_tests:: review_mcp_reads_tests:: review_mcp_tests:: review_totals_tests:: review_token_api_tests:: review_token_merge_tests:: review_token_revert_tests:: review_token_batch_tests:: review_token_guard_tests:: review_routes::attribution_tests:: review_routes::conflict_tests:: review_routes::phrase_tests:: review_token_defect_tests:: review_token_fact_tests:: review_token_conflict_tests:: review_token_violation_tests:: review_token_errata_tests:: review_token_alignment_tests:: review_token_mapping_tests:: review_item_proposals_tests:: review_token_ontology_tests:: review_token_run_tests:: review_token_kind_word_tests:: 2>&1 | tee /dev/stderr | grep -c -E "test result: ok\. [1-9]" | grep -qvx 0
```
<!-- Runs in a fresh sidecar worktree at the landed commit with nothing installed. It creates and migrates its own database(s) on the compose Postgres (`utopia-db-1`, 127.0.0.1:1517). The orchestrator's environment supplies an empty `CARGO_TARGET_DIR` outside the worktree, PostgreSQL 16 client tools and the `tee` shim first on PATH. The leading `test -s` checks name the test files this task owns. -->

Heavy: yes

## Log

- 2026-11-20 — created by planner
- 2026-11-20 — coder build_T008 (took over from a coder stopped by a usage limit; its uncommitted work in `D:\utopia` reviewed, kept and corrected). Implementation: `review_ops.rs` adds `token_required()` (existing `AppError::CodedForbidden`, no new variant), `propose_pending` (authorize_write → `unknown_action` → `target_open("pending.nod")` → `land_proposal(PendingNod, "pending.nod", "pending_fact")`) and `propose_governor` (authorize_write → `target_open("governor.answer")` → status/action checks with the person's codes `agent_answer` / `not_a_merge` → `land_proposal(AnswerGovernor, ...)`; stored request is `{"action"}` only, rationale stays in `why`). `review_routes.rs` adds token-only handlers `propose_pending` / `propose_governor` (`SessionOrToken`, raw `Bytes`, session → 403 `token_required`, unparsable body → `authorize_write` then 422 `invalid_body`); moves `agent_answer`'s statements unchanged into `governor_answer(state, kb, user, decision, body, proposal: Option<Uuid>)` (session calls with None, detail unchanged); adds `pending_nod` (statements of `decide_pending` with the approving person as actor and `agent_proposal` in the detail; `decide_pending`, `pending_for_chunk` and their routes untouched); `answer_proposal` gains arms `pending.nod` (answers `fact_id` on confirm) and `governor.answer` (rationale forced to None). The nod/governor audit rows are written in `review_routes.rs` (in `files`). `mod.rs` mounts `/review/pending/{pending_id}/propose`, `/review/agent/{decision_id}/propose` and the two test modules. `review_tools.rs` adds `review_pending_fact` and `review_governor` (serves, writes, propose-only annotations, descriptions, call). `review_mcp_reads_tests.rs`: only the exact tool list and `writes` expression gained the two names (E22).
  Correction made: `an_unknown_or_already_nodded_pending_id_is_not_found` counted the existing `mcp.tool_called` audit row that the MCP entry writes for every call (refused ones included, out of scope to change) — the count now excludes that action, as `review_token_batch_tests` does. `cargo fmt` applied to the two new test files.
  Commands (own DB `m004_t008_dev`, migrated; `UTOPIA_TEST_REQUIRE_DB=1`): the 10 named Verify tests → 10 passed, 0 failed (each filter matched one mounted test); `cargo test -p utopia-server` → 965 passed, 0 failed, 2 ignored (957 before + 8 new); `cargo fmt --all --check` → clean; `cargo clippy --workspace --all-targets -- -D warnings` → clean. The Verify command itself (database `m004_t008`) was not run, per brief. Deviations: none from the Interface contract. Untested: the Review tab nod card (N5 chip rendering) in a browser — covered only by the list-item key test.
- 2026-11-20 — orchestrator Verify (sidecar gsd-path-verify/task-t008-verify): pass, exit 0; output tail:
  ```
  test api::review_totals_tests::a_token_reading_duplicates_receives_the_same_total_as_a_session ... ok
  test api::review_token_reads_tests::a_token_receives_each_sections_items_in_the_session_order ... ok
  test api::review_token_revert_tests::a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided ... ok
  test api::review_token_run_tests::a_token_consistency_check_and_inference_run_each_write_one_applied_row ... ok
  test api::review_totals_tests::alignment_items_with_one_timestamp_are_each_listed_once_across_pages ... ok
  test api::review_totals_tests::a_session_list_response_differs_from_before_only_by_the_added_total_key ... ok
  test api::review_totals_tests::an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total ... ok
  test api::review_token_reads_tests::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values ... ok
  test api::review_token_revert_tests::a_token_reverts_any_token_agents_merge_of_the_base_and_an_unknown_or_foreign_merge_id_is_not_found ... ok
  test api::review_token_revert_tests::after_a_token_merge_and_its_revert_every_fact_entity_and_evidence_row_is_as_before ... ok
  test api::review_token_run_tests::the_call_after_the_200th_finalized_write_answers_429_and_changes_nothing ... ok
  test api::review_totals_tests::the_seed_puts_items_in_every_one_of_the_thirteen_sections ... ok
  test api::review_totals_tests::the_total_of_every_list_queue_is_the_number_of_rows_its_pages_return ... ok
  test api::review_token_reads_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read ... ok
  test api::review_token_merge_tests::of_a_person_the_governor_and_a_token_deciding_one_pair_exactly_one_is_applied_and_the_losing_token_gets_already_decided ... ok
  test api::review_token_violation_tests::a_token_violation_decision_waits_as_a_proposal_and_a_person_applies_it ... ok
  test api::review_token_reads_tests::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items ... ok
  
  test result: ok. 140 passed; 0 failed; 0 ignored; 0 measured; 827 filtered out; finished in 32.50s
  ```
- 2026-11-20 — orchestrator regression Verify of T002 (sidecar gsd-path-verify/task-t008-verify): pass, exit 0; output tail:
  ```
  test api::review_totals_tests::the_alignment_low_confidence_and_agent_totals_follow_the_list_while_counts_keep_their_values ... ok
  test api::review_token_revert_tests::a_token_revert_of_a_persons_or_the_governors_merge_lands_a_proposal_and_reverts_nothing ... ok
  test api::review_totals_tests::alignment_items_with_one_timestamp_are_each_listed_once_across_pages ... ok
  test api::review_token_revert_tests::review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall ... ok
  test api::review_token_revert_tests::a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided ... ok
  test api::review_token_reads_tests::a_token_receives_each_sections_items_in_the_session_order ... ok
  test api::review_totals_tests::an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total ... ok
  test api::review_token_merge_tests::each_of_the_six_hold_reasons_lands_a_proposal_that_names_it_for_the_queue_merge_and_the_manual_merge ... ok
  test api::review_totals_tests::the_seed_puts_items_in_every_one_of_the_thirteen_sections ... ok
  test api::review_token_reads_tests::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values ... ok
  test api::review_totals_tests::the_total_of_every_list_queue_is_the_number_of_rows_its_pages_return ... ok
  test api::review_token_revert_tests::a_token_reverts_any_token_agents_merge_of_the_base_and_an_unknown_or_foreign_merge_id_is_not_found ... ok
  test api::review_token_revert_tests::after_a_token_merge_and_its_revert_every_fact_entity_and_evidence_row_is_as_before ... ok
  test api::review_token_reads_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read ... ok
  test api::review_token_merge_tests::of_a_person_the_governor_and_a_token_deciding_one_pair_exactly_one_is_applied_and_the_losing_token_gets_already_decided ... ok
  test api::review_token_violation_tests::a_token_violation_decision_waits_as_a_proposal_and_a_person_applies_it ... ok
  test api::review_token_reads_tests::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items ... ok
  
  test result: ok. 124 passed; 0 failed; 0 ignored; 0 measured; 843 filtered out; finished in 28.51s
  ```
- 2026-11-20 — orchestrator regression Verify of T003 (sidecar gsd-path-verify/task-t008-verify): pass, exit 0; output tail:
  ```
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:36660) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:40456) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:16284) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:32488) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:19680) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:7088) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:34456) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  [33m
  (!) Some chunks are larger than 500 kB after minification. Consider:
  - Using dynamic import() to code-split the application
  - Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
  - Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.[39m
  ```
- 2026-11-20 — orchestrator regression Verify of T004 (sidecar gsd-path-verify/task-t008-verify): pass, exit 0; output tail:
  ```
  test api::review_totals_tests::alignment_items_with_one_timestamp_are_each_listed_once_across_pages ... ok
  test api::review_token_revert_tests::a_token_revert_of_a_persons_or_the_governors_merge_lands_a_proposal_and_reverts_nothing ... ok
  test api::review_totals_tests::the_alignment_low_confidence_and_agent_totals_follow_the_list_while_counts_keep_their_values ... ok
  test api::review_token_reads_tests::a_token_receives_each_sections_items_in_the_session_order ... ok
  test api::review_token_revert_tests::review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall ... ok
  test api::review_token_revert_tests::a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided ... ok
  test api::review_totals_tests::an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total ... ok
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
  
  test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 840 filtered out; finished in 29.69s
  ```
- 2026-11-20 — orchestrator regression Verify of T005 (sidecar gsd-path-verify/task-t008-verify): pass, exit 0; output tail:
  ```
  test api::review_totals_tests::the_alignment_low_confidence_and_agent_totals_follow_the_list_while_counts_keep_their_values ... ok
  test api::review_token_revert_tests::a_token_revert_of_a_persons_or_the_governors_merge_lands_a_proposal_and_reverts_nothing ... ok
  test api::review_token_reads_tests::a_token_receives_each_sections_items_in_the_session_order ... ok
  test api::review_totals_tests::alignment_items_with_one_timestamp_are_each_listed_once_across_pages ... ok
  test api::review_token_revert_tests::review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall ... ok
  test api::review_token_revert_tests::a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided ... ok
  test api::review_token_merge_tests::each_of_the_six_hold_reasons_lands_a_proposal_that_names_it_for_the_queue_merge_and_the_manual_merge ... ok
  test api::review_totals_tests::an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total ... ok
  test api::review_token_reads_tests::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values ... ok
  test api::review_totals_tests::the_seed_puts_items_in_every_one_of_the_thirteen_sections ... ok
  test api::review_token_revert_tests::a_token_reverts_any_token_agents_merge_of_the_base_and_an_unknown_or_foreign_merge_id_is_not_found ... ok
  test api::review_token_revert_tests::after_a_token_merge_and_its_revert_every_fact_entity_and_evidence_row_is_as_before ... ok
  test api::review_totals_tests::the_total_of_every_list_queue_is_the_number_of_rows_its_pages_return ... ok
  test api::review_token_reads_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read ... ok
  test api::review_token_merge_tests::of_a_person_the_governor_and_a_token_deciding_one_pair_exactly_one_is_applied_and_the_losing_token_gets_already_decided ... ok
  test api::review_token_violation_tests::a_token_violation_decision_waits_as_a_proposal_and_a_person_applies_it ... ok
  test api::review_token_reads_tests::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items ... ok
  
  test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 840 filtered out; finished in 29.56s
  ```
- 2026-11-20 — orchestrator regression Verify of T006 (sidecar gsd-path-verify/task-t008-verify): pass, exit 0; output tail:
  ```
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:38904) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:23796) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:37196) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:29028) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:21600) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:18868) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:38140) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  [33m
  (!) Some chunks are larger than 500 kB after minification. Consider:
  - Using dynamic import() to code-split the application
  - Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
  - Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.[39m
  ```
- 2026-11-20 — orchestrator regression Verify of T007 (sidecar gsd-path-verify/task-t008-verify): pass, exit 0; output tail:
  ```
  test api::review_totals_tests::a_token_reading_duplicates_receives_the_same_total_as_a_session ... ok
  test api::review_totals_tests::a_session_list_response_differs_from_before_only_by_the_added_total_key ... ok
  test api::review_totals_tests::alignment_items_with_one_timestamp_are_each_listed_once_across_pages ... ok
  test api::review_token_revert_tests::review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall ... ok
  test api::review_token_revert_tests::a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided ... ok
  test api::review_token_run_tests::a_token_consistency_check_and_inference_run_each_write_one_applied_row ... ok
  test api::review_totals_tests::an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total ... ok
  test api::review_token_reads_tests::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values ... ok
  test api::review_token_run_tests::the_call_after_the_200th_finalized_write_answers_429_and_changes_nothing ... ok
  test api::review_token_revert_tests::a_token_reverts_any_token_agents_merge_of_the_base_and_an_unknown_or_foreign_merge_id_is_not_found ... ok
  test api::review_totals_tests::the_seed_puts_items_in_every_one_of_the_thirteen_sections ... ok
  test api::review_token_revert_tests::after_a_token_merge_and_its_revert_every_fact_entity_and_evidence_row_is_as_before ... ok
  test api::review_totals_tests::the_total_of_every_list_queue_is_the_number_of_rows_its_pages_return ... ok
  test api::review_token_reads_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read ... ok
  test api::review_token_merge_tests::of_a_person_the_governor_and_a_token_deciding_one_pair_exactly_one_is_applied_and_the_losing_token_gets_already_decided ... ok
  test api::review_token_violation_tests::a_token_violation_decision_waits_as_a_proposal_and_a_person_applies_it ... ok
  test api::review_token_reads_tests::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items ... ok
  
  test result: ok. 140 passed; 0 failed; 0 ignored; 0 measured; 827 filtered out; finished in 31.72s
  ```
