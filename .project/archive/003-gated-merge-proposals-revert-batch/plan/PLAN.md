# Plan — utopia M003 gated-merge-proposals-revert-batch

<!-- Written by the planner role. Executed wave-by-wave by /gsd-path-build. -->

Project verify: `docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m003_project WITH (FORCE)" -c "CREATE DATABASE m003_project" -c "DROP DATABASE IF EXISTS m003_project_store WITH (FORCE)" -c "CREATE DATABASE m003_project_store" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m003_project && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test --workspace --exclude utopia-store && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m003_project_store && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-store && (cd web && pnpm install --frozen-lockfile && pnpm test && pnpm build)`
<!-- Run once at ship, in a fresh sidecar worktree checked out at the
     reviewed commit. It creates its own databases `m003_project` and
     `m003_project_store` on the compose `db` container (`utopia-db-1`,
     pgvector/pgvector:pg16, published on 127.0.0.1:1517), migrates each, runs
     every crate except `utopia-store` against the first and the `utopia-store`
     crate against the second, all with database-backed tests required, then
     installs, tests and builds the web app (the two-database form of
     `.project/LESSONS.md`). Host preconditions, supplied by the orchestrator's
     environment and not written in the command: an empty `CARGO_TARGET_DIR`
     outside the worktree, PostgreSQL 16 client tools (`pg_dump`, `pg_restore`)
     and the `tee` shim on PATH. A task Verify must not copy this command. -->

## Config

- max_review_cycles: 3
- wave_budget: none
- review_panel: off
- finding_skeptics: off

## Wave 1 — risk burn-down

Goal: Prove in the store, before any route opens, the five assumptions the write surface stands on (T001). (1) The ledger holds everything this milestone writes, by migration 0129 with `CURRENT_SCHEMA_VERSION` bumped to 115: gate reason, batch id, merge link, later state, answerer, and one open proposal per base, operation and target. (2) A token merge is atomic with its ledger and audit rows, a follow-up failure after the merge is reported on an applied result, and the person's `merge_entities` path is unchanged. (3) The gate answers the two new holds, a pair a person kept and a pair with a reverted merge, by entity ids. (4) A token merge followed by its revert returns facts, entities and evidence to their state under the comparison of E12. (5) The precedent rule of E25 holds in every governor read. If any of the five fails the task stops and the write surface is redesigned before wave 2.
Review depth: full

## Wave 2 — the walking skeleton

Goal: One gated merge end to end and its undo. A write token of an editor merges a duplicate pair, or two entities, over the token API and the MCP tool `review_merge`, behind the gate; the same call on a held pair changes nothing and lands one proposal (T002). The token then reverts a token agent's merge over the token API and the MCP tool `review_revert`, and its revert of a person's or the governor's merge waits as a proposal (T003). The wave opens write routes to tokens, so it is reviewed in full.
Review depth: full

## Wave 3 — the person's answer

Goal: A signed-in editor answers proposals, one or many, through session-only routes and in the Agent History section, sees the open proposal on the Duplicates and Merges cards, and reverts an agent merge that the Merges section shows as owner via token name; no token can answer a proposal (T004).
Review depth: full

## Wave 4 — batch, guards, documentation

Goal: A token batch of up to 50 duplicate pairs with a batch id and a unit revert, and every agent action attributed in Agent History and Activity (T005); the limit of 200 finalized writes per rolling hour, the per-token breaker, and the proof that a signed-in person's merge, batch and revert and the governor's precedent are unchanged (T006); the API and MCP documentation and the amended ADR 0076 (T007).
Review depth: full
<!-- INTENT.md records no request for adversarial review, so `deep` is not
     allowed for any wave. -->

## Surface contract

### token-authenticated HTTP API — T002

Criteria: SC1, SC2
Entry: `POST /api/v1/kbs/{kb_id}/review/{review_id}` with `{"action":"merge"}` and `POST /api/v1/kbs/{kb_id}/entities/merge` with `{"source","target"}`, each sent with `Authorization: Bearer utp_pat_…`
States: empty — a review id with no pending pair answers `404`, or `409` with code `already_decided` when the pair was decided before; loading — none, each call is one JSON response; error — `403` with `token_read_only` or `token_owner_below_editor`, `422` with `self_merge`, `404` for an unknown id, `401` for a dead token; success — `200` with `"outcome":"applied"` and a `merge_id` when the gate passes, or `200` with `"outcome":"held"`, an `action_id` and a `reason` when the gate holds, and `"code":"already_proposed"` on a repeat.
Walkthrough:
1. Run a build of the reviewed commit against its own migrated database on the compose instance; sign in, create a base, and seed it with `psql -v ON_ERROR_STOP=1 -c "SET seed.kb = 'KB_ID'" -f scripts/seed-review-sections.sql postgres://utopia:utopia@127.0.0.1:1517/DATABASE` (options before the connection string). Issue a `write` token as a base Editor and a `read` token.
2. `curl` `GET /api/v1/kbs/{kb_id}/review?queue=duplicates` with the write token and pick two pending pairs. As the signed-in editor, keep the first pair in the Review tab.
3. With the write token, `curl -X POST` `{"action":"merge"}` to the second pair: a pair the gate passes answers `"outcome":"applied"` with a `merge_id`, and `GET …/review?queue=merges` lists that merge with `agent` naming the owner and the token; a seeded pair the gate holds answers `"outcome":"held"` with its `reason` instead (then repeat the step on another pair until one applies).
4. With the write token, `curl -X POST` to `/entities/merge` the two entity ids of the pair kept in step 2: the answer is `"outcome":"held"` with `"reason":"kept_by_person"`; both entities are still listed and `GET …/review?queue=agent` shows one open proposal row with the owner's and the token's name. Send the same call again: the same `action_id` comes back with `"code":"already_proposed"` and Agent History still holds one row for it.
5. Send the merge of step 3 with the read token: `403` with `token_read_only`, and nothing new appears in Agent History.

### MCP endpoint — T003

Criteria: SC5
Entry: `POST /api/v1/kbs/{kb_id}/mcp` (JSON-RPC 2.0) with a personal token; tools `review_merge` (`id`, or `source` and `target`) and `review_revert` (`merge_id`)
States: empty — `review_revert` with a merge id the base does not have is a tool error with code `not_found`; loading — none, each `tools/call` is one JSON-RPC reply; error — a tool result with `isError: true` whose `structuredContent.code` is `already_decided`, `not_found`, `token_read_only` or `token_owner_below_editor`, and JSON-RPC error -32601 for a tool name that does not exist; success — `structuredContent` with `"outcome":"applied"` and the `merge_id`, equal to the token API's body for the same call.
Walkthrough:
1. On the running build and base of the HTTP API walkthrough, `curl` a `tools/list` request with the write token: `review_merge` and `review_revert` are listed and the description of `review_revert` says what a revert does not recall; with the read token neither is listed.
2. `curl` a `tools/call` of `review_merge` with the `id` of a pending pair the gate passes: `"outcome":"applied"` with a `merge_id`. Note the two entities with `GET /api/v1/kbs/{kb_id}/review?queue=merges`.
3. `curl` a `tools/call` of `review_revert` with that `merge_id`: `"outcome":"applied"`; the merges list shows the merge as reverted and both entities are back.
4. Call `review_revert` with the same `merge_id` again: a tool error with code `already_decided`.
5. Revert a merge with the token API route `POST /api/v1/kbs/{kb_id}/merges/{merge_id}/revert` after making it with `review_merge`: the same body as the tool returns.

### Review tab in the web app (Agent History, Duplicates and Merges sections) — T004

Criteria: SC3, SC7
Entry: the Review tab of a base in the web app, signed in as an Editor: the sections Agent History, Duplicates and Merges
States: empty — Agent History with no agent row shows its existing empty state and no answer control; loading — the sections show their existing loading state and no control until rows arrive; error — an answer that comes back `already_decided` or `superseded` shows that state on the row and the row loses its controls; success — an approved proposal shows the state label Approved, the pair leaves Duplicates and the merge appears in Merges as made by the person; a declined one shows Declined and the pair stays.
Walkthrough:
1. On the running build and base of the HTTP API walkthrough, land three proposals with the write token (held merges as in step 4 of that walkthrough, on three pairs). Open the Review tab as the signed-in Editor.
2. Open Agent History: each proposal is a row reading owner via token name with its operation label, its reason label and the controls Approve and Decline; no code is shown raw. Open Duplicates: the card of a pair with an open queue-merge proposal shows the agent's proposal chip beside the governor's.
3. Click Approve on one row: the row shows Approved, the pair is gone from Duplicates, and Merges lists the merge as made by the signed-in person, not by the token.
4. Click Decline on the second row: the row shows Declined and nothing else changes. Select several open rows and use "Approve selected" (land more proposals first if needed): each selected row shows its new state after one request.
5. Make a merge with the write token (step 3 of the HTTP API walkthrough). Open Merges: its row reads owner via token name. Click Revert on it: the row shows as reverted, both entities are back in the graph, and its Agent History row shows Undone.

## Intent coverage

| Criterion | Task | Acceptance |
|-----------|------|------------|
| SC1 | T002 | AC1 |
| SC2 | T002 | AC2 |
| SC3 | T004 | AC1 |
| SC4 | T004 | AC2 |
| SC5 | T003 | AC1 |
| SC6 | T004 | AC3 |
| SC7 | T004 | AC4 |
| SC8 | T005 | AC1 |
| SC9 | T006 | AC1 |
| SC10 | T005 | AC2 |
| SC11 | T006 | AC2 |

## Held-out checks

| Edge | Task | Test |
|------|------|------|
| E1 | T002 | crates/utopia-server/src/api/review_token_merge_tests.rs::a_token_merge_of_a_pair_that_is_already_decided_answers_already_decided_and_writes_no_second_row |
| E2 | T002 | crates/utopia-server/src/api/review_token_merge_tests.rs::of_a_person_the_governor_and_a_token_deciding_one_pair_exactly_one_is_applied_and_the_losing_token_gets_already_decided |
| E3 | T002 | crates/utopia-server/src/api/review_token_merge_tests.rs::a_token_manual_merge_of_an_entity_with_itself_a_merged_one_or_an_unknown_one_is_refused_and_lands_nothing |
| E4 | T002 | crates/utopia-server/src/api/review_token_merge_tests.rs::a_token_queue_merge_takes_the_persons_direction_and_a_manual_merge_takes_the_callers_and_closes_the_pairs_review |
| E5 | T002 | crates/utopia-server/src/api/review_token_merge_tests.rs::each_of_the_six_hold_reasons_lands_a_proposal_that_names_it_for_the_queue_merge_and_the_manual_merge |
| E6 | T002 | crates/utopia-server/src/api/review_token_merge_tests.rs::a_repeated_or_simultaneous_held_merge_returns_the_one_open_proposal_with_already_proposed |
| E8 | T004 | crates/utopia-server/src/api/review_proposal_answer_tests.rs::answering_needs_a_signed_in_editor_and_an_answered_proposal_answers_already_decided |
| E9 | T004 | crates/utopia-server/src/api/review_proposal_answer_tests.rs::approving_a_proposal_whose_target_can_no_longer_be_decided_closes_it_superseded_and_merges_nothing |
| E10 | T004 | crates/utopia-server/src/api/review_proposal_answer_tests.rs::a_bulk_answer_takes_1_to_500_ids_with_one_outcome_each_and_refuses_0_or_501_as_a_whole |
| E11 | T004 | crates/utopia-server/src/api/review_proposal_answer_tests.rs::every_token_gets_401_on_the_proposal_answer_and_mcp_offers_no_tool_that_answers_one |
| E12 | T003 | crates/utopia-server/src/api/review_token_revert_tests.rs::after_a_token_merge_and_its_revert_every_fact_entity_and_evidence_row_is_as_before |
| E14 | T003 | crates/utopia-server/src/api/review_token_revert_tests.rs::a_token_reverts_any_token_agents_merge_of_the_base_and_an_unknown_or_foreign_merge_id_is_not_found |
| E15 | T004 | crates/utopia-server/src/api/review_proposal_answer_tests.rs::a_token_revert_of_a_persons_or_the_governors_merge_is_a_proposal_that_moves_no_fuse_until_a_person_approves_it |
| E16 | T004 | crates/utopia-server/src/api/review_proposal_answer_tests.rs::after_a_person_reverts_an_agent_merge_its_ledger_row_reads_undone_and_a_token_revert_answers_already_decided |
| E17 | T004 | crates/utopia-server/src/api/review_proposal_answer_tests.rs::a_token_agents_merge_is_listed_as_owner_via_token_name_and_every_other_merge_row_is_unchanged |
| E18 | T005 | crates/utopia-server/src/api/review_token_batch_tests.rs::a_token_batch_of_0_or_51_ids_or_of_another_action_is_refused_as_a_whole_and_writes_nothing |
| E19 | T005 | crates/utopia-server/src/api/review_token_batch_tests.rs::a_token_batch_answers_one_outcome_per_id_in_request_order_with_its_code |
| E20 | T005 | crates/utopia-server/src/api/review_token_batch_tests.rs::a_unit_revert_reverts_each_unreverted_merge_of_the_batch_once_and_leaves_keeps |
| E21 | T006 | crates/utopia-server/src/api/review_token_guard_tests.rs::the_200th_finalized_write_applies_and_the_next_answers_agent_rate_limited_with_429 |
| E22 | T006 | crates/utopia-server/src/api/review_token_guard_tests.rs::the_second_revert_by_a_person_within_seven_days_makes_the_tokens_merge_and_revert_proposals_and_its_keep_still_finalizes |
| E23 | T005 | crates/utopia-server/src/api/review_token_batch_tests.rs::every_token_merge_revert_batch_member_and_proposal_reads_as_owner_via_token_name_in_agent_history_and_activity |
| E24 | T006 | crates/utopia-server/src/api/review_session_unchanged_tests.rs::a_session_editors_manual_merge_revert_and_500_id_batch_answer_as_before |
| E25 | T006 | crates/utopia-server/src/api/review_token_guard_tests.rs::a_token_merge_and_a_token_reverted_token_merge_move_no_governor_read_and_a_persons_revert_of_one_counts_as_any_revert |

## Dependency notes

- T001 has no dependency and is alone in wave 1: migration, gate, merge core and precedent all pass through four store modules, so a second wave-1 task would collide with it. It owns no criterion; its proofs are the wave-1 blockers of the synthesis.
- T002 → T001: T002 calls `token_agent_actions::merge_duplicate_pair` and `merge_entity_pair` with `TokenWrite` and reads `TokenWriteOutcome`, and its tests read the ledger columns, reason codes and audit rows; all are named in T001's Interface contract. No file is shared.
- T003 → T002: both edit `review_routes.rs`, `review_ops.rs`, `review_tools.rs`, `crates/utopia-server/src/api/mod.rs`, `review_token_api_tests.rs` and `review_mcp_reads_tests.rs`, and T003's tests make merges through `review_ops::merge_pair` and `review_merge`, named in T002's Interface contract. T003 also consumes T001's `token_agent_actions::revert_merge` through T002's dependency.
- T004 → T003: T004 answers the proposals T002 and T003 land (the answers of the merge and revert routes are named in their Interface contracts) and its tests call the token revert route. It shares `review_routes.rs` and `crates/utopia-server/src/api/mod.rs` with T003, and `token_agent_actions.rs` and `resolution.rs` with T001, so T001's and T003's Verifies also run at T004's landing.
- T005 → T004: T005 edits `review_routes.rs`, `crates/utopia-server/src/api/mod.rs`, `token_agent_actions.rs`, `models.rs`, `TokenAgentRow.tsx`, `TokenAgentRow.test.ts`, `api.ts`, `en.ts` and `zh.ts` after T004, and its batch members run through `review_ops::merge_pair` and `review_ops::revert_merge` (T002, T003, reached through T004).
- T006 → T005: T006 edits `review_ops.rs`, `review_tools.rs`, `crates/utopia-server/src/api/mod.rs` and `token_agent_actions.rs` after T005 and wraps the single-item paths that `review_ops::batch`, named in T005's Interface contract, calls per member.
- T007 → T002, T003, T004, T005, T006: T007 documents the routes, tools, keys and codes named in those tasks' Interface contracts, as they landed. It shares no file with any task.
- No two tasks can run concurrently: T002 to T006 form one chain over `review_routes.rs`, `review_ops.rs`, `review_tools.rs` and `crates/utopia-server/src/api/mod.rs`, and T007 waits on all of them.
- Lesson 1 (`.project/LESSONS.md`, 002: refusal tests belong to the opening task). The mapper's list is placed as follows: `review_token_api_tests.rs` in T002, T003 and T005; `review_token_reads_tests.rs` in T003 and T005; `review_mcp_reads_tests.rs` in T002, T003 and T005; `web/src/pages/TokenAgentRow.test.ts` in T004 and T005. Each Approach says which assertions change. Where two tasks lift different refusals in one test function the earlier task leaves the later task's assertions intact: T002 leaves the revert and batch 401, T003 leaves the batch 401. The planner's search found three more pinned assertions: `crates/utopia-store/tests/a_token_keep_is_never_human_precedent.rs::replaying_migration_0128_changes_nothing` (exact column list of `token_agent_actions`, lines 341 to 356) and `::the_compiled_table_holds_the_owners_ruling` (982, calls `agent_policy` with `Hold` values), both in T001's files; and `review_attribution_tests.rs::agent_history_shows_a_token_keep_beside_unchanged_governor_rows` (163; exact key set of a keep's ledger row, lines 204 to 227), which stays true because the new ledger-row keys are present only when they have a value (T001 Interface contract) and so is in no task's files.
- Lesson 2 (001/002: no `git diff --quiet` against a base on a declared file). No Verify compares a file with a base. Unchanged session behaviour is shown by session tests run by name or by module (`review_session_unchanged_tests`), in T002, T003, T004, T005 and T006.
- Lesson 3 (002: an earlier Verify must stay true after later landings). The three legacy refusal tests are renamed once, by the first task that lifts one of their refusals, to a name that stays true for the whole milestone (`a_token_refused_on_merge_manual_merge_revert_or_batch_leaves_the_graph_equal`, `the_mcp_endpoint_offers_exactly_the_review_tools_the_ruled_table_opens`, `a_token_cannot_nod_and_every_review_write_outside_the_ruled_table_answers_401`); later tasks change assertions inside them and keep the names, and no task renames or removes a test function an earlier Verify names. Verifies count only named tests and otherwise require that a whole test module passes, so no Verify pins a tool list, a row count, a label set or a refusal that a later task changes. T001's column check is "contains", not "equals". With that, no task needs `supersedes` and none is declared.
- Build recovery at `f1cd650` (INTENT.md C1, driver D116): T001 proved that the unchanged merge copies a duplicate typed fact's statement links onto the surviving fact and the unchanged revert leaves them. The E12 comparison in T001, T003 and T004 was corrected to match C1, the `review_revert` description sentence shared by T002, T003, T005 and T007 gained the leftover statement links, and T007 documents them. No task, dependency, wave or file list changed.
- Docs-audit queue: the eight queued fix-doc rulings are declined for this milestone (driver D113); T007 absorbs none.
