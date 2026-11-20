---
id: T005
title: A write token of an editor batches up to 50 duplicate pairs under one batch id, the batch is reverted as a unit, and every agent action reads as owner via token name with its batch id in Agent History and Activity
wave: 4
deps: [T004]
status: pending
agent: null
base: null
worktree: null
task_branch: null
files:
  - crates/utopia-server/src/api/review_routes.rs
  - crates/utopia-server/src/api/review_ops.rs
  - crates/utopia-server/src/api/review_tools.rs
  - crates/utopia-server/src/api/mod.rs
  - crates/utopia-store/src/token_agent_actions.rs
  - crates/utopia-store/src/audit.rs
  - crates/utopia-core/src/models.rs
  - crates/utopia-server/src/api/review_token_batch_tests.rs
  - crates/utopia-server/src/api/review_token_revert_tests.rs
  - crates/utopia-server/src/api/review_token_api_tests.rs
  - crates/utopia-server/src/api/review_token_reads_tests.rs
  - crates/utopia-server/src/api/review_mcp_reads_tests.rs
  - web/src/pages/TokenAgentRow.tsx
  - web/src/pages/TokenAgentRow.test.ts
  - web/src/pages/KbSettings.tsx
  - web/src/pages/KbActivity.test.ts
  - web/src/api.ts
  - web/src/i18n/en.ts
  - web/src/i18n/zh.ts
---

# T005 — A write token of an editor batches up to 50 duplicate pairs under one batch id, the batch is reverted as a unit, and every agent action reads as owner via token name with its batch id in Agent History and Activity

## Context

The spec's case is a base where one name is eight entities: the agent needs to clear many pairs in one call, and a person needs to undo that call as one thing. Waves 2 and 3 delivered the single-item token paths (`review_ops::keep`, `merge_pair`, `revert_merge`), proposals and their answer. Ground truth at `6c52ce8`: `review_routes::batch` (567) takes `AuthUser`, body `BatchBody` with `ids`, `action`, `rationale`, cap `BATCH_MAX` 500 with 422 `batch_size`, sequential decisions, answer `decided` plus `outcomes` of `id` and `error`; a token gets 401. There is no batch identity anywhere. Activity is the Admin-only session route behind `audit::list_for_kb`, whose `AuditEventView` (`utopia-core/src/models.rs:439`) serializes `token_id`, `token_name` and `declared_client` only when present; `web/src/pages/KbSettings.tsx` (647 to 690) renders a token event with `viaToken` and `tokenOperationLabel`, tested in `KbActivity.test.ts`. Existing tests assert the refusal this task lifts: the batch 401 inside `review_token_api_tests.rs::a_token_refused_on_merge_manual_merge_revert_or_batch_leaves_the_graph_equal` and the `/review/batch` entry of the closed-route list in `review_token_reads_tests.rs::a_token_cannot_nod_and_every_review_write_outside_the_ruled_table_answers_401` (names set by T002 and T003; originally `review_token_api_tests.rs` 354 to 375 and `review_token_reads_tests.rs` 1097 to 1100). The exact tool list is in `review_mcp_reads_tests.rs::the_mcp_endpoint_offers_exactly_the_review_tools_the_ruled_table_opens`.

## Approach

- Batch: `batch` changes extractor to `SessionOrToken`; a token caller goes to `review_ops::batch`. It calls `authorize_write` once, refuses a wrong size or action as a whole before anything is written, issues one `batch_id` (`Uuid::now_v7`), and runs the ids in request order, each through `review_ops::merge_pair` or `review_ops::keep` with that `batch_id`, catching each item's error into its own outcome. `keep` gains the `batch_id` parameter so a kept member's ledger and audit rows carry it. An id given twice gets, the second time, the answer its state then gives. A failed ledger or audit write is that item's error, never a silent success.
- The session branch is not rewritten. In each handler whose extractor changes to `SessionOrToken`, a session caller runs the statements that are there today, in the same order, and gets the same answer; a token caller leaves the handler for `review_ops` before any of them. `resolution::revert_merge` is not changed by any task.
- Unit revert: new handler at `POST /kbs/{id}/review/agent-batches/{batch_id}/revert` taking `SessionOrToken`. It lists the batch's `applied` merge rows from the ledger (`token_agent_actions::batch_merges`); none at all for that base and id is 404. A token caller reverts each through `review_ops::revert_merge`; a signed-in Editor reverts each through the person's revert path (the statements of today's session revert, per merge). Best effort: one outcome per merge, an already reverted merge is `already_decided`, keeps are untouched. No Review tab button is added for it.
- MCP: add `review_batch`, and the `batch_id` argument of `review_revert`, to `serves`, `definitions`, `annotations`, the write filter of `listed` and `call`.
- Attribution: Agent History ledger rows already carry `batch_id` (T001). For Activity add an optional `batch_id` to `AuditEventView`, read from `detail.batch_id` in `audit::list_for_kb` and serialized only when present, so every other event is unchanged. In the web app `TokenAgentRow` and the Activity row show a short batch label when `batch_id` is present, and `tokenOperationLabel` covers `agent.proposal`, `agent.proposal.declined` and `agent.proposal.superseded`; add the `en.ts` and `zh.ts` labels. The token name shown is always the stored snapshot.
- Existing tests this task changes (lesson 1). In `review_token_api_tests.rs`, test `a_token_refused_on_merge_manual_merge_revert_or_batch_leaves_the_graph_equal`: replace the batch 401 with the refusals that remain (`token_read_only`, `token_owner_below_editor`, 422 `batch_size` for 51 ids); keep the name. In `review_token_reads_tests.rs`, test `a_token_cannot_nod_and_every_review_write_outside_the_ruled_table_answers_401`: remove only the `/review/batch` entry; the governor answer route and every other listed route keep their 401; keep the name. In `review_mcp_reads_tests.rs`, test `the_mcp_endpoint_offers_exactly_the_review_tools_the_ruled_table_opens`: add `review_batch`; keep the name. Every other assertion keeps its expectation (E24).
- New server tests go in `crates/utopia-server/src/api/review_token_batch_tests.rs` (declare it in `crates/utopia-server/src/api/mod.rs`); web tests extend `TokenAgentRow.test.ts` and `KbActivity.test.ts`, including one that fails when an operation or state written by this milestone has no label.
- N9 (judgment prohibition, SC8). Must not: give a batch member anything the single-item token path would refuse or hold. Detail: the reviewer checks that each member runs through the single-item token path, so gate, limit and breaker apply per member. Here: `review_ops::batch` contains no merge, keep or gate logic of its own; it only calls `review_ops::merge_pair` and `review_ops::keep`.
- N11 is a criterion prohibition of SC10: a token-agent action is never shown as the person's own decision or as a governor row.
- Conventions (bind this task): Chinese explanatory comments; sentence-named tests; English user-facing strings, each refusal with a stable code; one behaviour-sentence commit subject; DCO sign-off is deferred to the upstream submission; nothing is sent to `deeplethe/utopia` or a deployed instance. Tests seed through store functions or direct inserts, never through model jobs.
- File-scope repair (Delegated, driver D120): `crates/utopia-server/src/api/review_token_revert_tests.rs` is in `files` for one purpose only. Its test `review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall` pins the `review_revert` input schema as `required: ["merge_id"]` with exactly the properties `merge_id` and `rationale` (lines 1118 to 1123 at `f17d3ca`); this task adds `batch_id` and makes "exactly one of `merge_id` and `batch_id`" the rule, so only that schema assertion changes to the new shape. The test keeps its name (T003's Verify names it) and every other assertion in the file is unchanged.

## Interface contract

- **Ledger (delivered by T001).** Migration `migrations/0129_a_token_agent_action_keeps_its_reason_its_batch_its_merge_and_its_later_state.sql` adds to `token_agent_actions`, all nullable: `gate_reason TEXT`, `batch_id UUID`, `merge_id UUID`, `target_key TEXT`, `state TEXT` (checked: `approved`, `declined`, `superseded`, `undone`), `state_at TIMESTAMPTZ`, `state_by UUID`; and the partial unique index `token_agent_actions_open_proposal_idx` on `(kb_id, operation, target_key)` where `outcome IN ('held','proposed') AND state IS NULL`. `CURRENT_SCHEMA_VERSION` is 115. The ledger `operation` and the audit `action` share one name: `review.keep`; `review.merge` (`target_kind` `review`, `target_id` and `target_key` the review id); `merge.manual` (`target_kind` `entity`, `target_id` the target entity, `target_key` the two entity ids in ascending order joined by `|`); `merge.revert` (`target_kind` `merge`, `target_id` and `target_key` the merge id). `outcome` is `applied`, `held` (a gate hold) or `proposed` (propose only). `gate_reason` is one of `identity`, `contradiction`, `derived`, `answered`, `kept_by_person`, `reverted_merge` (with `held`) or `merge_by_person`, `merge_by_governor`, `breaker` (with `proposed`). `merge_id` on an `applied` merge row is the merge it made, on a `merge.revert` row the merge it targets, on an `approved` merge proposal the merge the person's approval made. `payload` holds the full request. An open proposal is a row with outcome `held` or `proposed` and `state` null. `approved`, `declined` and `superseded` are stored by the answer route with `state_at` and `state_by` (the answering person). `undone` is never stored: an `applied` merge row reads `state` `undone`, with `state_at` the revert time, whenever `entity_merges.reverted_at` of its `merge_id` is set, so a person's revert through today's unchanged handler shows it with no write. A merge made by a token agent is a merge named by `merge_id` of a row with operation `review.merge` or `merge.manual` and outcome `applied`; a merge reverted by a token is one named by an `applied` `merge.revert` row; every other reverted merge was reverted by a person or the governor's fuse.
- **Audit rows of token writes (delivered by T001).** Written in the transaction of the ledger row, `actor_id` the token's owner, detail carrying `token_id`, `token_name`, `action_id` (the ledger row), `merge_id` when a merge is named and `batch_id` when the call is a batch member: `review.merge`, `merge.manual` and `merge.revert` for an applied call (target the review, the target entity, the merge), and `agent.proposal` (`target_kind` `agent_action`, target the ledger row, detail also `operation`, `outcome`, `reason`) for a held or propose-only call.
- **Store functions (delivered by T001; no later task changes a signature).** In `utopia_store::execution_gate`: `Hold` gains the variants `KeptByPerson` and `RevertedMerge` and the method `Hold::code(&self) -> &'static str` returning the `gate_reason` code; `hold_for_token_merge(pool: &PgPool, kb_id: Uuid, a: Uuid, b: Uuid) -> AppResult<Option<Hold>>` asks `hold_for_merge` first, then the kept-pair lookup, then the reverted-merge lookup, both by entity ids in either order. `hold_for_merge`, `agent_ruling` and the signature of `agent_policy` are unchanged, and `hold_for_merge` never returns a new variant. In `utopia_store::token_agent_actions`: `pub struct TokenWrite { pub kb_id: Uuid, pub owner_id: Uuid, pub token_id: Uuid, pub rationale: Option<String>, pub batch_id: Option<Uuid>, pub breaker: bool }`; `pub struct TokenWriteOutcome { pub action: TokenAgentAction, pub existing: bool, pub follow_up_error: Option<String> }` (`existing` is true when the call met an open proposal and wrote nothing; `follow_up_error` is set when the merge committed and a later follow-up step failed); `merge_duplicate_pair(pool: &PgPool, w: &TokenWrite, review_id: Uuid) -> AppResult<TokenWriteOutcome>`, `merge_entity_pair(pool: &PgPool, w: &TokenWrite, source: Uuid, target: Uuid) -> AppResult<TokenWriteOutcome>` and `revert_merge(pool: &PgPool, w: &TokenWrite, merge_id: Uuid) -> AppResult<TokenWriteOutcome>`. Each asks the gate or the propose-only rule itself before it writes; with `breaker` true a merge or a revert lands `proposed` with reason `breaker`. Errors: `AppError::NotFound` (unknown id or another base); `AppError::CodedConflict` with code `already_decided` (decided pair, lost race, entity already merged away, merge already reverted); `AppError::Invalid` with code `self_merge`; the uncoded `AppError::Conflict` that `keep_duplicate_pair` returns for a pair a running governor round holds. `TokenAgentAction` gains `gate_reason`, `batch_id`, `merge_id`, `state`, `state_at`, `state_by`; an Agent History ledger row gains the keys `reason`, `batch_id`, `merge_id`, `state`, `state_at`, each present only when it has a value, so the row of a keep has exactly the keys it has today. `resolution::merge_entities` and `resolution::revert_merge` keep their signatures and behaviour. `governance::type_pair_stats` no longer counts, as reverted, a merge made by a token agent and reverted by a token.
- **Token merge over REST (delivered by T002).** `POST /api/v1/kbs/{kb_id}/review/{review_id}` with body `{"action":"merge","rationale":"…"}` and `POST /api/v1/kbs/{kb_id}/entities/merge` with body `{"source":"…","target":"…","rationale":"…"}` take `SessionOrToken`; `rationale` is optional. A token caller goes to `review_ops::merge_pair(state, caller, review_id, rationale, batch_id)` or `review_ops::merge_entities(state, caller, source, target, rationale)`, both `pub(crate) async` and returning `Result<serde_json::Value, AppError>`. Answers, HTTP 200: applied `{"ok":true,"outcome":"applied","action_id":…,"merge_id":…}` plus `"follow_up_error"` (a string) only when a follow-up step failed after the merge; held or propose only `{"ok":true,"outcome":"held"|"proposed","action_id":…,"reason":…,"explanation":…}` plus `"code":"already_proposed"` only when the call met the open proposal, whose id `action_id` then is. Refusals: 403 `token_read_only`, 403 `token_owner_below_editor` (scope checked first), 404, 409 `already_decided`, 409 with no code for a pair a running governor round holds, 422 `self_merge`, 403 `token_operation_not_open` for an action other than `keep` or `merge` on the decision route. A session caller of either route runs today's code and gets today's answers.
- **Token revert over REST (delivered by T003).** `POST /api/v1/kbs/{kb_id}/merges/{merge_id}/revert` takes `SessionOrToken`; a token may send `{"rationale":"…"}` or no body. A token caller goes to `review_ops::revert_merge(state, caller, merge_id, rationale, batch_id)` (`pub(crate) async`, `Result<serde_json::Value, AppError>`). Answers, HTTP 200: applied `{"ok":true,"outcome":"applied","action_id":…,"merge_id":…}` when a token agent made the merge; `{"ok":true,"outcome":"proposed","action_id":…,"reason":"merge_by_person"|"merge_by_governor"|"breaker","explanation":…}` otherwise, plus `"code":"already_proposed"` on a repeat. Refusals: 403 `token_read_only`, 403 `token_owner_below_editor`, 404 for an unknown merge id or one of another base, 409 `already_decided` for a merge already reverted. A session caller runs today's code and gets today's answer.
- **MCP write tools (file `crates/utopia-server/src/api/review_tools.rs`).** Listed in `tools/list` and callable only for a `write` token whose owner is at least Editor, each annotated `readOnlyHint: false`; a caller without that right that calls one gets a tool error with the REST code. `review_merge` (T002): `id` (a pending duplicate pair) or `source` and `target` (entity ids), optional `rationale`; `structuredContent` is the REST body of the matching merge route. `review_revert` (T003; `batch_id` added by T005): exactly one of `merge_id` and `batch_id`, optional `rationale`; `structuredContent` is the REST body of the revert route or of the unit revert; its description says that a revert restores graph state only and does not recall derivations, violations, exports or answers drawn while the merge was live, that a target that had no type before the merge keeps the type it took, and that a surviving fact keeps the statement links the merge copied onto it. `review_batch` (T005): `ids`, `action` (`merge` or `keep`), optional `rationale`; `structuredContent` is the REST body of the token batch. A refused call is a tool result with `isError: true` and `structuredContent` `{"code","message"}` carrying the REST code (`not_found` for a 404). No tool answers a proposal: a call to any other `review_` name is JSON-RPC error -32601.
- **Token batch and unit revert (delivered by T005).** `POST /api/v1/kbs/{kb_id}/review/batch` takes `SessionOrToken`. A token caller goes to `review_ops::batch(state, caller, ids, action, rationale)`: 1 to `TOKEN_BATCH_MAX` (50) ids, `action` `merge` or `keep`; otherwise 422 `batch_size` (size) or the route's existing refusal of an unknown action, as a whole, with nothing written. The server issues one `batch_id` and passes each id through the single-item token path (`review_ops::merge_pair` or `review_ops::keep`). Answer, HTTP 200: `{"batch_id":…,"decided":n,"outcomes":[{"id","outcome","action_id","merge_id","reason","code","error"}]}` in request order, one per id; `outcome` is `applied`, `held`, `proposed` or null with `code` and `error` set (`not_found`, `already_decided`, `agent_rate_limited` and so on); `decided` counts the `applied` items. A session caller runs today's code: cap 500, answer `{"decided","outcomes":[{"id","error"}]}`. `POST /api/v1/kbs/{kb_id}/review/agent-batches/{batch_id}/revert` takes `SessionOrToken`: a `write` token of an Editor (each merge through `review_ops::revert_merge`) or a signed-in Editor (each merge through the person's revert path). Answer, HTTP 200: `{"batch_id":…,"reverted":n,"outcomes":[{"merge_id","outcome","code","error"}]}`, one per merge the batch made; an already reverted merge is that item's `already_decided`; keeps are left; a batch id no ledger row of the base carries is 404.

## Intent coverage

- SC8
- SC10

## Acceptance criteria

1. SC8: a token batch of 51 ids is refused with 422 `batch_size`; a `merge` batch of passed pairs, held pairs and an unknown id answers one outcome per id with the held members as proposals and one `batch_id`, which every ledger row and audit row of the batch carries; `POST /review/agent-batches/{batch_id}/revert` then reverts every merge the batch made.
2. SC10: after a token merge, manual merge, revert, batch and held call, `GET /review?queue=agent` shows each as a row with source `token`, the owner's name and the token's name, batch members with `batch_id`; the Admin's Activity returns each audit event with `token_id` and `token_name`, batch members with `batch_id`; none appears as a governor row or without its token; the web rows render "owner via token name" and the batch label (static render).
3. E18 (held-out; ruling: a token batch takes 1 to 50 ids; 0 or 51 ids is refused as a whole with the existing `batch_size` code, nothing changes and no ledger row is written; an action other than merge or keep refuses the whole batch).
4. E19 (held-out; ruling: outcomes come back in the order of the request, one per id, each with its outcome, `applied`, `proposed` or an error, and stable code; an unknown id is that item's `not_found` and does not fail the batch; an id given twice gets, the second time, the answer its state then gives).
5. E20 (held-out; ruling: a unit revert reverts each not yet reverted merge of the batch and reports each already reverted one as `already_decided`; a second unit revert changes nothing; keeps are left as they are; an unknown batch id is not found; a write token of an editor and a signed-in editor may both call it; no Review tab button is added for it).
6. E23 (held-out; ruling: the token name shown is the snapshot stored with the action, whatever happens to the token afterwards; batch members carry the batch id in Agent History and in Activity; every operation and every state this milestone writes, `approved`, `declined`, `superseded`, `undone` included, has a label in the web app and none is shown raw; Activity is read by a signed-in base Admin).
7. `review_batch` and `review_revert` with `batch_id` return over MCP the bodies the REST routes return, and are listed only for a `write` token of an Editor.
8. The three existing tests named in the Approach pass with only the batch assertions changed; a signed-in editor's batch answers as today, cap 500 included (`review_session_unchanged_tests` passes unedited); `pnpm test` and `pnpm build` pass.

## Verify

```bash
set -o pipefail && test -s crates/utopia-server/src/api/review_token_batch_tests.rs && test -s web/src/pages/KbActivity.test.ts && docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m003_t005 WITH (FORCE)" -c "CREATE DATABASE m003_t005" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m003_t005 && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- a_mixed_token_batch_lands_gated_members_as_proposals_under_one_batch_id_and_its_unit_revert_undoes_every_merge a_token_batch_of_0_or_51_ids_or_of_another_action_is_refused_as_a_whole_and_writes_nothing a_token_batch_answers_one_outcome_per_id_in_request_order_with_its_code a_unit_revert_reverts_each_unreverted_merge_of_the_batch_once_and_leaves_keeps every_token_merge_revert_batch_member_and_proposal_reads_as_owner_via_token_name_in_agent_history_and_activity review_batch_and_the_unit_revert_run_over_mcp_with_the_rest_answers a_token_refused_on_merge_manual_merge_revert_or_batch_leaves_the_graph_equal the_mcp_endpoint_offers_exactly_the_review_tools_the_ruled_table_opens a_token_cannot_nod_and_every_review_write_outside_the_ruled_table_answers_401 2>&1 | tee /dev/stderr | grep -c -E "(a_mixed_token_batch_lands_gated_members_as_proposals_under_one_batch_id_and_its_unit_revert_undoes_every_merge|a_token_batch_of_0_or_51_ids_or_of_another_action_is_refused_as_a_whole_and_writes_nothing|a_token_batch_answers_one_outcome_per_id_in_request_order_with_its_code|a_unit_revert_reverts_each_unreverted_merge_of_the_batch_once_and_leaves_keeps|every_token_merge_revert_batch_member_and_proposal_reads_as_owner_via_token_name_in_agent_history_and_activity|review_batch_and_the_unit_revert_run_over_mcp_with_the_rest_answers|a_token_refused_on_merge_manual_merge_revert_or_batch_leaves_the_graph_equal|the_mcp_endpoint_offers_exactly_the_review_tools_the_ruled_table_opens|a_token_cannot_nod_and_every_review_write_outside_the_ruled_table_answers_401) \.\.\. ok" | grep -qx 9 && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- review_token_api_tests:: review_token_reads_tests:: review_mcp_reads_tests:: review_session_unchanged_tests:: review_routes::attribution_tests:: 2>&1 | tee /dev/stderr | grep -c -E "test result: ok\. [1-9]" | grep -qvx 0 && (cd web && pnpm install --frozen-lockfile && pnpm test && pnpm build)
```
<!-- Runs in a fresh sidecar worktree at the landed commit with nothing installed. It creates and migrates its own database(s) on the compose Postgres (`utopia-db-1`, 127.0.0.1:1517). The orchestrator's environment supplies an empty `CARGO_TARGET_DIR` outside the worktree, PostgreSQL 16 client tools and the `tee` shim first on PATH. The leading `test -s` checks name the test files this task owns. -->

Heavy: yes

## Log

- 2026-11-19 — created by planner
- 2026-11-19 — orchestrator: the Verify filter `review_attribution_tests::` matched no test (the file is mounted as `api::review_routes::attribution_tests`, `crates/utopia-server/src/api/review_routes.rs:26`); replaced with `review_routes::attribution_tests::` (wave 3 review, deviation 1).
- 2026-11-19 — coder build_T005: ready. Preflight clean (paths, SC8 and SC10 in INTENT, Interface bullet verbatim in T006 and T007; no full `AuditEventView` literal and no exact tool list outside `files`).
  - Server: `review_routes::batch` takes `SessionOrToken`; a token leaves for `review_ops::batch` (authorize once, 422 `batch_size` outside 1 to `TOKEN_BATCH_MAX` 50, the session batch's own 422 "action must be merge or keep" for another action, one `Uuid::now_v7` batch id, each id through `review_ops::merge_pair` or `review_ops::keep`, seven keys per outcome plus `follow_up_error` only when the single path reports one); the session statements are unchanged in order. New handler `review_routes::agent_batch_revert` at `POST /kbs/{id}/review/agent-batches/{batch_id}/revert` (route in `api/mod.rs`): token through `review_ops::revert_batch` (each merge through `review_ops::revert_merge` with the batch id), signed-in Editor through `revert_as_person` (the statements of the session revert per merge; its audit detail carries `agent_batch`, no `token_id`). `review_ops::keep` gained `batch_id`. Per-item `code` and `error` come from the REST rendering (`review_ops::refused`, now also used by `review_tools::refusal`), so a later error variant such as T006's 429 needs no change here.
  - Store: `keep_duplicate_pair` keeps its signature and delegates to the new `keep_duplicate_pair_in_batch` (external store tests call the old one); new `batch_merges` (None when no ledger row of the base carries the id, so a keep-only batch answers 200 with no outcomes) and `merge_already_reverted`. `AuditEventView.batch_id` is read from `detail->>'batch_id'` in `audit::list_for_kb` only and serialized when present.
  - MCP: `review_batch` added to `serves`, `definitions`, `annotations`, the write filter and `call`; `review_revert` has no `required` list and `call` enforces exactly one of `merge_id` and `batch_id`. `ids` has no `minItems`/`maxItems` so 0 and 51 ids are refused by the operation with `batch_size`.
  - Web: `batchLabel` (last eight hex digits) on `TokenAgentRow` and the Activity row; `tokenOperations` gained `agent.proposal`, `agent.proposal.declined`, `agent.proposal.superseded` in `en.ts` and `zh.ts`, plus `tokenBatch`; `activityTokenLabel` also labels a person's declined or superseded event, which Activity showed raw. `AuditEvent.batch_id` in `api.ts`. No Review tab button and no API client function for the unit revert.
  - Existing tests changed, names kept: `a_token_refused_on_merge_manual_merge_revert_or_batch_leaves_the_graph_equal` (batch 401 replaced by `token_read_only`, `token_owner_below_editor`, 422 `batch_size` for 51 ids); `a_token_cannot_nod_and_every_review_write_outside_the_ruled_table_answers_401` (only the `/review/batch` entry removed); `the_mcp_endpoint_offers_exactly_the_review_tools_the_ruled_table_opens` (`review_batch` added to the list and to the write-tool check); `review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall` (schema assertion only). New: the six tests of `review_token_batch_tests.rs`; web tests in `TokenAgentRow.test.ts` (`OPERATIONS` extended, batch label, `activityTokenLabel`) and `KbActivity.test.ts` (every token action in words, batch label, a person's answer to a proposal).
  - Reviewer observations: schema relaxed and enforced in `call` (done); no new operation, reason, outcome or state code is written by the ledger, the three audit actions above got labels and list entries (done); MCP refusals carry the REST code, and the unknown-action refusal has none on REST either, so its tool error has only `message`; two unit reverts at once: a process-wide mutex (`review_ops::unit_revert_turn`) queues unit reverts of both callers, so the second sees `already_decided` per merge. It does not cover two server processes or a single-merge revert racing a unit revert; that stands, as `resolution::revert_merge` is unchanged.
  - Noted, not changed: the MCP endpoint writes one `mcp.tool_called` audit row per tool call, refused ones included (`mcp.rs`, outside `files`); the "refused batch writes nothing" test excludes that action. A person's `review.keep` and similar rows still show the raw action in Activity as before (pinned by the existing test `leaves events without a token name exactly as they were`).
  - Verify (full command, database `m003_t005`): exit 0; 9 of 9 named tests ok; module run 36 passed, 0 failed; web 56 files, 388 tests passed; `pnpm build` ok. Whole `utopia-server` on fresh `m003_t005_server`: 909 passed, 0 failed, 2 ignored. Whole `utopia-store` on fresh `m003_t005_store`: 527 passed, 0 failed, 4 ignored. T002, T003 and T004 modules (`review_token_merge_tests`, `review_token_revert_tests`, `review_proposal_answer_tests`, `review_mcp_tests`) pass. `cargo fmt --all --check` clean; `cargo clippy` on core, store and server with all targets: 0 warnings.
  - Guard: one inline script was refused by the shell wrapper (quoting); the edits were made with the edit tool instead. An earlier inline script edited `token_agent_actions.rs`, `audit.rs` and `models.rs` before that; its result was reviewed in the diff.
- 2026-11-20 — orchestrator Verify (sidecar gsd-path-verify/task-t005-verify): pass, exit 0; output tail:
  ```
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:2700) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:39404) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:39040) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:38972) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:37172) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:34424) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:6644) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  [33m
  (!) Some chunks are larger than 500 kB after minification. Consider:
  - Using dynamic import() to code-split the application
  - Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
  - Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.[39m
  ```
- 2026-11-20 — orchestrator regression Verify of T001 (sidecar gsd-path-verify/task-t005-verify): pass, exit 0; output tail:
  ```
  test a_token_keep_writes_the_review_row_the_ledger_row_and_the_audit_row_together ... ok
  test a_second_keep_of_a_decided_pair_answers_already_decided ... ok
  test a_token_keep_with_no_token_to_name_changes_nothing ... ok
  
  test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.54s
  
     Compiling utopia-store v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\verify\task-t005-verify\crates\utopia-store)
      Finished `test` profile [unoptimized + debuginfo] target(s) in 18.77s
       Running tests\store\main.rs (C:/Users/Owner/.gsd-path/cargo-target/finish-m003-T005\debug\deps\store-97773e81a9073f97.exe)
  
  running 6 tests
  test a_name_is_a_fact::merging_moves_names_and_reverting_brings_them_back ... ok
  test merge_direction_counts_live_facts_in_each_base::merge_direction_counts_live_facts_in_each_base ... ok
  test a_timeline_holds_whatever_the_order::a_revert_restores_both_timelines_a_merge_rewrote ... ok
  test an_automatic_merge_is_gated_by_what_it_can_undo::a_merge_that_would_leave_the_graph_is_held ... ok
  test history_shows_the_merge_itself::history_carries_the_merge_and_its_direction ... ok
  test a_signature_holds_on_every_path::a_merge_that_breaks_the_signature_is_recorded ... ok
  
  test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 355 filtered out; finished in 0.91s
  ```
- 2026-11-20 — orchestrator regression Verify of T002 (sidecar gsd-path-verify/task-t005-verify): pass, exit 0; output tail:
  ```
  test api::review_mcp_tests::a_refused_keep_carries_the_same_code_over_the_api_and_over_mcp ... ok
  test api::review_token_api_tests::an_empty_duplicates_section_reads_the_same_for_a_token_and_a_session ... ok
  test api::review_mcp_reads_tests::review_item_defaults_to_duplicates_and_finds_a_kind_word_by_its_exact_word_and_an_agent_row_by_its_row_id ... ok
  test api::review_token_api_tests::a_keep_over_the_api_is_refused_with_its_code ... ok
  test api::review_token_api_tests::a_token_keep_over_the_api_starts_no_govern_round ... ok
  test api::review_token_api_tests::a_write_token_of_an_editor_keeps_a_pair_over_the_api ... ok
  test api::review_mcp_reads_tests::review_item_refuses_mappings_and_an_unknown_section_with_unknown_queue ... ok
  test api::review_token_api_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_review_reads ... ok
  test api::review_mcp_tests::two_keeps_of_one_pair_at_once_apply_exactly_one ... ok
  test api::review_session_unchanged_tests::a_session_editor_gets_todays_duplicates_overview_keep_merge_and_batch_responses ... ok
  test api::review_token_api_tests::a_token_refused_on_merge_manual_merge_revert_or_batch_leaves_the_graph_equal ... ok
  test api::review_token_api_tests::a_read_token_of_a_viewer_lists_duplicates_and_overview_as_a_session_editor_sees_them ... ok
  test api::review_token_api_tests::a_token_receives_duplicates_in_the_session_order ... ok
  test api::review_mcp_reads_tests::review_list_returns_what_the_token_api_returns_for_each_of_the_thirteen_sections ... ok
  test api::review_mcp_reads_tests::review_item_returns_the_listed_items_own_fields_for_one_item_of_each_section ... ok
  test api::review_mcp_reads_tests::review_list_pages_decisions_by_page_and_per_and_the_queue_sections_by_limit_and_offset ... ok
  test api::review_mcp_reads_tests::review_item_answers_one_not_found_for_an_unknown_id_another_bases_id_and_an_item_no_longer_waiting ... ok
  
  test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 881 filtered out; finished in 5.92s
  ```
- 2026-11-20 — orchestrator regression Verify of T003 (sidecar gsd-path-verify/task-t005-verify): pass, exit 0; output tail:
  ```
  test api::review_token_reads_tests::a_token_asking_for_mappings_or_an_unknown_section_is_refused_with_unknown_queue ... ok
  test api::review_token_reads_tests::a_token_cannot_nod_and_every_review_write_outside_the_ruled_table_answers_401 ... ok
  test api::review_token_api_tests::a_token_receives_duplicates_in_the_session_order ... ok
  test api::review_token_reads_tests::agent_history_read_by_a_token_names_the_owner_and_the_token_of_a_keep_beside_governor_rows ... ok
  test api::review_token_reads_tests::a_token_paging_alignment_with_equal_timestamps_visits_each_waiting_item_exactly_once ... ok
  test api::review_mcp_reads_tests::review_item_refuses_mappings_and_an_unknown_section_with_unknown_queue ... ok
  test api::review_token_reads_tests::a_read_token_of_a_viewer_lists_each_of_the_thirteen_sections_as_a_session_editor_sees_them ... ok
  test api::review_token_reads_tests::an_empty_section_reads_as_an_empty_list_with_total_0_for_a_token_as_for_a_session ... ok
  test api::review_token_reads_tests::agent_history_without_a_token_action_reads_governor_rows_only_and_a_ledger_row_keeps_its_stored_token_name ... ok
  test api::review_token_reads_tests::a_token_receives_each_sections_items_in_the_session_order ... ok
  test api::review_mcp_reads_tests::review_list_returns_what_the_token_api_returns_for_each_of_the_thirteen_sections ... ok
  test api::review_mcp_reads_tests::review_item_returns_the_listed_items_own_fields_for_one_item_of_each_section ... ok
  test api::review_token_reads_tests::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values ... ok
  test api::review_mcp_reads_tests::review_list_pages_decisions_by_page_and_per_and_the_queue_sections_by_limit_and_offset ... ok
  test api::review_token_reads_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read ... ok
  test api::review_mcp_reads_tests::review_item_answers_one_not_found_for_an_unknown_id_another_bases_id_and_an_item_no_longer_waiting ... ok
  test api::review_token_reads_tests::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items ... ok
  
  test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 880 filtered out; finished in 16.12s
  ```
- 2026-11-20 — orchestrator regression Verify of T004 (sidecar gsd-path-verify/task-t005-verify): pass, exit 0; output tail:
  ```
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:17876) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:792) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:6640) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:34828) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:10636) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:38744) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:30956) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  [33m
  (!) Some chunks are larger than 500 kB after minification. Consider:
  - Using dynamic import() to code-split the application
  - Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
  - Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.[39m
  ```
- 2026-11-19 — orchestrator: landing refused (the landing allow-list is read from the committed brief, and the file-scope repair D120 was made after activation). The repaired brief is checkpointed here with the task returned to pending; the coder work is held in a Git stash and restored on re-activation, then landed without a new coder run.
