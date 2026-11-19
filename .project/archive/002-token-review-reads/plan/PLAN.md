# Plan — utopia M002 token-review-reads

<!-- Written by the planner role. Executed wave-by-wave by /gsd-path-build. -->

Project verify: `docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m002_project WITH (FORCE)" -c "CREATE DATABASE m002_project" -c "DROP DATABASE IF EXISTS m002_project_store WITH (FORCE)" -c "CREATE DATABASE m002_project_store" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m002_project && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test --workspace --exclude utopia-store && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m002_project_store && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-store && (cd web && pnpm install --frozen-lockfile && pnpm test && pnpm build)`
<!-- Run once at ship, in a fresh sidecar worktree checked out at the
     reviewed commit: no node_modules, virtualenv, build output, or untracked
     env files exist there. It creates its own databases `m002_project` and
     `m002_project_store` on the compose `db` container (`utopia-db-1`,
     pgvector/pgvector:pg16, published on 127.0.0.1:1517), migrates each,
     runs every crate except `utopia-store` against the first and the
     `utopia-store` crate against the second, all with database-backed tests
     required, then installs, tests and builds the web app. This is the
     two-database form of M001's amended plan (`.project/LESSONS.md`): the
     upstream test `intake_runs_while_model_work_holds_every_slot` times out
     behind the jobs the server tests leave queued, so the store crate gets
     its own freshly migrated database. Host preconditions, supplied by the
     orchestrator's environment and not written in the command: an empty
     `CARGO_TARGET_DIR` outside the worktree, and `pg_dump` and `pg_restore`
     of PostgreSQL 16 on PATH. A task Verify must not copy this command. -->

## Config

- max_review_cycles: 3
- wave_budget: none
- review_panel: off
- finding_skeptics: off

## Wave 1 — risk burn-down

Goal: Prove the two things every token read of this milestone stands on. (1) The total: every list response of `GET /kbs/{id}/review` carries `total`, the number of rows that queue's list returns over all pages, for all twelve list queues; the three sections whose badge count disagrees with their list today (Alignment rules, Agent History rows, Low confidence derived facts) are shown with a correct total while every existing `counts` key keeps its value; the Alignment order gains an identifier tie-break; and a signed-in person's responses differ only by the added key (the existing session tests still pass, the web app still builds). If a list join drops a row its count includes, or the session shape cannot take the key without breaking the tab, the read design returns to the owner before any token branch is built on it. (2) The seed: one seeding source puts items in every one of the thirteen sections, including the expensive ones (held errata actions, proposed implication rules, a derived contradiction whose proof expands), by direct inserts and store functions. If a section cannot be seeded that way, "the same items a signed-in editor sees" cannot be shown per section and wave 2 does not start.
Review depth: full

## Wave 2 — the read surface

Goal: A read-scope token of a base Viewer lists all thirteen sections through the three handlers the Review tab uses and reads both proof routes (T002); MCP `review_list` returns the same bodies for all thirteen sections and `review_item` returns one listed item of any section, with the proof steps of a derived contradiction (T003); the read surface is documented and ADR 0076 is amended and corrected (T004). The first step of T002 is the walking skeleton named by the synthesis: one non-Duplicates queue end to end through `review_ops::list`. The wave touches authorization (the reader check on eleven newly opened queues, the Decisions history and two proof routes; the refusals for a dead token and for a token whose `kb_ids` exclude the base), so it is reviewed in full. No Review write is added.
Review depth: full
<!-- INTENT.md records no request for adversarial review, so `deep` is not
     allowed for either wave. -->

## Surface contract

### token-authenticated HTTP API — T002

Criteria: SC1, SC4, SC5, SC6
Entry: `GET /api/v1/kbs/{kb_id}/review?queue=<pending|duplicates|conflicts|unconfirmed|lowconf|violations|defects|alignment|errata|agent|merges>`, `GET /api/v1/kbs/{kb_id}/review/summary`, `GET /api/v1/kbs/{kb_id}/review/history?page=&per=`, `GET /api/v1/kbs/{kb_id}/violations/{violation_id}/proof` and `GET /api/v1/kbs/{kb_id}/derived/{derived_id}/proof`, each sent with `Authorization: Bearer utp_pat_…`
States: empty — `200` with `"items": []` and `"total": 0` for a queue with nothing in it, and `{"events": [], "total": 0}` for Decisions, the same as a signed-in editor gets; loading — none, each call is one JSON response; error — `401` for a dead or revoked token, `404` when the token's `kb_ids` exclude the base (the same body as for a base that does not exist), `422` with code `unknown_queue` for `mappings` or any name that is not a list queue; success — `200` with `{"counts","queue","items","total"}` for a queue, the Overview summary object, `{"events","total"}` for Decisions, `{"steps": …}` and `{"proof": …}` for the two proof reads.
Walkthrough:
1. Run a build of the reviewed commit against its own migrated database on the compose instance; sign in, create a base, and seed it with `psql postgres://utopia:utopia@127.0.0.1:1517/<that database> -v ON_ERROR_STOP=1 -c "SET seed.kb = '<kb id>'" -f scripts/seed-review-sections.sql`. Issue a `read` token as a base Viewer, a `write` token as a base Editor, and a `read` token whose base list excludes this base.
2. With the write token, `curl -X POST` `{"action":"keep"}` to one pair of the Duplicates list (the M001 keep), so Agent History and Decisions hold a token row.
3. With the read token, `curl` each of the eleven queue URLs, the summary and the history; `curl` the same URLs with the signed-in editor's session and compare `items` (and `events`) one section at a time: they are equal, and every queue response carries `total`.
4. Read `total` from `?queue=alignment`, then `curl` `?queue=alignment&limit=1&offset=<total − 1>`: one item comes back. Page from offset 0 to `total − 1` with `limit=1` and see every item with `"kind": "rule"`, the last one included, each item once.
5. `curl` `?queue=agent` with the read token: the keep of step 2 is a row with `"source": "token"`, the owner's name and the token's name, beside the governor's rows, which carry no `source`.
6. Repeat the reads of step 3 and one proof read with the token whose base list excludes the base: each answers `404` with the body a made-up base id gets. Revoke the read token and repeat one read: `401`.

### MCP endpoint — T003

Criteria: SC2, SC3
Entry: `POST /api/v1/kbs/{kb_id}/mcp` (JSON-RPC 2.0) with a personal token; tools `review_list` (`section`, and `limit`/`offset` or, for `decisions`, `page`/`per`) and `review_item` (`section`, `id`)
States: empty — `review_list` returns `items: []` with `total: 0` for an empty queue and `{"events": [], "total": 0}` for Decisions; loading — none, each `tools/call` is one JSON-RPC reply; error — a tool result with `isError: true` whose `structuredContent.code` is `unknown_queue` (a section outside the thirteen, `mappings` included) or `not_found` (an id not listed in that section of this base), HTTP `401` from the endpoint for a dead token and its unchanged HTTP `403` for a token whose `kb_ids` exclude the base; success — `structuredContent` equal to the token API's body for the same section and token, and for `review_item` the listed item's own fields, with `proof` on an Axioms item.
Walkthrough:
1. On the running build and base of the HTTP API walkthrough, `curl` a `tools/list` request with the read token: `review_list` and `review_item` are listed, `review_keep` is not, and no other tool name starts with `review_`.
2. `curl` a `tools/call` of `review_list` for each of the thirteen sections (`decisions` with `page` and `per`, `overview` with no paging argument) and compare `structuredContent` with the token API's response to the same token for that section.
3. For one item of each queue section and of `decisions`, `curl` a `tools/call` of `review_item` with that `section` and the item's `id` (for an Alignment kind word, its word): the result equals the item in the list. For the Axioms item of kind `derived_contradiction` it also carries `proof`, equal to `steps` of `GET /violations/{id}/proof` with the same token; for an Axioms item of another kind `proof` is `null`.
4. Call `review_item` with a made-up id: a tool error with code `not_found` and no evidence. Call any tool with the token whose base list excludes the base: HTTP `403` and no item, count or evidence.

## Intent coverage

| Criterion | Task | Acceptance |
|-----------|------|------------|
| SC1 | T002 | AC1 |
| SC2 | T003 | AC1 |
| SC3 | T003 | AC3 |
| SC4 | T002 | AC7 |
| SC5 | T002 | AC9 |
| SC6 | T002 | AC11 |
| SC7 | T001 | AC1 |

## Held-out checks

| Edge | Task | Test |
|------|------|------|
| E1 | T002 | crates/utopia-server/src/api/review_token_reads_tests.rs::an_empty_section_reads_as_an_empty_list_with_total_0_for_a_token_as_for_a_session |
| E2 | T002 | crates/utopia-server/src/api/review_token_reads_tests.rs::a_token_receives_each_sections_items_in_the_session_order |
| E3 | T002 | crates/utopia-server/src/api/review_token_reads_tests.rs::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items |
| E4 | T002 | crates/utopia-server/src/api/review_token_reads_tests.rs::a_token_asking_for_mappings_or_an_unknown_section_is_refused_with_unknown_queue |
| E5 | T002 | crates/utopia-server/src/api/review_token_reads_tests.rs::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values |
| E6 | T003 | crates/utopia-server/src/api/review_mcp_reads_tests.rs::review_list_pages_decisions_by_page_and_per_and_the_queue_sections_by_limit_and_offset |
| E7 | T003 | crates/utopia-server/src/api/review_mcp_reads_tests.rs::review_item_answers_one_not_found_for_an_unknown_id_another_bases_id_and_an_item_no_longer_waiting |
| E8 | T003 | crates/utopia-server/src/api/review_mcp_reads_tests.rs::review_item_carries_proof_steps_for_a_derived_contradiction_and_a_null_proof_for_another_kind |
| E9 | T003 | crates/utopia-server/src/api/review_mcp_reads_tests.rs::review_item_defaults_to_duplicates_and_finds_a_kind_word_by_its_exact_word_and_an_agent_row_by_its_row_id |
| E11 | T002 | crates/utopia-server/src/api/review_token_reads_tests.rs::a_token_paging_alignment_with_equal_timestamps_visits_each_waiting_item_exactly_once |
| E12 | T002 | crates/utopia-server/src/api/review_token_reads_tests.rs::a_dead_token_gets_401_and_an_excluded_token_gets_404_on_the_api_and_403_on_the_mcp_endpoint |
| E13 | T002 | crates/utopia-server/src/api/review_token_reads_tests.rs::agent_history_without_a_token_action_reads_governor_rows_only_and_a_ledger_row_keeps_its_stored_token_name |
| E14 | T001 | crates/utopia-server/src/api/review_totals_tests.rs::a_session_list_response_differs_from_before_only_by_the_added_total_key |

## Dependency notes

- T001 has no dependency. It is alone in wave 1: the total, the tie-break and the seed all pass through `review_routes.rs` and one test file, so a second wave-1 task would collide with it.
- T002 → T001: T002's `review_ops::list` returns what `review_routes::queue_page` returns, its tests seed the base with `seed_every_section`, and its criteria compare the `total` of the list response; all three are named in T001's Interface contract. The two tasks share `review_routes.rs`, `review_ops.rs` and `api/mod.rs`, so T001's Verify also runs at T002's landing.
- T003 → T002: T003's `review_tools` calls `review_ops::list` with its eleven-section meaning, `review_ops::history` and `review_ops::violation_proof`, and its parity tests call the token-open REST routes; all are named in T002's Interface contract. The two tasks share `review_ops.rs`, `review_mcp_tests.rs` and `api/mod.rs`, so T002's Verify also runs at T003's landing.
- T003 → T001: T003's tests seed the base with `seed_every_section`, and `review_ops::item` may find a listed item through `queue_page`; both are named in T001's Interface contract. The two tasks share `review_ops.rs`, `api/mod.rs` and the store modules of the list queues, so T001's Verify also runs at T003's landing.
- T004 → T002 and T004 → T003: T004 documents the token-open REST routes named in T002's Interface contract and the two MCP tools' arguments and results named in T003's Interface contract, as they landed. It shares no file with any other task; a reviewer sees both read slices in T004's recorded base.
- T002 and T003 cannot run in the same round: both edit `review_ops.rs`, `review_mcp_tests.rs` and `api/mod.rs`. No two tasks of this plan can run concurrently except that nothing forbids it for T004, which waits on data, not on files.
- Docs-audit queue rows 35, 36 and 37 (all on ADR 0076) are absorbed by T004 as ordinary work. Every other queued ruling is declined for this milestone (driver D086).
