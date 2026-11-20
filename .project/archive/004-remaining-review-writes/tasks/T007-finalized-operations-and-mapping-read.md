---
id: T007
title: Let a token decide a mapping, run the check and inference, and record its kind-word decision
wave: 3
deps: [T006]
status: done
agent: build_T007
base: d7d0ddc3b7ca8e6d42a9f4e65422b56ffb039ae6
worktree: null
task_branch: null
files:
  - crates/utopia-server/src/api/review_ops.rs
  - crates/utopia-server/src/api/review_routes.rs
  - crates/utopia-server/src/api/review_tools.rs
  - crates/utopia-server/src/api/mapping_routes.rs
  - crates/utopia-server/src/api/mod.rs
  - crates/utopia-store/src/mappings.rs
  - crates/utopia-store/src/token_agent_actions.rs
  - crates/utopia-store/tests/a_token_finalize_writes_its_row_with_the_change.rs
  - crates/utopia-server/src/api/review_token_mapping_tests.rs
  - crates/utopia-server/src/api/review_token_run_tests.rs
  - crates/utopia-server/src/api/review_token_kind_word_tests.rs
  - crates/utopia-server/src/api/review_mcp_reads_tests.rs
---

# T007 — Let a token decide a mapping, run the check and inference, and record its kind-word decision

## Context

Three rows of the ruled table finalize: a mapping decision, a consistency check or inference run, and the kind-word decision, which already finalizes for a write token. The M003 pattern for a finalizing token operation is `review_ops::keep` (`crates/utopia-server/src/api/review_ops.rs` lines 361 to 388) with `review_ops::authorize_write` (lines 69 to 84) and the limit check `review_ops::guard` (lines 317 to 344: 200 applied rows per token and base in a rolling hour, `agent_rate_limited`). The session handlers are `decide_mapping` (`crates/utopia-server/src/api/review_routes.rs` line 1475, audit `mapping.decided`, store call `mappings::decide` at `crates/utopia-store/src/mappings.rs` line 192, which takes a pool), `run_consistency_check` (line 1679, audit `consistency.checked`), `run_inference` (line 1863, `inference_off`, audit `inference.materialized`), `decide_alignment_kind_word` (line 2091, already `SessionOrToken`, `type_bindings::decide_and_apply_human`, whose transaction variant `decide_and_apply_human_tx` is at `crates/utopia-store/src/type_bindings.rs` line 476) and the mappings list `mapping_routes::list` (`crates/utopia-server/src/api/mapping_routes.rs` line 36).

## Approach

- Mapping: give `mappings::decide` a core that runs on a connection, keep the pool function's signature and behaviour for the person, and write `token_agent_actions::decide_mapping` so the status change, the `applied` row (`record_applied`) and the audit row commit together. A mapping has no decided state: a status equal to the current one is `already_decided`, a different one applies.
- Runs: the run itself is today's code with no new lock. `inference_off` is checked before `guard` and writes no row. The `applied` row and the audit row are written right after the run succeeds; the answer is the report a person gets.
- N7 (judgment): the mapping decision and both runs go through `review_ops::guard` before they change anything and write their `applied` row with the change; neither has a path around the policy row, which must answer `AgentOutcome::Applied`.
- Kind word: the REST handler keeps its two checks (`caller.require_write` and the role check), its status, body, `unknown_class`, `alignment_busy` and its plain 403. Only the store call for a token caller changes, to `token_agent_actions::decide_kind_word`, and the audit detail gains `token_name` and `action_id` beside the `token_id` that `audit::via_token` writes. It never calls `guard`. A session call is unchanged. The MCP tool goes through `review_ops::decide_kind_word`.
- The mappings read: `SessionOrToken` on the list route with the token rule of the M002 reads. `mappings` stays `unknown_queue` on the shared list and on `review_list`; the tests `review_list_refuses_mappings_and_an_unknown_section_with_unknown_queue` and those of `review_totals_tests` are not edited.
- Replace `a_token_is_still_refused_on_the_mapping_route` with the new behaviour. Declare `review_token_run_tests` and `review_token_kind_word_tests` in `crates/utopia-server/src/api/mod.rs`. The limit test seeds 200 `applied` rows with direct inserts, as `review_token_guard_tests` does.
- `review_mapping_list` joins the read tools of `the_mcp_endpoint_offers_exactly_the_review_tools_the_ruled_table_opens`; the three finalize tools are write tools.
- MCP: follow the M003 write tools in `crates/utopia-server/src/api/review_tools.rs` (`serves` line 39, `writes` line 44, `definitions` line 70, `annotations` line 358, `listed` line 378, `bad_argument` line 403, `call` line 487). `crates/utopia-server/src/api/mcp.rs` calls only `listed`, `serves` and `call` and is not edited. Add each new tool name to the expected lists and to the `writes` expression of `the_mcp_endpoint_offers_exactly_the_review_tools_the_ruled_table_opens` in `crates/utopia-server/src/api/review_mcp_reads_tests.rs` (lists at lines 1050 to 1073, expression at lines 1087 to 1090); every other assertion of that test, the unknown names at lines 1180 to 1199 included, keeps its expectation.
- If an existing test outside this task's files fails because of this change, do not edit it: record `NEEDS-ORCHESTRATOR` in the Log with the test name. The only existing-test edits this milestone allows are a lifted token refusal, an exact tool list and a label list (edge E22).
- Conventions (bind this task): Chinese explanatory comments; sentence-named tests; English user-facing strings, each refusal with a stable code; one behaviour-sentence commit subject; DCO sign-off is deferred to the upstream submission; nothing is sent to `deeplethe/utopia` or a deployed instance. Tests seed through store functions or direct inserts, never through model jobs. No migration is written and `CURRENT_SCHEMA_VERSION` stays 115; no `AgentOperation` variant, no `AppError` variant and no item struct field is added. Line numbers in this brief are those of commit `d5d2356`.

## Interface contract

- **Ledger names of this milestone (delivered by T001).** No migration; `token_agent_actions` keeps its columns. A propose-only row has `outcome` `proposed` and `gate_reason` `propose_only`; a finalized row has `outcome` `applied`. Operation and `target_kind`: `fact.confirm`, `fact.reject`, `fact.close` (`fact`); `conflict.resolve` (`conflict`); `violation.decide` (`axiom_violation`); `defect.decide` (`ontology_defect`); `errata.decide` (`errata_action`); `alignment.phrase` (`phrase_binding`); `alignment.rule` (`implication_rule`); `pending.nod` (`pending_fact`); `governor.answer` (`agent_decision`); `mapping.decide` (`concept_mapping`) — each with the row's own id as `target_id` and as `target_key`; `ontology.proposal` (`ontology_proposal`, `target_id` the id of the `ontology_proposals` row, `target_key` the section and the key joined by `:`); `alignment.kind_word` (`type_binding`, `target_id` the base id, `target_key` the normalized kind word); `consistency.check` and `inference.run` (`knowledge_base`, `target_id` and `target_key` the base id). There is one open proposal per base, operation and `target_key`. `payload` keys: `request` (the full request body), `target_label` (words naming the target), `action` (when the operation has one), `why` (the rationale, when given) and `fingerprint` (conflict and ontology proposal only). `action` codes: `confirm`, `reject`, `close` (the three fact operations); `close`, `retime_new`, `keep`, `reject_new` (conflict); `fact_retracted`, `fact_closed`, `axiom_relaxed`, `accepted` (violation); `fixed`, `accepted` (defect); `approve`, `reject` (errata and rule); `bind`, `none` (phrase and kind word); `adopt`, `reject` (ontology proposal); `confirm`, `reject` (pending fact); `merge`, `keep`, `revert` (governor answer); `confirmed`, `rejected` (mapping); the two runs have none.
- **Store functions for proposals and finalized rows (delivered by T001; no later task changes a signature).** In `crates/utopia-store/src/token_agent_actions.rs`: `pub struct Proposal<'a> { pub operation: &'a str, pub target_kind: &'a str, pub target_id: Uuid, pub target_key: &'a str, pub payload: serde_json::Value }`; `pub async fn propose(pool: &PgPool, w: &TokenWrite, p: &Proposal<'_>) -> AppResult<TokenWriteOutcome>` writes in one transaction one `proposed` ledger row and its `agent.proposal` audit row (`target_kind` `agent_action`, detail `operation`, `outcome`, `reason`, `token_id`, `token_name`, `action_id`) and touches no other table; when an open proposal exists for that base, operation and `target_key`, or a second call arrives at the same moment, it writes nothing and returns the open row with `existing` true. `pub async fn record_applied(conn: &mut sqlx::PgConnection, w: &TokenWrite, p: &Proposal<'_>) -> AppResult<TokenAgentAction>` writes one `applied` ledger row on the caller's connection, so it commits or rolls back with the caller's change. `pub fn token_audit_detail(detail: serde_json::Value, action: &TokenAgentAction) -> serde_json::Value` adds `token_id`, `token_name` and `action_id` to an audit detail. `pub async fn open_proposals_for(pool: &PgPool, kb_id: Uuid, target_kind: &str, target_ids: &[Uuid]) -> AppResult<std::collections::HashMap<Uuid, Vec<AgentProposalView>>>` returns the open proposals of those targets, oldest first. `AgentProposalView` gains `action: Option<String>`, serialized only when set. `held_back`, `land_proposal`, the three merge functions, `keep_duplicate_pair`, `answer`, `proposal`, `target_gone` and `finalized_since` keep their signatures and behaviour.
- **MCP tools of this milestone (file `crates/utopia-server/src/api/review_tools.rs`).** Each new write tool is in `serves` and `writes`, is listed in `tools/list` only for a `write` token whose owner is at least Editor, and a caller without that right that calls one gets a tool error with the REST code (`token_read_only`, `token_owner_below_editor`), not JSON-RPC -32601. Arguments are the route's id and the fields of the handler's request struct; `structuredContent` is the REST body of the matching token call; a refused call is `isError: true` with `{"code","message"}` (`not_found` for a 404). Propose-only tools — `review_defect`, `review_fact`, `review_conflict`, `review_violation`, `review_errata`, `review_alignment`, `review_ontology`, `review_pending_fact`, `review_governor` — are annotated `{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":false}`; each description begins "Propose that a person", says that the call changes nothing, that a signed-in editor answers it in the Review tab, the result shape and `already_proposed` on a repeat, and ends "No tool answers a proposal." Finalize tools — `review_mapping`, `review_kind_word`, `review_check` — are annotated `{"readOnlyHint":false,"destructiveHint":true,"idempotentHint":false}`. The read tools `review_mapping_list` and `review_ontology_list` are annotated exactly `{"readOnlyHint":true}` and listed for every token that reaches the base. No listed name contains `proposal`, `approve`, `decline` or `answer`; `review_confirm`, `review_decide`, `review_pending` and `review_approve` stay unknown tools answered with JSON-RPC -32601. Each tool is delivered by the task that opens its route: `review_defect` by T002; `review_fact` and `review_conflict` by T004; `review_violation`, `review_errata` and `review_alignment` by T005; `review_ontology` and `review_ontology_list` by T006; `review_mapping`, `review_mapping_list`, `review_kind_word` and `review_check` by T007; `review_pending_fact` and `review_governor` by T008.
- **Test support for token writes (delivered by T002).** `crates/utopia-server/src/api/review_token_write_support.rs`, declared `#[cfg(test)] mod review_token_write_support;` in `crates/utopia-server/src/api/mod.rs` and so mounted as `api::review_token_write_support`, holds `pub(super)` helpers that later test modules import and do not edit: a REST call with a bearer token, an MCP `tools/list` and `tools/call`, the check that a tool result is a refusal with a given code, a snapshot of every `facts`, `entities` and `fact_evidence` row of the base with the count of queued jobs, the open proposals and the ledger rows of the base, and the audit rows of the base. It builds on `Fx` of `review_session_unchanged_tests` and `seed_every_section` of `review_totals_tests`.
- **Finalized token operations (delivered by T007).** `GET /api/v1/kbs/{id}/mappings` takes `SessionOrToken` (a token needs an owner at least Viewer and `kb_ids` that reach the base, 404 otherwise; `review_ops::mappings` is the token read; `mappings` stays `unknown_queue` on the shared list and on `review_list`). `POST /api/v1/kbs/{id}/review/mappings/{mapping_id}`, `POST /api/v1/kbs/{id}/consistency/check` and `POST /api/v1/kbs/{id}/inference/run` take `SessionOrToken`. Token callers go to `review_ops::decide_mapping(state, caller, mapping_id, status)` and `review_ops::run_check(state, caller, run)` with `run` `consistency` or `inference` (policy rows `Mapping` and `ConsistencyRun`). Order: `authorize_write`; `bad_status` or, for inference on a base with the setting off, `inference_off` (no row); the target read (an unknown mapping is 404, a status equal to the mapping's is 409 `already_decided` with no row); the policy row, which must give `AgentOutcome::Applied`; `guard` (HTTP 429 `agent_rate_limited`, nothing changed, no row); the store call. `token_agent_actions::decide_mapping(pool: &PgPool, w: &TokenWrite, mapping_id: Uuid, status: &str) -> AppResult<TokenAgentAction>` writes the mapping's new status, the `applied` ledger row and the audit row `mapping.decided` in one transaction; a run writes its `applied` row (`consistency.check` or `inference.run`) and its audit row (`consistency.checked` or `inference.materialized`) right after the run succeeds, and a failed row fails the call. Every such audit row has the owner as actor and `token_id`, `token_name` and `action_id` in its detail. The answer is the person's body. The kind-word route `POST /api/v1/kbs/{id}/review/alignment/kind-words/{kind_word}` keeps its two checks, its status, its body, `unknown_class`, `alignment_busy` and its plain 403; for a token caller `token_agent_actions::decide_kind_word(pool: &PgPool, w: &TokenWrite, kind_word: &str, type_id: Option<Uuid>, votes: &serde_json::Value) -> AppResult<Option<TokenAgentAction>>` writes the decision and one `applied` `alignment.kind_word` row in one transaction, the audit row `alignment.kind_word_decided` gains `token_name` and `action_id`, and `guard` is never called. MCP tools `review_mapping_list` (`status`, `q`, `limit`, `offset`), `review_mapping` (`id`, `status`), `review_check` (`run`) and `review_kind_word` (`kind_word`, `class`), the last through `review_ops::decide_kind_word(state, caller, kind_word, class)`, which checks `authorize_write` first so a refused caller gets the coded tool error.

## Intent coverage

- SC3
- SC4

## Acceptance criteria

1. E13 (held-out; ruling: a mapping decision equal to the mapping's current status answers `already_decided` and writes no ledger row; a different status applies and writes one `applied` row; an unknown mapping id is not found; a status other than confirmed or rejected keeps `bad_status`): all four cases hold over REST and over `review_mapping`, and the audit row `mapping.decided` names the owner, the token id, the token name and the action.
2. E14 (held-out; ruling: each consistency check and each inference run by a token writes one `applied` ledger row whose target is the base and returns the report a person gets; the run is today's code with no new lock; inference on a base with the setting off answers `inference_off` and writes no ledger row): both runs hold this over REST and over `review_check`.
3. E15 (held-out; ruling: mapping decisions, consistency checks and inference runs count toward the 200 finalized writes per token and base in a rolling hour; the call after the 200th answers `agent_rate_limited` with HTTP 429, changes nothing and writes no `applied` row): each of the three is refused so after 200 seeded rows, and its own rows are counted.
4. E16 (held-out; ruling: the kind-word REST route keeps its status, response body, `unknown_class` and `alignment_busy` codes and its plain 403 for a below-editor owner; a token call now also writes one `applied` ledger row and the token name in its audit detail; it is never refused by the rate limit, and its rows count toward the token's hourly total): all of it holds, and a session kind-word decision writes no ledger row.
5. A read-scope token whose owner can view the base reads the mappings a person sees over the list route and `review_mapping_list`; `mappings` is still `unknown_queue` on the shared list.
6. `review_mapping`, `review_check` and `review_kind_word` return the REST bodies, are listed only for a write token of an editor, and a refused caller gets the coded tool error; each finalized action has an Agent History row and an Activity row reading "owner via token name".

## Verify

```bash
test -s crates/utopia-store/tests/a_token_finalize_writes_its_row_with_the_change.rs && test -s crates/utopia-server/src/api/review_token_mapping_tests.rs && test -s crates/utopia-server/src/api/review_token_run_tests.rs && test -s crates/utopia-server/src/api/review_token_kind_word_tests.rs && docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m004_t007_store WITH (FORCE)" -c "CREATE DATABASE m004_t007_store" -c "DROP DATABASE IF EXISTS m004_t007 WITH (FORCE)" -c "CREATE DATABASE m004_t007" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m004_t007_store && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-store --test a_token_finalize_writes_its_row_with_the_change -- a_token_mapping_decision_and_its_applied_row_are_one_transaction a_token_kind_word_decision_and_its_applied_row_are_one_transaction 2>&1 | tee /dev/stderr | grep -c -E "(a_token_mapping_decision_and_its_applied_row_are_one_transaction|a_token_kind_word_decision_and_its_applied_row_are_one_transaction) \.\.\. ok" | grep -qx 2 && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-store --test a_token_proposal_waits_for_a_person --test a_token_merge_is_gated_recorded_and_undone --test a_token_keep_is_never_human_precedent 2>&1 | tee /dev/stderr | grep -c -E "test result: ok\. [1-9]" | grep -qx 3 && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m004_t007 && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- a_token_mapping_decision_applies_at_once_with_one_applied_row a_mapping_decision_equal_to_the_status_answers_already_decided_and_writes_no_row a_token_reads_the_mappings_a_person_sees a_token_consistency_check_and_inference_run_each_write_one_applied_row inference_on_a_base_with_the_setting_off_answers_inference_off_and_writes_no_row the_call_after_the_200th_finalized_write_answers_429_and_changes_nothing a_token_kind_word_decision_keeps_its_answers_and_gains_one_applied_row a_kind_word_decision_is_never_refused_by_the_limit_and_counts_toward_it the_mcp_finalize_tools_answer_the_rest_bodies_and_refuse_with_the_rest_codes the_mcp_endpoint_offers_exactly_the_review_tools_the_ruled_table_opens 2>&1 | tee /dev/stderr | grep -c -E "(a_token_mapping_decision_applies_at_once_with_one_applied_row|a_mapping_decision_equal_to_the_status_answers_already_decided_and_writes_no_row|a_token_reads_the_mappings_a_person_sees|a_token_consistency_check_and_inference_run_each_write_one_applied_row|inference_on_a_base_with_the_setting_off_answers_inference_off_and_writes_no_row|the_call_after_the_200th_finalized_write_answers_429_and_changes_nothing|a_token_kind_word_decision_keeps_its_answers_and_gains_one_applied_row|a_kind_word_decision_is_never_refused_by_the_limit_and_counts_toward_it|the_mcp_finalize_tools_answer_the_rest_bodies_and_refuse_with_the_rest_codes|the_mcp_endpoint_offers_exactly_the_review_tools_the_ruled_table_opens) \.\.\. ok" | grep -qx 10 && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- review_session_unchanged_tests:: review_proposal_answer_tests:: review_token_reads_tests:: review_mcp_reads_tests:: review_mcp_tests:: review_totals_tests:: review_token_api_tests:: review_token_merge_tests:: review_token_revert_tests:: review_token_batch_tests:: review_token_guard_tests:: review_routes::attribution_tests:: review_routes::conflict_tests:: review_routes::phrase_tests:: review_token_defect_tests:: review_token_fact_tests:: review_token_conflict_tests:: review_token_violation_tests:: review_token_errata_tests:: review_token_alignment_tests:: review_token_mapping_tests:: review_item_proposals_tests:: review_token_ontology_tests:: review_token_run_tests:: review_token_kind_word_tests:: 2>&1 | tee /dev/stderr | grep -c -E "test result: ok\. [1-9]" | grep -qvx 0
```
<!-- Runs in a fresh sidecar worktree at the landed commit with nothing installed. It creates and migrates its own database(s) on the compose Postgres (`utopia-db-1`, 127.0.0.1:1517). The orchestrator's environment supplies an empty `CARGO_TARGET_DIR` outside the worktree, PostgreSQL 16 client tools and the `tee` shim first on PATH. The leading `test -s` checks name the test files this task owns. -->

Heavy: yes

## Log

- 2026-11-20 — created by planner
- 2026-11-20 — coder build_T007: preflight at base `d7d0ddc`, clean. Every path in Context, Approach and the Interface contract exists or is declared in `files`; SC3 and SC4 exist in INTENT.md; T001 to T006 signatures used as landed. Early checks from the brief, all inside `files`, no NEEDS-ORCHESTRATOR: (1) mapping: the person's `mapping.decided` audit row is written in `review_routes.rs` and the status in `mappings.rs`, so the token path's status, ledger row and audit row fit in one transaction in `token_agent_actions.rs`; (2) runs: `reasoning::run` / `check_ontology` / `materialize` are called unchanged and the ledger and audit rows are written by `review_ops.rs`; (3) kind word: `type_bindings::decide_and_apply_human_tx`, `busy_on_lock_timeout`, `normalize` and `jobs::enqueue_unless_queued_tx` are already public, so `type_bindings.rs` and `review_targets.rs` are not edited, and the `alignment.kind_word_decided` audit row is written by the handler. No existing signature changed (`mappings::decide` keeps its own; grep of callers incl. `tests/` and `*_tests.rs`). No test outside `files` asserts a token refusal on the mappings list, the mapping decision, the two runs, or an exact tool list (the one exact list is in `review_mcp_reads_tests.rs`, in `files`).
- 2026-11-20 — coder build_T007: built.
  - `crates/utopia-store/src/mappings.rs`: `decide_on(conn, …)` holds the UPDATE; `decide(pool, …)` keeps its signature and calls it on a pooled connection.
  - `crates/utopia-store/src/token_agent_actions.rs`: `decide_mapping(pool, w, mapping_id, status)` (policy row `Mapping` must finalize; `bad_status`; label read; then one transaction: row lock, `NotFound` / `already_decided` on an equal status, `mappings::decide_on`, `record_applied`, audit `mapping.decided` with `token_audit_detail`) and `decide_kind_word(pool, w, kind_word, type_id, votes)` (policy row `AlignmentKindWord`; one transaction with the person's 2 s lock timeout: `decide_and_apply_human_tx`, the `align_phrases` job, `record_applied`; lock timeout maps to `alignment_busy`; `None` when the person's code reports nothing written). Neither reads the rate limit.
  - `crates/utopia-server/src/api/review_ops.rs`: `mappings` (token read), `decide_mapping`, `run_check`, `decide_kind_word` (MCP entry, `authorize_write` first) and `kind_word_decision` (shared with the REST token branch, no `guard`). Order in `decide_mapping` and `run_check` is the contract's: `authorize_write`; `bad_status` / `unknown_run` / `inference_off`; `target_open`; policy row must be `Applied`; `guard`; store call or run. A run writes its `applied` row and its audit row in one transaction right after the run returns; a failure there fails the call.
  - `crates/utopia-server/src/api/review_routes.rs`: `decide_mapping`, `run_consistency_check`, `run_inference` take `SessionOrToken`, token branch returns first. Session statements moved unchanged into `consistency_run`, `inference_on`, `inference_run` and `kind_word_class`, which the token path also calls, so the report and the refusals are the person's by construction. Kind-word handler: its two checks stay first for both callers; only then a token goes to `review_ops::kind_word_decision`.
  - `crates/utopia-server/src/api/mapping_routes.rs`: `list` takes `SessionOrToken`; the body moved into `page_body`.
  - `crates/utopia-server/src/api/review_tools.rs`: `review_mapping_list` (read, `{"readOnlyHint":true}`, listed for every token that reaches the base), `review_mapping`, `review_check`, `review_kind_word` (write tools, finalize annotations).
  - `crates/utopia-server/src/api/mod.rs`: `review_token_run_tests` and `review_token_kind_word_tests` declared; two route comments.
  - Tests: new store file `a_token_finalize_writes_its_row_with_the_change.rs` (2 tests); `review_token_mapping_tests.rs` replaced (3 tests; `a_token_is_still_refused_on_the_mapping_route` removed); new `review_token_run_tests.rs` (3) and `review_token_kind_word_tests.rs` (3); `review_mcp_reads_tests.rs` gains the four names in the expected lists and the three write names in the `writes` expression, nothing else. Seeds are direct inserts; no model job.
  - No migration, no dependency, no `AgentOperation` / `AppError` variant, no item struct field; `CURRENT_SCHEMA_VERSION` untouched.
- 2026-11-20 — coder build_T007: carried item settled (kind-word label). `token_agent_actions::decide_kind_word` stores `request: {"kind_word": <the word as sent>, "class": <the body's class>}`, so `review_targets::target_label` returns the word. Asserted not empty and equal to the word in the store test (ledger payload and Agent History row) and in `a_token_kind_word_decision_keeps_its_answers_and_gains_one_applied_row` (Agent History row over HTTP). `review_targets.rs` is unchanged.
- 2026-11-20 — coder build_T007: readings taken where the contract is silent (none changes a contract shape).
  1. The token answer of all three finalized operations is exactly the person's body (`{"ok": true}` for a mapping and a kind word, the report for a run); it carries no `outcome` or `action_id`. The ledger row is found through Agent History.
  2. Ledger payloads: mapping `{request: {status}, target_label, action: <status>}`; run `{request: {}, target_label: <base name>}` (the routes have no body; the MCP `run` argument is not stored); kind word `{request: {kind_word, class}, target_label, action: bind | none}`. Kind-word `target_key` is `type_bindings::normalize(word)`; `target_id` is the base id.
  3. `review_check` with a `run` other than `consistency` or `inference` is refused with a new code `unknown_run` (after the write check); REST cannot reach it.
  4. The kind-word audit row stays best-effort after the commit, as it is today (`let _ = audit::record`); only its detail grew. The ledger row is in the decision's transaction. The mapping and run audit rows are in the same transaction as their ledger row.
  5. A person's kind-word decision is an upsert, so a word the base has not listed is recorded as a new binding for a token as for a person (existing `token_setup_tests` rely on it); the 404 remains only for the store reporting nothing written. The tool description says so.
  6. The store functions re-check the policy row and `bad_status` themselves, so a caller other than `review_ops` cannot finalize past the table.
  7. `guard` is called with `AgentOperation::Mapping` / `ConsistencyRun`: the limit applies, the breaker is not read (it concerns merges only).
- 2026-11-20 — coder build_T007: commands, primary worktree, Git Bash, `CARGO_TARGET_DIR=C:/Users/Owner/.gsd-path/cargo-target/coder-m004`, own databases `m004_t007_dev_store` and `m004_t007_dev` (created, migrated with `cargo run -p utopia-store --example migrate`, both "数据库迁移完成"), `UTOPIA_TEST_REQUIRE_DB=1`. The four test files passed on their first run; no test was changed after a failure.
  - `cargo test -p utopia-store --test a_token_finalize_writes_its_row_with_the_change -- <the two Verify names>` → both `... ok`; `test result: ok. 2 passed; 0 failed`.
  - `cargo test -p utopia-store --test a_token_proposal_waits_for_a_person --test a_token_merge_is_gated_recorded_and_undone --test a_token_keep_is_never_human_precedent` → `ok. 8 passed`, `ok. 10 passed`, `ok. 6 passed` (three `test result: ok. [1-9]` lines).
  - Store tests my store change can affect: `cargo test -p utopia-store --test store -- a_mapping_is_not_a_fact a_kind_word_binds_to_a_class` → `ok. 6 passed; 355 filtered out`; `cargo test -p utopia-store --lib` → `ok. 102 passed; 1 ignored`. The rest of the store suite was not run.
  - `cargo test -p utopia-server -- <the ten Verify names>` → `test result: ok. 10 passed; 0 failed; 949 filtered out`; each of the ten filters matched exactly one mounted test (`api::review_token_mapping_tests::` 3, `api::review_token_run_tests::` 3, `api::review_token_kind_word_tests::` 3, `api::review_mcp_reads_tests::` 1). The two store filters match the two tests of the store file.
  - `cargo test -p utopia-server` (whole crate, after `cargo fmt`) → `test result: ok. 957 passed; 0 failed; 2 ignored` (949 before, minus the removed 401 test, plus nine new). `chat_empty_reply_tests::gathering_errors_cannot_spoof_the_private_handoff` passed in this run. No test outside this task's files failed.
  - `cargo fmt --all --check` → first run reported diffs in four of this task's files; `cargo fmt --all` changed only those; check exit 0.
  - `cargo clippy --workspace --all-targets -- -D warnings` → exit 0.
  - The Verify command itself was not run (it names the orchestrator's databases). Its module-filter step was not run separately; the whole-crate run covers those modules, and the two new module filters each match three tests.
- 2026-11-20 — coder build_T007: deviations from the brief or the interface contract: none. Not tested: a run whose ledger or audit write fails after the run succeeded (the code path returns the error; no test forces it); two concurrent runs of one base by a token (no lock was added, as ruled); the `None` return of `decide_kind_word` (the person's upsert always writes); a token of a non-member owner on the mappings list (covered only by the shared `authorize_read`, tested for an excluded base). The web surface (Agent History and Activity rows reading "owner via token name") is covered here only by the ledger and audit data those views read; the browser walkthrough is left to final review.
- 2026-11-20 — orchestrator Verify (sidecar gsd-path-verify/task-t007-verify): pass, exit 0; output tail:
  ```
  test api::review_totals_tests::a_token_reading_duplicates_receives_the_same_total_as_a_session ... ok
  test api::review_token_reads_tests::a_token_receives_each_sections_items_in_the_session_order ... ok
  test api::review_totals_tests::a_session_list_response_differs_from_before_only_by_the_added_total_key ... ok
  test api::review_token_run_tests::a_token_consistency_check_and_inference_run_each_write_one_applied_row ... ok
  test api::review_token_revert_tests::a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided ... ok
  test api::review_token_revert_tests::review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall ... ok
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
  
  test result: ok. 140 passed; 0 failed; 0 ignored; 0 measured; 819 filtered out; finished in 30.80s
  ```
- 2026-11-20 — orchestrator regression Verify of T001 (sidecar gsd-path-verify/task-t007-verify): pass, exit 0; output tail:
  ```
  test api::review_token_reads_tests::agent_history_without_a_token_action_reads_governor_rows_only_and_a_ledger_row_keeps_its_stored_token_name ... ok
  test api::review_token_reads_tests::a_token_paging_alignment_with_equal_timestamps_visits_each_waiting_item_exactly_once ... ok
  test api::review_totals_tests::a_session_list_response_differs_from_before_only_by_the_added_total_key ... ok
  test api::review_token_reads_tests::a_token_asking_for_mappings_or_an_unknown_section_is_refused_with_unknown_queue ... ok
  test api::review_totals_tests::the_alignment_low_confidence_and_agent_totals_follow_the_list_while_counts_keep_their_values ... ok
  test api::review_token_reads_tests::a_dead_token_gets_401_and_an_excluded_token_gets_404_on_the_api_and_403_on_the_mcp_endpoint ... ok
  test api::review_totals_tests::alignment_items_with_one_timestamp_are_each_listed_once_across_pages ... ok
  test api::review_totals_tests::a_token_reading_duplicates_receives_the_same_total_as_a_session ... ok
  test api::review_totals_tests::an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total ... ok
  test api::review_totals_tests::the_seed_puts_items_in_every_one_of_the_thirteen_sections ... ok
  test api::review_routes::attribution_tests::agent_history_merges_ledger_rows_by_action_time_and_keeps_governor_order ... ok
  test api::review_token_reads_tests::a_read_token_of_a_viewer_lists_each_of_the_thirteen_sections_as_a_session_editor_sees_them ... ok
  test api::review_token_reads_tests::a_token_receives_each_sections_items_in_the_session_order ... ok
  test api::review_totals_tests::the_total_of_every_list_queue_is_the_number_of_rows_its_pages_return ... ok
  test api::review_token_reads_tests::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values ... ok
  test api::review_token_reads_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read ... ok
  test api::review_token_reads_tests::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items ... ok
  
  test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 933 filtered out; finished in 14.30s
  ```
- 2026-11-20 — orchestrator regression Verify of T002 (sidecar gsd-path-verify/task-t007-verify): pass, exit 0; output tail:
  ```
  test api::review_totals_tests::a_token_reading_duplicates_receives_the_same_total_as_a_session ... ok
  test api::review_totals_tests::the_alignment_low_confidence_and_agent_totals_follow_the_list_while_counts_keep_their_values ... ok
  test api::review_totals_tests::alignment_items_with_one_timestamp_are_each_listed_once_across_pages ... ok
  test api::review_token_revert_tests::review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall ... ok
  test api::review_token_reads_tests::a_token_receives_each_sections_items_in_the_session_order ... ok
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
  
  test result: ok. 124 passed; 0 failed; 0 ignored; 0 measured; 835 filtered out; finished in 28.68s
  ```
- 2026-11-20 — orchestrator regression Verify of T003 (sidecar gsd-path-verify/task-t007-verify): pass, exit 0; output tail:
  ```
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:21152) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:40884) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:39152) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:21992) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:35076) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:35828) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:38160) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  [33m
  (!) Some chunks are larger than 500 kB after minification. Consider:
  - Using dynamic import() to code-split the application
  - Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
  - Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.[39m
  ```
- 2026-11-20 — orchestrator regression Verify of T004 (sidecar gsd-path-verify/task-t007-verify): pass, exit 0; output tail:
  ```
  test api::review_totals_tests::a_token_reading_duplicates_receives_the_same_total_as_a_session ... ok
  test api::review_totals_tests::alignment_items_with_one_timestamp_are_each_listed_once_across_pages ... ok
  test api::review_totals_tests::the_alignment_low_confidence_and_agent_totals_follow_the_list_while_counts_keep_their_values ... ok
  test api::review_token_reads_tests::a_token_receives_each_sections_items_in_the_session_order ... ok
  test api::review_token_revert_tests::a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided ... ok
  test api::review_token_revert_tests::review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall ... ok
  test api::review_token_merge_tests::each_of_the_six_hold_reasons_lands_a_proposal_that_names_it_for_the_queue_merge_and_the_manual_merge ... ok
  test api::review_totals_tests::an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total ... ok
  test api::review_token_reads_tests::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values ... ok
  test api::review_totals_tests::the_seed_puts_items_in_every_one_of_the_thirteen_sections ... ok
  test api::review_totals_tests::the_total_of_every_list_queue_is_the_number_of_rows_its_pages_return ... ok
  test api::review_token_revert_tests::a_token_reverts_any_token_agents_merge_of_the_base_and_an_unknown_or_foreign_merge_id_is_not_found ... ok
  test api::review_token_revert_tests::after_a_token_merge_and_its_revert_every_fact_entity_and_evidence_row_is_as_before ... ok
  test api::review_token_reads_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read ... ok
  test api::review_token_merge_tests::of_a_person_the_governor_and_a_token_deciding_one_pair_exactly_one_is_applied_and_the_losing_token_gets_already_decided ... ok
  test api::review_token_violation_tests::a_token_violation_decision_waits_as_a_proposal_and_a_person_applies_it ... ok
  test api::review_token_reads_tests::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items ... ok
  
  test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 832 filtered out; finished in 29.05s
  ```
- 2026-11-20 — orchestrator regression Verify of T005 (sidecar gsd-path-verify/task-t007-verify): pass, exit 0; output tail:
  ```
  test api::review_totals_tests::alignment_items_with_one_timestamp_are_each_listed_once_across_pages ... ok
  test api::review_token_revert_tests::a_token_revert_of_a_persons_or_the_governors_merge_lands_a_proposal_and_reverts_nothing ... ok
  test api::review_totals_tests::the_alignment_low_confidence_and_agent_totals_follow_the_list_while_counts_keep_their_values ... ok
  test api::review_token_reads_tests::a_token_receives_each_sections_items_in_the_session_order ... ok
  test api::review_token_revert_tests::review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall ... ok
  test api::review_totals_tests::an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total ... ok
  test api::review_token_revert_tests::a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided ... ok
  test api::review_token_merge_tests::each_of_the_six_hold_reasons_lands_a_proposal_that_names_it_for_the_queue_merge_and_the_manual_merge ... ok
  test api::review_token_reads_tests::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values ... ok
  test api::review_totals_tests::the_seed_puts_items_in_every_one_of_the_thirteen_sections ... ok
  test api::review_token_revert_tests::a_token_reverts_any_token_agents_merge_of_the_base_and_an_unknown_or_foreign_merge_id_is_not_found ... ok
  test api::review_totals_tests::the_total_of_every_list_queue_is_the_number_of_rows_its_pages_return ... ok
  test api::review_token_revert_tests::after_a_token_merge_and_its_revert_every_fact_entity_and_evidence_row_is_as_before ... ok
  test api::review_token_reads_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read ... ok
  test api::review_token_merge_tests::of_a_person_the_governor_and_a_token_deciding_one_pair_exactly_one_is_applied_and_the_losing_token_gets_already_decided ... ok
  test api::review_token_violation_tests::a_token_violation_decision_waits_as_a_proposal_and_a_person_applies_it ... ok
  test api::review_token_reads_tests::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items ... ok
  
  test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 832 filtered out; finished in 29.03s
  ```
- 2026-11-20 — orchestrator regression Verify of T006 (sidecar gsd-path-verify/task-t007-verify): pass, exit 0; output tail:
  ```
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:32464) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:33284) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:14252) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:18652) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:39732) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:8312) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:40288) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  [33m
  (!) Some chunks are larger than 500 kB after minification. Consider:
  - Using dynamic import() to code-split the application
  - Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
  - Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.[39m
  ```
