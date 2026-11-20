---
id: T011
title: Name the proposing token on a person's approval, decline and supersession rows in Activity
wave: 6
deps: [T003, T009, T010]
status: done
agent: build_T011
base: 65c3e88f4a0b254bc09cee0aceead4d45b29625f
worktree: null
task_branch: null
files:
  - crates/utopia-core/src/models.rs
  - crates/utopia-store/src/audit.rs
  - crates/utopia-server/src/api/review_token_table_tests.rs
  - web/src/api.ts
  - web/src/pages/KbSettings.tsx
  - web/src/pages/TokenAgentRow.tsx
  - web/src/pages/KbActivity.test.ts
  - web/src/i18n/en.ts
  - web/src/i18n/zh.ts
  - docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md
---

# T011 — Name the proposing token on a person's approval, decline and supersession rows in Activity

## Context

Final review blocked SC7 with finding P001 (`.project/review/PATCH-FINDINGS.md`, source `.project/review/FINAL.md` block `### SC7`). Evidence, verbatim: In Activity, the rows for a person's decline (`agent.proposal.declined`), supersession (`agent.proposal.superseded`) and approval (the person's decision row carrying `agent_proposal`) of a token proposal do not read "owner via token name": they show "Mira Okafor declined an agent proposal", "Mira Okafor closed an agent proposal that no longer applies" and e.g. "Mira Okafor violation.decided", although E20 requires an Activity row reading owner via token name for "a proposal, its approval, decline or supersession". The token name is stored (`detail.proposed_by_token` on decline and supersession) but never rendered; approval rows carry only `agent_proposal`.

Every answer row already names its proposal: the answer route writes `detail.agent_proposal` (the `token_agent_actions` id) on the decline and supersession rows (`crates/utopia-server/src/api/review_routes.rs` lines 1330 to 1340) and every approval arm writes it into the person's own decision row (`review_routes.rs` lines 317, 532, 1216, 1686, 1734, 2115, 2157, 2194, 2341, 2692, 2840, 2991; `crates/utopia-server/src/api/ontology_routes.rs` line 746; `crates/utopia-server/src/ontology_agent.rs` line 1004; `crates/utopia-store/src/temporal/conflicts.rs` line 209), for every propose-only family and for the M003 merge and revert proposals. The ledger row it points to holds `owner_id` and the `token_name` snapshot taken at propose time (`crates/utopia-store/src/token_agent_actions.rs`, `COLS` line 83). `GET /kbs/{id}/audit` is `kbs::audit_log` (`crates/utopia-server/src/api/kbs.rs` line 384) over `utopia_store::audit::list_for_kb` (`crates/utopia-store/src/audit.rs` line 170), whose rows are `AuditEventView` (`crates/utopia-core/src/models.rs` line 439); today it returns `token_name` only from `detail.token_name`, which a person's answer row never has (N2). `web/src/pages/KbSettings.tsx` lines 656 to 693 render "owner via token" only when that top-level `token_name` is set, and `web/src/pages/KbActivity.test.ts` lines 186 to 237 assert that an approval of an ontology proposal and the decline and supersession rows contain no " via ".

## Approach

- Fix at read time, once, for every answer row: `list_for_kb` resolves `detail.agent_proposal` to its ledger row in the same base and returns two new top-level keys, `proposed_by_token` (that row's stored `token_name`) and `proposed_by_owner` (the display name of that row's `owner_id`). Because every approval arm, decline and supersession already writes `agent_proposal`, this covers every operation family, the M003 merge and revert approvals included, without touching a write path. Do not add keys to stored audit details: the exact detail assertions of a person's decision rows (for example `review_token_defect_tests.rs` line 458, `review_token_errata_tests.rs` line 362, `review_token_violation_tests.rs` line 568, `review_token_ontology_tests.rs` lines 708 and 874) and of the answer rows stay byte-equal (E22).
- The person stays the actor (N2, N10): the answer row keeps `actor_id`/`actor_name` the person, and gains no `token_id`, `token_name`, `declared_client` or `batch_id`; `review_token_batch_tests.rs` lines 1189 to 1196 assert those keys are absent on a decline and must keep passing unchanged.
- `AuditEventView` gains the two fields as `#[sqlx(default)]` and `#[serde(skip_serializing_if = "Option::is_none")]`, like `token_name`, so an event that answers no token proposal (a person's own decision, a governor proposal, any other event) and the Decisions history (`audit::review_history`, same struct) have exactly the keys they have today. `proposed_by_owner` is absent when the owner's account is gone while `proposed_by_token` is present.
- Pitfall: `detail.agent_proposal` is free JSON text; the join must not cast it in a way that fails the whole page on a value that is not a uuid, and must match only a ledger row of the same `kb_id`. The page and total queries stay one statement each; the filters and their binding stay as they are.
- Web: `AuditEvent` in `web/src/api.ts` gains optional `proposed_by_token` and `proposed_by_owner`. In `KbSettings.tsx` the actor part of a row is unchanged (the person, with no " via " before the action); when `proposed_by_token` is set the row adds one worded phrase naming the proposer as `viaToken(proposed_by_owner, proposed_by_token)` (`web/src/pages/TokenAgentRow.tsx` line 11), e.g. "Mira Okafor declined an agent proposal · proposed by Mira Okafor via graph-curator". Add that phrase to `kbset` (or `review`) in `web/src/i18n/en.ts` and `web/src/i18n/zh.ts` in both languages.
- Out of scope (INTENT Scope out, driver D128): Activity labels for a person's own Review actions. An approval row such as `violation.decided` keeps its current label or code; this task adds the proposer phrase beside it and no entry to `auditActions`.
- ADR 0076 is public-API record: add a dated revision (2026-11-20 or later) under decision 8 saying that an Activity event answering a token proposal returns `proposed_by_token` and `proposed_by_owner` read from the proposal's ledger row, with the person as actor; add no other change to the ADR.
- Tests: change `every_action_reads_owner_via_token_name_in_agent_history_and_activity` (`crates/utopia-server/src/api/review_token_table_tests.rs` lines 1840 to 1870) so the decline, supersession and approval rows each assert `proposed_by_token` equal to the stored token name after the rename and revoke, `proposed_by_owner` the owner, and no `token_id`. Add a sentence-named test to the same file that, for every propose-only operation of the table (`every_row()`) and for a gated merge proposal, approves one proposal and declines another, and asserts each answer row from `GET /audit` carries `proposed_by_token`, while a person's own decision of the same operation and every row of `GET /review/history` carry neither key. In `KbActivity.test.ts` replace the absence assertions of lines 186 to 237 and add a test named exactly `names the token whose proposal a person declined, closed or approved` that renders a decline, a supersession and an approval (e.g. `violation.decided` with `agent_proposal`) and asserts the row text contains "Mira Okafor via graph-curator" after the action while the actor part is the person alone.
- If an existing test outside this task's files fails because of this change, do not edit it: record `NEEDS-ORCHESTRATOR` in the Log with the test name.
- Conventions (bind this task): Chinese explanatory comments; sentence-named tests; English user-facing strings with a Chinese counterpart; one behaviour-sentence commit subject; nothing is sent to `deeplethe/utopia` or a deployed instance. No migration (`CURRENT_SCHEMA_VERSION` stays 115), no new dependency, no write path changed. Tests seed through store functions or the token API, never through model jobs.

## Interface contract

- None

## Intent coverage

- SC7

## Acceptance criteria

1. SC7 and E20 (held-out; ruling: the token name shown is the snapshot stored with the action; a proposal, its approval, decline or supersession, a mapping decision, a run and a kind-word decision each have an Agent History row and an Activity row reading "owner via token name"): `GET /kbs/{id}/audit` returns, on the decline row, the supersession row and the approval row of a token proposal, `proposed_by_token` equal to the token name stored at propose time (still after the token is renamed and revoked) and `proposed_by_owner` the token's owner, for every propose-only operation of the ruled table and for a gated merge proposal.
2. N2 and N10: on those rows `actor_id` and `actor_name` stay the answering person and `token_id`, `token_name`, `declared_client` and `batch_id` stay absent; the stored audit details are unchanged.
3. An Activity event that answers no token proposal, and every row of `GET /review/history`, has exactly the keys it had before this task.
4. In Activity (`KbSettings.tsx`), a decline, a supersession and an approval row of a token proposal render the person as actor followed by the action and a phrase naming the proposer as "owner via token name" in English and Chinese; a row without `proposed_by_token` renders as before.
5. ADR 0076 carries a dated revision under decision 8 that names `proposed_by_token` and `proposed_by_owner` on Activity events answering a token proposal.

## Verify

```bash
test -s crates/utopia-server/src/api/review_token_table_tests.rs && test -s web/src/pages/KbActivity.test.ts && grep -q "proposed_by_token" docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md && docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m004_t011 WITH (FORCE)" -c "CREATE DATABASE m004_t011" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m004_t011 && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- every_action_reads_owner_via_token_name_in_agent_history_and_activity activity_names_the_proposing_token_on_every_approval_decline_and_supersession 2>&1 | tee /dev/stderr | grep -c -E "(every_action_reads_owner_via_token_name_in_agent_history_and_activity|activity_names_the_proposing_token_on_every_approval_decline_and_supersession) \.\.\. ok" | grep -qx 2 && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- review_token_table_tests:: review_routes::attribution_tests:: review_token_batch_tests:: review_proposal_answer_tests:: review_session_unchanged_tests:: review_token_defect_tests:: review_token_fact_tests:: review_token_conflict_tests:: review_token_violation_tests:: review_token_errata_tests:: review_token_alignment_tests:: review_token_ontology_tests:: review_token_pending_tests:: review_token_governor_tests:: review_token_merge_tests:: review_token_revert_tests:: 2>&1 | tee /dev/stderr | grep -c -E "test result: ok\. [1-9]" | grep -qvx 0 && (cd web && pnpm install --frozen-lockfile && pnpm exec vitest run KbActivity -t "names the token whose proposal a person declined, closed or approved" 2>&1 | tee /dev/stderr | grep -q -E "Tests +[1-9][0-9]* passed" && pnpm test && pnpm build)
```

Heavy: yes

## Log

- 2026-11-20 — created by planner (patch mode, finding P001)
- 2026-11-20 — build_T011 (coder), base 65c3e88, serial round in D:\utopia. Preflight: `kbs::audit_log` serializes `list_for_kb` rows as-is, so no edit to `kbs.rs` was needed; `AuditEventView` has no struct-literal constructor anywhere; no existing test outside `files` asserts an exact key set of an `/audit` event.
  - Built: `audit::list_for_kb` LEFT JOINs `token_agent_actions p` on `p.kb_id = e.kb_id`, `e.detail->>'token_id' IS NULL`, and `p.id` = `detail.agent_proposal` cast to uuid only when it matches a strict uuid regex (CASE, so a non-uuid value never fails the page), plus `users o` on `p.owner_id`; it returns `proposed_by_token` (`p.token_name` snapshot) and `proposed_by_owner` (owner display name). The total query, filters and binds are unchanged; no stored detail, write path or migration changed. `AuditEventView` gained both as `#[sqlx(default)]` + `skip_serializing_if`, so `review_history` and events with no proposal keep their keys. Web: `AuditEvent` gained both optional fields; `KbActivity` keeps the actor part unchanged and, on a row without `token_name` that has `proposed_by_token`, appends ` · ` + `kbset.auditProposedBy(viaToken(owner, token))` ("proposed by Mira Okafor via graph-curator" / "由 Mira Okafor 经 graph-curator 提议"). No `auditActions` entry was added. ADR 0076: one dated revision (2026-11-20) under decision 8, no other change.
  - Tests: `every_action_reads_owner_via_token_name_in_agent_history_and_activity` now asserts on the decline, supersession and approval rows: `proposed_by_token` == stored name after a rename and revoke, `proposed_by_owner` the owner, the actor the person, and no `token_id`/`token_name`/`declared_client`/`batch_id`. The new `activity_names_the_proposing_token_on_every_approval_decline_and_supersession` approves one and declines one proposal for all 13 propose-only entries (`CASES` filtered by `agent_policy`, ontology reject and adopt both included) and for a gated merge, plus one supersession. It checks every answer row in `GET /audit` the same way, checks that the person's own decisions of the same operations (and a person's merge) and token rows carry neither key, and checks that every `GET /review/history` row carries neither key. KbActivity.test.ts: the absence assertions of the two person-answer tests were replaced (actor part `<span class="text-ink">Mira Okafor</span>` alone, proposer phrase after the action), and the new test `names the token whose proposal a person declined, closed or approved` was added (decline, supersession, `violation.decided` approval; Chinese string; removed owner; a row with no proposal renders without a proposer).
  - Results (own DBs m004_t011_dev / m004_t011_dev_store, freshly migrated; CARGO_TARGET_DIR coder-m004): the two named server tests: 2 passed (each `... ok`). Whole `cargo test -p utopia-server` (UTOPIA_TEST_REQUIRE_DB=1): 976 passed, 0 failed, 2 ignored. This run includes every module in the Verify group filter, and both known flaky tests passed. `cargo test -p utopia-store` (whole crate, own DB): 535 passed, 0 failed across 23 test binaries. `cargo fmt --all --check`: clean. `cargo clippy --workspace --all-targets -- -D warnings`: exit 0. Web: `pnpm install --frozen-lockfile` ok. `vitest run KbActivity -t "names the token whose proposal a person declined, closed or approved"`: 1 passed (13 skipped). `pnpm test`: 418 passed. `pnpm build`: built (existing chunk-size warning only).
  - Deviation: the literal Verify uses database `m004_t011`; per dispatch I ran the same steps against `m004_t011_dev`. The orchestrator brief said the ADR revision belongs under `## Status history`; the task contract (Approach, AC5) says under decision 8, and I followed the contract.
  - Render tests only: the Activity proposer phrase is checked by static render (KbActivity.test.ts) and not clicked in a browser.
- 2026-11-20 — orchestrator Verify (sidecar gsd-path-verify/task-t011-verify): pass, exit 0; output tail:
  ```
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:1052) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:24668) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:37128) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:32000) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:24440) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:32584) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:39736) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  [33m
  (!) Some chunks are larger than 500 kB after minification. Consider:
  - Using dynamic import() to code-split the application
  - Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
  - Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.[39m
  ```
- 2026-11-20 — orchestrator regression Verify of T001 (sidecar gsd-path-verify/task-t011-verify): pass, exit 0; output tail:
  ```
  test api::review_token_reads_tests::agent_history_without_a_token_action_reads_governor_rows_only_and_a_ledger_row_keeps_its_stored_token_name ... ok
  test api::review_totals_tests::a_session_list_response_differs_from_before_only_by_the_added_total_key ... ok
  test api::review_token_reads_tests::a_token_paging_alignment_with_equal_timestamps_visits_each_waiting_item_exactly_once ... ok
  test api::review_token_reads_tests::a_token_asking_for_mappings_or_an_unknown_section_is_refused_with_unknown_queue ... ok
  test api::review_token_reads_tests::a_dead_token_gets_401_and_an_excluded_token_gets_404_on_the_api_and_403_on_the_mcp_endpoint ... ok
  test api::review_totals_tests::the_alignment_low_confidence_and_agent_totals_follow_the_list_while_counts_keep_their_values ... ok
  test api::review_totals_tests::alignment_items_with_one_timestamp_are_each_listed_once_across_pages ... ok
  test api::review_totals_tests::a_token_reading_duplicates_receives_the_same_total_as_a_session ... ok
  test api::review_totals_tests::an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total ... ok
  test api::review_routes::attribution_tests::agent_history_merges_ledger_rows_by_action_time_and_keeps_governor_order ... ok
  test api::review_token_reads_tests::a_token_receives_each_sections_items_in_the_session_order ... ok
  test api::review_token_reads_tests::a_read_token_of_a_viewer_lists_each_of_the_thirteen_sections_as_a_session_editor_sees_them ... ok
  test api::review_totals_tests::the_seed_puts_items_in_every_one_of_the_thirteen_sections ... ok
  test api::review_token_reads_tests::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values ... ok
  test api::review_totals_tests::the_total_of_every_list_queue_is_the_number_of_rows_its_pages_return ... ok
  test api::review_token_reads_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read ... ok
  test api::review_token_reads_tests::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items ... ok
  
  test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 952 filtered out; finished in 14.54s
  ```
- 2026-11-20 — orchestrator regression Verify of T003 (sidecar gsd-path-verify/task-t011-verify): pass, exit 0; output tail:
  ```
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:40628) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:38228) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:31692) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:31420) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:29268) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:13848) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:14444) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  [33m
  (!) Some chunks are larger than 500 kB after minification. Consider:
  - Using dynamic import() to code-split the application
  - Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
  - Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.[39m
  ```
- 2026-11-20 — orchestrator regression Verify of T006 (sidecar gsd-path-verify/task-t011-verify): pass, exit 0; output tail:
  ```
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:3216) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:38600) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:37992) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:24244) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:27740) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:34408) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:38092) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  [33m
  (!) Some chunks are larger than 500 kB after minification. Consider:
  - Using dynamic import() to code-split the application
  - Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
  - Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.[39m
  ```
- 2026-11-20 — orchestrator regression Verify of T009 (sidecar gsd-path-verify/task-t011-verify): pass, exit 0; output tail:
  ```
  test api::review_totals_tests::the_alignment_low_confidence_and_agent_totals_follow_the_list_while_counts_keep_their_values ... ok
  test api::review_totals_tests::a_token_reading_duplicates_receives_the_same_total_as_a_session ... ok
  test api::review_totals_tests::alignment_items_with_one_timestamp_are_each_listed_once_across_pages ... ok
  test api::review_token_revert_tests::review_revert_is_listed_only_to_a_write_token_of_an_editor_and_says_what_a_revert_does_not_recall ... ok
  test api::review_token_revert_tests::a_token_reverts_its_own_merge_over_the_api_and_over_mcp_and_a_second_revert_answers_already_decided ... ok
  test api::review_token_run_tests::a_token_consistency_check_and_inference_run_each_write_one_applied_row ... ok
  test api::review_token_reads_tests::a_tokens_total_counts_the_listed_rows_and_counts_keep_their_values ... ok
  test api::review_totals_tests::an_empty_queue_has_total_0_and_an_offset_past_the_end_returns_no_items_with_the_true_total ... ok
  test api::review_token_run_tests::the_call_after_the_200th_finalized_write_answers_429_and_changes_nothing ... ok
  test api::review_totals_tests::the_seed_puts_items_in_every_one_of_the_thirteen_sections ... ok
  test api::review_token_revert_tests::a_token_reverts_any_token_agents_merge_of_the_base_and_an_unknown_or_foreign_merge_id_is_not_found ... ok
  test api::review_totals_tests::the_total_of_every_list_queue_is_the_number_of_rows_its_pages_return ... ok
  test api::review_token_revert_tests::after_a_token_merge_and_its_revert_every_fact_entity_and_evidence_row_is_as_before ... ok
  test api::review_token_reads_tests::a_token_whose_kb_ids_exclude_the_base_gets_404_on_every_review_read ... ok
  test api::review_token_merge_tests::of_a_person_the_governor_and_a_token_deciding_one_pair_exactly_one_is_applied_and_the_losing_token_gets_already_decided ... ok
  test api::review_token_violation_tests::a_token_violation_decision_waits_as_a_proposal_and_a_person_applies_it ... ok
  test api::review_token_reads_tests::a_token_page_size_is_clamped_and_an_offset_at_or_past_the_total_returns_no_items ... ok
  
  test result: ok. 148 passed; 0 failed; 0 ignored; 0 measured; 830 filtered out; finished in 32.47s
  ```
- 2026-11-20 — orchestrator regression Verify of T010 (sidecar gsd-path-verify/task-t011-verify): pass, exit 0; output tail:
  ```
  DROP DATABASE
  CREATE DATABASE
  数据库迁移完成
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s
       Running `C:/Users/Owner/.gsd-path/cargo-target/finish-m004-T011\debug\examples\migrate.exe`
      Finished `test` profile [unoptimized + debuginfo] target(s) in 0.36s
       Running unittests src\main.rs (C:/Users/Owner/.gsd-path/cargo-target/finish-m004-T011\debug\deps\utopia_server-0a73fc6c26d41d5f.exe)
  
  running 1 test
  test docs_corpus::tests::every_corpus_md_file_is_indexed ... ok
  
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 977 filtered out; finished in 0.00s
  ```
