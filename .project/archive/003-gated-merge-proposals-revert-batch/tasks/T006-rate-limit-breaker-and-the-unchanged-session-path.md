---
id: T006
title: Finalized token writes are limited to 200 per rolling hour, two reverts by a person make a token propose only, and a signed-in person's merge, batch and revert and the governor's precedent are proven unchanged
wave: 4
deps: [T005]
status: done
agent: build_T006
base: b012c8cd83957cedb126e6751648739d925a2d7a
worktree: null
task_branch: null
files:
  - crates/utopia-core/src/error.rs
  - crates/utopia-server/src/error.rs
  - crates/utopia-server/src/api/review_ops.rs
  - crates/utopia-server/src/api/review_tools.rs
  - crates/utopia-server/src/api/mod.rs
  - crates/utopia-store/src/token_agent_actions.rs
  - crates/utopia-server/src/api/review_token_guard_tests.rs
  - crates/utopia-server/src/api/review_session_unchanged_tests.rs
---

# T006 — Finalized token writes are limited to 200 per rolling hour, two reverts by a person make a token propose only, and a signed-in person's merge, batch and revert and the governor's precedent are proven unchanged

## Context

With merge, revert and batch open, two guards bound what one token can finalize, and the milestone's standing promise is checked on the finished write surface. Ground truth at `6c52ce8`: no rate limiter exists and no `AppError` variant renders 429 (`crates/utopia-core/src/error.rs`; the mapping is the exhaustive match in `crates/utopia-server/src/error.rs`, with the test `conflict_codes_preserve_other_error_mappings`). The governor's fuse constants are `governance::FUSE_WINDOW_DAYS` (7) and `governance::FUSE_REVERTS` (2). The ledger tells everything both guards need (T001): `applied` rows per token and base with `created_at`; a merge made by a token agent; whether a reverted merge was reverted by a token (an `applied` `merge.revert` row names it) or by a person (none does), and when (`entity_merges.reverted_at`). `TokenWrite.breaker` already turns a merge or a revert into a proposal with reason `breaker` in the store. `review_session_unchanged_tests.rs` pins a signed-in editor's Duplicates list, Overview, keep, merge and batch answers in `a_session_editor_gets_todays_duplicates_overview_keep_merge_and_batch_responses` (233); nothing yet pins the session manual merge, the session revert or the 500-id cap after the extractor changes of T002, T003 and T005.

## Approach

- Error: add `AppError::RateLimited { code: &'static str, message: String }` in `utopia-core` and render it as HTTP 429 with the code envelope in `utopia-server`; extend `conflict_codes_preserve_other_error_mappings` with it (an added case, no changed one). The MCP refusal mapping in `review_tools.rs` must carry `agent_rate_limited` for it.
- Store: `finalized_since(pool, kb_id, token_id, since)` counts `applied` rows (keep, merge, manual merge, revert); `breaker_tripped(pool, kb_id, token_id)` counts merges made by that token on that base that were reverted by a person within the window, at least `FUSE_REVERTS`. Reverts made by a token and reverts of another token's merges do not count; when the window has passed the token finalizes again.
- One place: put both checks in one private function of `review_ops` that `keep`, `merge_pair`, `merge_entities` and `revert_merge` all call after `authorize_write` and before the store function. The limit refuses with `agent_rate_limited`; the breaker only sets `TokenWrite.breaker` for merge and revert, a keep by a tripped token still finalizes. Proposals and refused calls write no `applied` row and so do not count. Inside a batch every member past the limit gets `agent_rate_limited` as its own outcome because each member runs the same function.
- Tests in `crates/utopia-server/src/api/review_token_guard_tests.rs` (declare it in `crates/utopia-server/src/api/mod.rs`). Reach the limit by inserting 199 `applied` ledger rows directly, then call; trip the breaker by two session reverts of that token's merges, and show the window by backdating `entity_merges.reverted_at`. The precedent test (E25) makes a token merge and a token-reverted token merge over the API and compares `governance::precedents_for`, `type_pair_stats`, `decided_before`, `gate` and `settled_by_people` and the queued govern jobs before and after; then a person reverts a token merge and `type_pair_stats` counts it.
- Session proof (E24): add the test `a_session_editors_manual_merge_revert_and_500_id_batch_answer_as_before` to `crates/utopia-server/src/api/review_session_unchanged_tests.rs` (manual merge answer and audit row, revert answer and audit row, 500 ids accepted and 501 refused with `batch_size`, no ledger row for any of it). The existing test in that file is not edited.
- N10 (judgment prohibition, SC9). Must not: let a rate-limited or breaker-tripped token finalize through another entry. Detail: the reviewer checks that the limit and the breaker sit in the shared operation layer and so hold for the queue merge, the manual merge, batch members and the MCP tools alike.
- N12 is a criterion prohibition of SC11: nothing here changes what a signed-in person's merge, 500-id batch or revert does, and no token action steers the governor.
- Conventions (bind this task): Chinese explanatory comments; sentence-named tests; English user-facing strings, each refusal with a stable code; one behaviour-sentence commit subject; DCO sign-off is deferred to the upstream submission; nothing is sent to `deeplethe/utopia` or a deployed instance. Tests seed through store functions or direct inserts, never through model jobs.

## Interface contract

- **Ledger (delivered by T001).** Migration `migrations/0129_a_token_agent_action_keeps_its_reason_its_batch_its_merge_and_its_later_state.sql` adds to `token_agent_actions`, all nullable: `gate_reason TEXT`, `batch_id UUID`, `merge_id UUID`, `target_key TEXT`, `state TEXT` (checked: `approved`, `declined`, `superseded`, `undone`), `state_at TIMESTAMPTZ`, `state_by UUID`; and the partial unique index `token_agent_actions_open_proposal_idx` on `(kb_id, operation, target_key)` where `outcome IN ('held','proposed') AND state IS NULL`. `CURRENT_SCHEMA_VERSION` is 115. The ledger `operation` and the audit `action` share one name: `review.keep`; `review.merge` (`target_kind` `review`, `target_id` and `target_key` the review id); `merge.manual` (`target_kind` `entity`, `target_id` the target entity, `target_key` the two entity ids in ascending order joined by `|`); `merge.revert` (`target_kind` `merge`, `target_id` and `target_key` the merge id). `outcome` is `applied`, `held` (a gate hold) or `proposed` (propose only). `gate_reason` is one of `identity`, `contradiction`, `derived`, `answered`, `kept_by_person`, `reverted_merge` (with `held`) or `merge_by_person`, `merge_by_governor`, `breaker` (with `proposed`). `merge_id` on an `applied` merge row is the merge it made, on a `merge.revert` row the merge it targets, on an `approved` merge proposal the merge the person's approval made. `payload` holds the full request. An open proposal is a row with outcome `held` or `proposed` and `state` null. `approved`, `declined` and `superseded` are stored by the answer route with `state_at` and `state_by` (the answering person). `undone` is never stored: an `applied` merge row reads `state` `undone`, with `state_at` the revert time, whenever `entity_merges.reverted_at` of its `merge_id` is set, so a person's revert through today's unchanged handler shows it with no write. A merge made by a token agent is a merge named by `merge_id` of a row with operation `review.merge` or `merge.manual` and outcome `applied`; a merge reverted by a token is one named by an `applied` `merge.revert` row; every other reverted merge was reverted by a person or the governor's fuse.
- **Audit rows of token writes (delivered by T001).** Written in the transaction of the ledger row, `actor_id` the token's owner, detail carrying `token_id`, `token_name`, `action_id` (the ledger row), `merge_id` when a merge is named and `batch_id` when the call is a batch member: `review.merge`, `merge.manual` and `merge.revert` for an applied call (target the review, the target entity, the merge), and `agent.proposal` (`target_kind` `agent_action`, target the ledger row, detail also `operation`, `outcome`, `reason`) for a held or propose-only call.
- **Store functions (delivered by T001; no later task changes a signature).** In `utopia_store::execution_gate`: `Hold` gains the variants `KeptByPerson` and `RevertedMerge` and the method `Hold::code(&self) -> &'static str` returning the `gate_reason` code; `hold_for_token_merge(pool: &PgPool, kb_id: Uuid, a: Uuid, b: Uuid) -> AppResult<Option<Hold>>` asks `hold_for_merge` first, then the kept-pair lookup, then the reverted-merge lookup, both by entity ids in either order. `hold_for_merge`, `agent_ruling` and the signature of `agent_policy` are unchanged, and `hold_for_merge` never returns a new variant. In `utopia_store::token_agent_actions`: `pub struct TokenWrite { pub kb_id: Uuid, pub owner_id: Uuid, pub token_id: Uuid, pub rationale: Option<String>, pub batch_id: Option<Uuid>, pub breaker: bool }`; `pub struct TokenWriteOutcome { pub action: TokenAgentAction, pub existing: bool, pub follow_up_error: Option<String> }` (`existing` is true when the call met an open proposal and wrote nothing; `follow_up_error` is set when the merge committed and a later follow-up step failed); `merge_duplicate_pair(pool: &PgPool, w: &TokenWrite, review_id: Uuid) -> AppResult<TokenWriteOutcome>`, `merge_entity_pair(pool: &PgPool, w: &TokenWrite, source: Uuid, target: Uuid) -> AppResult<TokenWriteOutcome>` and `revert_merge(pool: &PgPool, w: &TokenWrite, merge_id: Uuid) -> AppResult<TokenWriteOutcome>`. Each asks the gate or the propose-only rule itself before it writes; with `breaker` true a merge or a revert lands `proposed` with reason `breaker`. Errors: `AppError::NotFound` (unknown id or another base); `AppError::CodedConflict` with code `already_decided` (decided pair, lost race, entity already merged away, merge already reverted); `AppError::Invalid` with code `self_merge`; the uncoded `AppError::Conflict` that `keep_duplicate_pair` returns for a pair a running governor round holds. `TokenAgentAction` gains `gate_reason`, `batch_id`, `merge_id`, `state`, `state_at`, `state_by`; an Agent History ledger row gains the keys `reason`, `batch_id`, `merge_id`, `state`, `state_at`, each present only when it has a value, so the row of a keep has exactly the keys it has today. `resolution::merge_entities` and `resolution::revert_merge` keep their signatures and behaviour. `governance::type_pair_stats` no longer counts, as reverted, a merge made by a token agent and reverted by a token.
- **Token merge over REST (delivered by T002).** `POST /api/v1/kbs/{kb_id}/review/{review_id}` with body `{"action":"merge","rationale":"…"}` and `POST /api/v1/kbs/{kb_id}/entities/merge` with body `{"source":"…","target":"…","rationale":"…"}` take `SessionOrToken`; `rationale` is optional. A token caller goes to `review_ops::merge_pair(state, caller, review_id, rationale, batch_id)` or `review_ops::merge_entities(state, caller, source, target, rationale)`, both `pub(crate) async` and returning `Result<serde_json::Value, AppError>`. Answers, HTTP 200: applied `{"ok":true,"outcome":"applied","action_id":…,"merge_id":…}` plus `"follow_up_error"` (a string) only when a follow-up step failed after the merge; held or propose only `{"ok":true,"outcome":"held"|"proposed","action_id":…,"reason":…,"explanation":…}` plus `"code":"already_proposed"` only when the call met the open proposal, whose id `action_id` then is. Refusals: 403 `token_read_only`, 403 `token_owner_below_editor` (scope checked first), 404, 409 `already_decided`, 409 with no code for a pair a running governor round holds, 422 `self_merge`, 403 `token_operation_not_open` for an action other than `keep` or `merge` on the decision route. A session caller of either route runs today's code and gets today's answers.
- **Token revert over REST (delivered by T003).** `POST /api/v1/kbs/{kb_id}/merges/{merge_id}/revert` takes `SessionOrToken`; a token may send `{"rationale":"…"}` or no body. A token caller goes to `review_ops::revert_merge(state, caller, merge_id, rationale, batch_id)` (`pub(crate) async`, `Result<serde_json::Value, AppError>`). Answers, HTTP 200: applied `{"ok":true,"outcome":"applied","action_id":…,"merge_id":…}` when a token agent made the merge; `{"ok":true,"outcome":"proposed","action_id":…,"reason":"merge_by_person"|"merge_by_governor"|"breaker","explanation":…}` otherwise, plus `"code":"already_proposed"` on a repeat. Refusals: 403 `token_read_only`, 403 `token_owner_below_editor`, 404 for an unknown merge id or one of another base, 409 `already_decided` for a merge already reverted. A session caller runs today's code and gets today's answer.
- **Token batch and unit revert (delivered by T005).** `POST /api/v1/kbs/{kb_id}/review/batch` takes `SessionOrToken`. A token caller goes to `review_ops::batch(state, caller, ids, action, rationale)`: 1 to `TOKEN_BATCH_MAX` (50) ids, `action` `merge` or `keep`; otherwise 422 `batch_size` (size) or the route's existing refusal of an unknown action, as a whole, with nothing written. The server issues one `batch_id` and passes each id through the single-item token path (`review_ops::merge_pair` or `review_ops::keep`). Answer, HTTP 200: `{"batch_id":…,"decided":n,"outcomes":[{"id","outcome","action_id","merge_id","reason","code","error"}]}` in request order, one per id; `outcome` is `applied`, `held`, `proposed` or null with `code` and `error` set (`not_found`, `already_decided`, `agent_rate_limited` and so on); `decided` counts the `applied` items. A session caller runs today's code: cap 500, answer `{"decided","outcomes":[{"id","error"}]}`. `POST /api/v1/kbs/{kb_id}/review/agent-batches/{batch_id}/revert` takes `SessionOrToken`: a `write` token of an Editor (each merge through `review_ops::revert_merge`) or a signed-in Editor (each merge through the person's revert path). Answer, HTTP 200: `{"batch_id":…,"reverted":n,"outcomes":[{"merge_id","outcome","code","error"}]}`, one per merge the batch made; an already reverted merge is that item's `already_decided`; keeps are left; a batch id no ledger row of the base carries is 404.
- **Limit and breaker (delivered by T006).** `review_ops` holds `TOKEN_WRITES_PER_HOUR` (200). Before any finalizing token call (`keep`, `merge_pair`, `merge_entities`, `revert_merge`, and so each batch member) it counts the `applied` ledger rows of that token on that base in the last rolling hour with `token_agent_actions::finalized_since`; at 200 or more the call answers HTTP 429 with code `agent_rate_limited` (`AppError::RateLimited { code, message }`), changes nothing and writes no row. It then sets `TokenWrite.breaker` from `token_agent_actions::breaker_tripped(pool, kb_id, token_id)`: true when, within `governance::FUSE_WINDOW_DAYS`, at least `governance::FUSE_REVERTS` merges made by that token on that base were reverted by a person. Until T006 lands `breaker` is always false and no call is limited.

## Intent coverage

- SC9
- SC11

## Acceptance criteria

1. SC9: with 199 `applied` rows in the last hour a token's next finalizing call applies and the one after answers HTTP 429 `agent_rate_limited`, changes nothing and writes no row; after two reverts by a person of one token's merges within seven days that token's next merge on the base answers outcome `proposed` with reason `breaker`.
2. SC11: a signed-in editor's queue merge, manual merge, revert and 500-id batch answer exactly as before the milestone and write no ledger row, and a token merge changes no governor precedent read and queues no govern round.
3. E21 (held-out; ruling: the limit is 200 finalized writes per token and base in a rolling hour, a named constant; the 200th applies and the next answers `agent_rate_limited` with HTTP 429, changes nothing and writes no `applied` row; proposals and refused calls do not count; keep, merge and revert count; inside a batch every item past the limit gets `agent_rate_limited` as its own outcome).
4. E22 (held-out; ruling: the breaker trips at the second revert by a person of one token's finalized merges on a base within seven days; one such revert does not trip it; reverts made by a token and reverts of another token's merges do not count; a tripped token's merge and revert, batch merge members included, land as proposals whose reason names the breaker; its keep still finalizes; when the seven days have passed the token finalizes again).
5. E24 (held-out; ruling: the existing session tests for merge, batch and revert pass; the only edits to existing tests are the assertions that a token is refused on merge, manual merge, revert and batch, and every assertion for a route that stays closed keeps its expectation).
6. E25 (held-out; ruling: a live token merge counts in no governor precedent read; a merge made by a token and reverted by a token counts in none either; a person's revert of a token merge counts as that person's revert precedent as any revert does; no token action lowers the governor's bar, settles a pair as decided by people, or starts a govern round).
7. The limit and the breaker hold for the queue merge, the manual merge, a batch member and the MCP tools `review_merge`, `review_revert`, `review_batch` and `review_keep` alike; over MCP the refusal is a tool error with code `agent_rate_limited` (N10).
8. `AppError::RateLimited` renders 429 with its code and every other error mapping is unchanged.

## Verify

```bash
set -o pipefail && test -s crates/utopia-server/src/api/review_token_guard_tests.rs && test -s crates/utopia-server/src/api/review_session_unchanged_tests.rs && docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m003_t006 WITH (FORCE)" -c "CREATE DATABASE m003_t006" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m003_t006 && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- the_200th_finalized_write_applies_and_the_next_answers_agent_rate_limited_with_429 the_second_revert_by_a_person_within_seven_days_makes_the_tokens_merge_and_revert_proposals_and_its_keep_still_finalizes the_limit_and_the_breaker_hold_for_the_queue_merge_the_manual_merge_batch_members_and_the_mcp_tools a_token_merge_and_a_token_reverted_token_merge_move_no_governor_read_and_a_persons_revert_of_one_counts_as_any_revert a_session_editor_gets_todays_duplicates_overview_keep_merge_and_batch_responses a_session_editors_manual_merge_revert_and_500_id_batch_answer_as_before conflict_codes_preserve_other_error_mappings 2>&1 | tee /dev/stderr | grep -c -E "(the_200th_finalized_write_applies_and_the_next_answers_agent_rate_limited_with_429|the_second_revert_by_a_person_within_seven_days_makes_the_tokens_merge_and_revert_proposals_and_its_keep_still_finalizes|the_limit_and_the_breaker_hold_for_the_queue_merge_the_manual_merge_batch_members_and_the_mcp_tools|a_token_merge_and_a_token_reverted_token_merge_move_no_governor_read_and_a_persons_revert_of_one_counts_as_any_revert|a_session_editor_gets_todays_duplicates_overview_keep_merge_and_batch_responses|a_session_editors_manual_merge_revert_and_500_id_batch_answer_as_before|conflict_codes_preserve_other_error_mappings) \.\.\. ok" | grep -qx 7 && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- review_session_unchanged_tests:: review_token_merge_tests:: review_token_revert_tests:: review_token_batch_tests:: review_proposal_answer_tests:: 2>&1 | tee /dev/stderr | grep -c -E "test result: ok\. [1-9]" | grep -qvx 0
```
<!-- Runs in a fresh sidecar worktree at the landed commit with nothing installed. It creates and migrates its own database(s) on the compose Postgres (`utopia-db-1`, 127.0.0.1:1517). The orchestrator's environment supplies an empty `CARGO_TARGET_DIR` outside the worktree, PostgreSQL 16 client tools and the `tee` shim first on PATH. The leading `test -s` checks name the test files this task owns. -->

Heavy: yes

## Log

- 2026-11-19 — created by planner
- 2026-11-19 — coder build_T006: implemented, `ready`.
  - Preflight: clean. The only exhaustive `match` on `AppError` in the workspace is `crates/utopia-server/src/error.rs` (in `files`); every other match on it has a wildcard or is an `Err(AppError::X)` arm among others (`oidc_routes.rs:836`, `main.rs:484`, `token_agent_actions.rs` `lost_merge_race`/`deadlocked`/`target_gone`, the tests). `utopia-cli` does not mention `AppError`. No path outside `files` was needed or edited.
  - Error: `AppError::RateLimited { code, message }` (core) renders 429 with the code envelope (server); `conflict_codes_preserve_other_error_mappings` has one added case, none changed.
  - Store (`token_agent_actions.rs`): `finalized_since(pool, kb_id, token_id, since)` counts `applied` rows with `created_at > since`; `breaker_tripped(pool, kb_id, token_id)` counts merges made by that token on that base (`applied` `review.merge`/`merge.manual` row with that `token_id`) whose `reverted_at` is inside `governance::FUSE_WINDOW_DAYS`, reverted by a person, `>= governance::FUSE_REVERTS`. No existing signature changed.
  - `review_ops.rs`: `TOKEN_WRITES_PER_HOUR` (200) and one private `guard`, called by `keep`, `merge_pair`, `merge_entities`, `revert_merge` after `authorize_write` and before the store function. It refuses with `agent_rate_limited` at 200 or more, then returns the breaker flag for merge and revert only (keep never reads it). `token_write` takes the flag. Batch members, the unit revert and the six MCP tools reach the limit and the breaker only through those four functions.
  - `review_tools.rs`: no mapping change was needed: `refusal` already takes code and message from the rendered REST response (`review_ops::refused`), so 429 arrives as a tool error `{code: "agent_rate_limited", message}`. Added the limit and the breaker to the descriptions of `review_keep`, `review_merge`, `review_revert`, `review_batch`.
  - Tests added: `review_token_guard_tests.rs` (declared in `api/mod.rs`) with `the_200th_finalized_write_applies_and_the_next_answers_agent_rate_limited_with_429`, `the_second_revert_by_a_person_within_seven_days_makes_the_tokens_merge_and_revert_proposals_and_its_keep_still_finalizes`, `the_limit_and_the_breaker_hold_for_the_queue_merge_the_manual_merge_batch_members_and_the_mcp_tools`, `a_token_merge_and_a_token_reverted_token_merge_move_no_governor_read_and_a_persons_revert_of_one_counts_as_any_revert`; in `review_session_unchanged_tests.rs` the new `a_session_editors_manual_merge_revert_and_500_id_batch_answer_as_before` plus a private helper `merge_audits`. The existing test and `Fx` in that file are untouched. No existing test in any file was edited.
  - Reviewer observations:
    - Overshoot bound: the limit is a read before the write, not in its transaction. Calls of one token on one base that are in flight together can each read 199 and all apply; the overshoot is at most the number of such calls in flight at that moment (a batch runs its members one after another and counts as one). Later calls are refused.
    - A `breaker` revert (or merge) that meets an open proposal returns that proposal with its earlier reason: unchanged, the store's `land_proposal` returns the existing row. Related, seen while testing and not changed: when the window has passed, a token revert of a token-made merge applies while an earlier open `breaker` revert proposal on that merge stays open (T003 behaviour of `revert_merge`: the applied path does not close it).
    - Race of a person's and a token's revert of one merge: the simultaneous case stands for `type_pair_stats` (not in `files`; E25's test asserts the sequential case only). For the breaker I chose data the race cannot erase: a merge counts as reverted by a person when no `applied` `merge.revert` ledger row names it, or when an `audit_events` row `merge.revert` on that merge has an actor and no `token_id` in its detail (all three person revert paths write such a row). Asserted in the E22 test by inserting a token `merge.revert` row beside a person's revert. Limit of this choice: the person's audit row is best effort on the session path, so a race in which that audit write also fails is still not counted.
    - Attribution tests module path `api::review_routes::attribution_tests`: not touched by this task.
    - Web labels: `breaker` is already labelled (`web/src/i18n/en.ts:2173`, `zh.ts:1903`, listed in `TokenAgentRow.test.ts:88`). `agent_rate_limited` is a refusal to the token caller and writes no ledger row, so the Review tab has no row to show it on; no web file needs it. No open point.
  - Open point for the documentation task: the API and MCP documentation and ADR 0076 do not yet describe `agent_rate_limited` (429), `TOKEN_WRITES_PER_HOUR` or the breaker rule as built; no documentation path is in `files`.
  - Verify (full command, database `m003_t006`): exit 0. Part one 7 of 7 named tests `ok`; part two `test result: ok. 33 passed; 0 failed` (the five module filters, T001 to T005 tests included).
  - Whole `utopia-server` on fresh `m003_t006_server`: 914 passed, 0 failed, 2 ignored. Whole `utopia-store` on fresh `m003_t006_store`: every target ok, 0 failed (102 unit, 361 and 15 in the two large integration targets, the rest small). `cargo test -p utopia-core -p utopia-cli`: 15, 13 and 25 passed, 0 failed. `cargo check --workspace --all-targets`: ok. `cargo fmt --all --check`: clean. `cargo clippy --workspace --all-targets`: no warning, no error.
  - Guard refusals: none. Databases created: `m003_t006`, `m003_t006_server`, `m003_t006_store` (left in place). Nothing sent to a remote; no Git mutation.
- 2026-11-20 — orchestrator Verify (sidecar gsd-path-verify/task-t006-verify): pass, exit 0; output tail:
  ```
  test api::review_token_merge_tests::a_token_manual_merge_of_an_entity_with_itself_a_merged_one_or_an_unknown_one_is_refused_and_lands_nothing ... ok
  test api::review_token_batch_tests::a_unit_revert_reverts_each_unreverted_merge_of_the_batch_once_and_leaves_keeps ... ok
  test api::review_proposal_answer_tests::a_declined_proposal_changes_nothing_and_several_proposals_are_answered_in_one_request ... ok
  test api::review_token_merge_tests::a_token_merge_starts_no_govern_round_and_a_session_merge_still_asks_no_gate ... ok
  test api::review_token_batch_tests::review_batch_and_the_unit_revert_run_over_mcp_with_the_rest_answers ... ok
  test api::review_token_merge_tests::review_merge_is_listed_only_to_a_write_token_of_an_editor_and_a_refusal_carries_the_rest_code ... ok
  test api::review_token_revert_tests::a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided ... ok
  test api::review_token_merge_tests::a_token_merge_of_a_pair_that_is_already_decided_answers_already_decided_and_writes_no_second_row ... ok
  test api::review_token_merge_tests::a_token_queue_merge_takes_the_persons_direction_and_a_manual_merge_takes_the_callers_and_closes_the_pairs_review ... ok
  test api::review_proposal_answer_tests::approving_a_proposal_whose_target_can_no_longer_be_decided_closes_it_superseded_and_merges_nothing ... ok
  test api::review_proposal_answer_tests::a_token_revert_of_a_persons_or_the_governors_merge_is_a_proposal_that_moves_no_fuse_until_a_person_approves_it ... ok
  test api::review_token_revert_tests::a_token_reverts_any_token_agents_merge_of_the_base_and_an_unknown_or_foreign_merge_id_is_not_found ... ok
  test api::review_token_revert_tests::review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall ... ok
  test api::review_token_revert_tests::after_a_token_merge_and_its_revert_every_fact_entity_and_evidence_row_is_as_before ... ok
  test api::review_proposal_answer_tests::a_bulk_answer_takes_1_to_500_ids_with_one_outcome_each_and_refuses_0_or_501_as_a_whole ... ok
  test api::review_token_merge_tests::each_of_the_six_hold_reasons_lands_a_proposal_that_names_it_for_the_queue_merge_and_the_manual_merge ... ok
  test api::review_token_merge_tests::of_a_person_the_governor_and_a_token_deciding_one_pair_exactly_one_is_applied_and_the_losing_token_gets_already_decided ... ok
  
  test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 883 filtered out; finished in 8.25s
  ```
- 2026-11-20 — orchestrator regression Verify of T001 (sidecar gsd-path-verify/task-t006-verify): pass, exit 0; output tail:
  ```
  test a_token_keep_changes_no_precedent_no_settlement_and_queues_no_govern_round ... ok
  test a_persons_keep_still_counts_beside_a_token_keep ... ok
  test a_token_keep_with_no_token_to_name_changes_nothing ... ok
  
  test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.70s
  
     Compiling utopia-store v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\verify\task-t006-verify\crates\utopia-store)
      Finished `test` profile [unoptimized + debuginfo] target(s) in 22.25s
       Running tests\store\main.rs (C:/Users/Owner/.gsd-path/cargo-target/finish-m003-T006\debug\deps\store-97773e81a9073f97.exe)
  
  running 6 tests
  test a_name_is_a_fact::merging_moves_names_and_reverting_brings_them_back ... ok
  test merge_direction_counts_live_facts_in_each_base::merge_direction_counts_live_facts_in_each_base ... ok
  test a_timeline_holds_whatever_the_order::a_revert_restores_both_timelines_a_merge_rewrote ... ok
  test history_shows_the_merge_itself::history_carries_the_merge_and_its_direction ... ok
  test an_automatic_merge_is_gated_by_what_it_can_undo::a_merge_that_would_leave_the_graph_is_held ... ok
  test a_signature_holds_on_every_path::a_merge_that_breaks_the_signature_is_recorded ... ok
  
  test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 355 filtered out; finished in 0.91s
  ```
- 2026-11-20 — orchestrator regression Verify of T002 (sidecar gsd-path-verify/task-t006-verify): pass, exit 0; output tail:
  ```
  test api::review_mcp_reads_tests::review_item_defaults_to_duplicates_and_finds_a_kind_word_by_its_exact_word_and_an_agent_row_by_its_row_id ... ok
  test api::review_token_api_tests::an_empty_duplicates_section_reads_the_same_for_a_token_and_a_session ... ok
  test api::review_token_api_tests::a_token_keep_over_the_api_starts_no_govern_round ... ok
  test api::review_mcp_reads_tests::review_item_refuses_mappings_and_an_unknown_section_with_unknown_queue ... ok
  test api::review_token_api_tests::a_write_token_of_an_editor_keeps_a_pair_over_the_api ... ok
  test api::review_token_api_tests::a_keep_over_the_api_is_refused_with_its_code ... ok
  test api::review_token_api_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_review_reads ... ok
  test api::review_session_unchanged_tests::a_session_editor_gets_todays_duplicates_overview_keep_merge_and_batch_responses ... ok
  test api::review_mcp_tests::two_keeps_of_one_pair_at_once_apply_exactly_one ... ok
  test api::review_token_api_tests::a_read_token_of_a_viewer_lists_duplicates_and_overview_as_a_session_editor_sees_them ... ok
  test api::review_token_api_tests::a_token_refused_on_merge_manual_merge_revert_or_batch_leaves_the_graph_equal ... ok
  test api::review_session_unchanged_tests::a_session_editors_manual_merge_revert_and_500_id_batch_answer_as_before ... ok
  test api::review_token_api_tests::a_token_receives_duplicates_in_the_session_order ... ok
  test api::review_mcp_reads_tests::review_list_returns_what_the_token_api_returns_for_each_of_the_thirteen_sections ... ok
  test api::review_mcp_reads_tests::review_item_returns_the_listed_items_own_fields_for_one_item_of_each_section ... ok
  test api::review_mcp_reads_tests::review_list_pages_decisions_by_page_and_per_and_the_queue_sections_by_limit_and_offset ... ok
  test api::review_mcp_reads_tests::review_item_answers_one_not_found_for_an_unknown_id_another_bases_id_and_an_item_no_longer_waiting ... ok
  
  test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 885 filtered out; finished in 5.96s
  ```
- 2026-11-20 — orchestrator regression Verify of T003 (sidecar gsd-path-verify/task-t006-verify): pass, exit 0; output tail:
  ```
  test api::review_token_reads_tests::a_token_cannot_nod_and_every_review_write_outside_the_ruled_table_answers_401 ... ok
  test api::review_token_reads_tests::a_token_paging_alignment_by_the_total_reaches_the_last_implication_rule ... ok
  test api::review_token_reads_tests::agent_history_read_by_a_token_names_the_owner_and_the_token_of_a_keep_beside_governor_rows ... ok
  test api::review_mcp_reads_tests::review_item_refuses_mappings_and_an_unknown_section_with_unknown_queue ... ok
  test api::review_token_reads_tests::a_read_token_of_a_viewer_lists_each_of_the_thirteen_sections_as_a_session_editor_sees_them ... ok
  test api::review_token_reads_tests::a_token_asking_for_mappings_or_an_unknown_section_is_refused_with_unknown_queue ... ok
  test api::review_token_reads_tests::a_token_paging_alignment_with_equal_timestamps_visits_each_waiting_item_exactly_once ... ok
  test api::review_token_reads_tests::an_empty_section_reads_as_an_empty_list_with_total_0_for_a_token_as_for_a_session ... ok
  test api::review_token_reads_tests::agent_history_without_a_token_action_reads_governor_rows_only_and_a_ledger_row_keeps_its_stored_token_name ... ok
  test api::review_mcp_reads_tests::review_list_returns_what_the_token_api_returns_for_each_of_the_thirteen_sections ... ok
  test api::review_token_reads_tests::a_token_receives_each_sections_items_in_the_session_order ... ok
  test api::review_mcp_reads_tests::review_item_returns_the_listed_items_own_fields_for_one_item_of_each_section ... ok
  test api::review_token_reads_tests::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values ... ok
  test api::review_mcp_reads_tests::review_list_pages_decisions_by_page_and_per_and_the_queue_sections_by_limit_and_offset ... ok
  test api::review_token_reads_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read ... ok
  test api::review_mcp_reads_tests::review_item_answers_one_not_found_for_an_unknown_id_another_bases_id_and_an_item_no_longer_waiting ... ok
  test api::review_token_reads_tests::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items ... ok
  
  test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 884 filtered out; finished in 16.45s
  ```
- 2026-11-20 — orchestrator regression Verify of T004 (sidecar gsd-path-verify/task-t006-verify): pass, exit 0; output tail:
  ```
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:532) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:9084) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:37296) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:30864) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:6556) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:19760) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:13024) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  [33m
  (!) Some chunks are larger than 500 kB after minification. Consider:
  - Using dynamic import() to code-split the application
  - Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
  - Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.[39m
  ```
