# Plan — utopia M004 remaining-review-writes

<!-- Written by the planner role. Executed wave-by-wave by /gsd-path-build. -->

Project verify: `docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m004_project WITH (FORCE)" -c "CREATE DATABASE m004_project" -c "DROP DATABASE IF EXISTS m004_project_store WITH (FORCE)" -c "CREATE DATABASE m004_project_store" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m004_project && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test --workspace --exclude utopia-store && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m004_project_store && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-store && (cd web && pnpm install --frozen-lockfile && pnpm test && pnpm build)`
<!-- Run once at ship, in a fresh sidecar worktree checked out at the reviewed commit. It creates and
     migrates its own two databases on the compose Postgres (`utopia-db-1`, 127.0.0.1:1517); the
     `utopia-store` tests run on their own database. The orchestrator's environment supplies an empty
     `CARGO_TARGET_DIR` outside the worktree, PostgreSQL 16 client tools and the `tee` shim on PATH. -->

## Config

- max_review_cycles: 3
- wave_budget: none
- review_panel: off
- finding_skeptics: off

## Wave 1 — risk burn-down and walking skeleton

Goal: Prove that the ledger holds a proposal for any Review operation without a migration, and that one propose-only operation (the defect decision) runs end to end: a token proposes over REST and MCP, nothing changes, a person sees the proposal on the card and in Agent History and approves or declines it. If the store shapes or the answer dispatch fail here, the target naming or the approval pattern changes before eleven more families are built on them.
Review depth: full

## Wave 2 — the remaining propose-only families

Goal: Every propose-only operation that enters through the person's route is open to a write token of an editor as a proposal: fact confirm, reject and close, conflict resolution, violation, errata, alignment phrase and rule, and the ontology proposal with its read.
Review depth: full

## Wave 3 — finalized operations and the mapping read

Goal: A write token decides a mapping and runs the consistency check and inference at once behind the limit, each with an applied ledger row, reads the mappings, and its kind-word decision is recorded and attributed without narrowing the route.
Review depth: full

## Wave 4 — separate-entry proposals and the whole table

Goal: A token proposes an answer to a pending fact and to a governor decision through its own entries while the nod and the governor's route stay closed to it, and every row of the ruled table is proven over REST and MCP with its outcome, its refusals and its attribution.
Review depth: full

## Wave 5 — documentation close

Goal: ADR 0076 and the API and MCP documentation match every opened route and tool, and the four ruled documentation fixes are applied.
Review depth: full

## Surface contract

### token-authenticated HTTP API — T002

Criteria: SC1
Entry: `POST /api/v1/kbs/{id}/review/defects/{defect_id}` and each other opened Review write route, called with the header `Authorization: Bearer` and a personal token of scope `write` whose owner is an editor of the base.
States: empty — a base with no open item in a section: the call names an unknown id and answers 404 with nothing changed; loading — none, every propose-only call answers synchronously; error — 403 with `token_read_only`, `token_kb_out_of_scope` or `token_owner_below_editor`, 404, 409 `already_decided`, or the person's 422 code, each with nothing changed; success — HTTP 200 with `{"ok":true,"outcome":"proposed","action_id":…,"reason":"propose_only","explanation":…}`, plus `"code":"already_proposed"` on a repeat.
Walkthrough:
1. On a running build with a seeded base, create a `write` token for an editor and list one section with `GET /api/v1/kbs/{id}/review?queue=defects`; note one item id and the item's fields.
2. Call the item's write route with the token (for the defect: body `{"resolution":"accepted"}`); see HTTP 200 with outcome `proposed` and an `action_id`.
3. List the section again with the token: the item's own fields are unchanged and it now carries `agent_proposals` with that id; the facts and entities of the base are unchanged.
4. Repeat the call: see the same `action_id` and `"code":"already_proposed"`.
5. Repeat steps 2 and 3 for one fact (confirm), one conflict, one violation, one errata action, one alignment phrase, one ontology proposal, one pending fact (the propose route) and one governor decision (the propose route).
6. Call one of the routes with a `read` token and with a write token of a viewer: see 403 `token_read_only` and 403 `token_owner_below_editor`, and no new proposal.

### MCP endpoint — T007

Criteria: SC3, SC4
Entry: the MCP endpoint of the server, `tools/list` and `tools/call`, with a personal token of scope `write` whose owner is an editor of the base.
States: empty — `review_mapping_list` on a base with no mapping returns an empty list; loading — none, `review_check` answers when the run has finished; error — a tool result with `isError: true` and `{"code","message"}` carrying the REST code (`already_decided`, `bad_status`, `inference_off`, `agent_rate_limited`, `token_read_only`, `token_owner_below_editor`, `not_found`); success — `structuredContent` is the REST body of the same call and Agent History shows one `applied` row.
Walkthrough:
1. With the write token call `tools/list`: see `review_mapping`, `review_mapping_list`, `review_check` and `review_kind_word` among the Review tools; with a `read` token see `review_mapping_list` and none of the three write tools.
2. Call `review_mapping_list`, pick a proposed mapping, call `review_mapping` with status `confirmed`: the result is the person's body; call `review_mapping_list` again and see the new status.
3. Call `review_mapping` again with the same status: see the tool error `already_decided`.
4. Call `review_check` with run `consistency`: the result is the report a person gets.
5. Call `review_kind_word` for a kind word of the Alignment section: the result is the body the kind-word route returns today.
6. Call `review_list` for section `agent`: see one `applied` row for each of the three calls, each naming the owner and the token.
7. Call `review_mapping` with the `read` token: see the tool error `token_read_only`.

### Review tab in the web app (Agent History and the item cards of each section) — T003

Criteria: SC2, SC7
Entry: the Review tab of a base in the web app, opened in a browser by a signed-in editor on a running build; Activity is opened by a signed-in Admin in the base's settings.
States: empty — with no open token proposal no card shows a chip, the Agent badge shows the governor's count alone and every card looks as it does at `d5d2356`; loading — the section's existing loading state, unchanged; error — approving a proposal whose target was decided meanwhile shows it as superseded and changes nothing else, and an error of the person's own decision is shown as that decision's error with the proposal still open; success — the card shows one chip per open proposal reading owner via token name with the proposed action, Agent History lists the proposal, and after approval the item leaves its queue exactly as after the person's own decision while the row reads approved (an alignment approval reads queued).
Walkthrough:
1. In a browser, sign in as an editor of a seeded base on which a write token has proposed one decision in each section (use the HTTP walkthrough above), and open the Review tab.
2. See the Agent rail badge count the open token proposals beside the governor's proposals.
3. Open Unconfirmed, Low confidence, Conflicts, Axioms, Ontology defects, Errata, Alignment and Awaiting your nod in turn: the proposed item's card shows a chip naming the owner, the token and the proposed action; the nod card still shows who remembered the sentence; a card without a proposal is unchanged.
4. Open the Ontology page: the stored proposal that a token proposed on shows the same chip.
5. Open Agent History: each proposal is a row that names its target in words, with no raw code and no placeholder.
6. Approve the defect proposal: the defect leaves its queue as it does after a person's decision and the row reads approved. Decline the fact proposal: the fact is unchanged and the row reads declined.
7. Approve the alignment phrase proposal: the row reads queued, not applied.
8. Select several proposals of different sections and approve them in one bulk answer: each row shows its own outcome.
9. In Agent History find the rows of a token's mapping decision, consistency check and kind-word decision: each reads owner via token name.
10. Sign in as an Admin and open Activity: the proposal, its approval or decline, the mapping decision, the run and the kind-word decision each read owner via token name with a worded label.

## Intent coverage

| Criterion | Task | Acceptance |
|-----------|------|------------|
| SC1 | T002 | AC1 |
| SC1 | T004 | AC1 |
| SC1 | T005 | AC1 |
| SC1 | T006 | AC2 |
| SC1 | T008 | AC1 |
| SC1 | T009 | AC1 |
| SC2 | T002 | AC2 |
| SC2 | T003 | AC1 |
| SC2 | T004 | AC3 |
| SC2 | T005 | AC2 |
| SC2 | T006 | AC3 |
| SC2 | T008 | AC2 |
| SC2 | T009 | AC5 |
| SC3 | T007 | AC1 |
| SC4 | T007 | AC4 |
| SC5 | T008 | AC1 |
| SC6 | T009 | AC6 |
| SC7 | T003 | AC6 |
| SC7 | T009 | AC7 |
| SC8 | T010 | AC1 |
| SC9 | T009 | AC8 |

## Held-out checks

| Edge | Task | Test |
|------|------|------|
| E1 | T009 | crates/utopia-server/src/api/review_token_table_tests.rs::a_propose_only_call_changes_no_row_and_queues_no_job_for_every_operation |
| E2 | T004 | crates/utopia-server/src/api/review_token_fact_tests.rs::a_repeated_or_simultaneous_fact_proposal_returns_the_open_one |
| E3 | T009 | crates/utopia-server/src/api/review_token_table_tests.rs::an_unknown_foreign_or_decided_target_lands_no_proposal_for_every_operation |
| E4 | T009 | crates/utopia-server/src/api/review_token_table_tests.rs::an_invalid_body_gets_the_persons_code_and_lands_no_proposal |
| E5 | T009 | crates/utopia-server/src/api/review_token_table_tests.rs::proposals_and_refused_calls_never_count_toward_the_limit |
| E6 | T008 | crates/utopia-server/src/api/review_token_governor_tests.rs::a_token_proposed_governor_answer_changes_no_decision_pair_or_fuse |
| E7 | T002 | crates/utopia-server/src/api/review_token_defect_tests.rs::an_answered_proposal_answers_already_decided_and_only_an_editor_session_answers |
| E8 | T004 | crates/utopia-server/src/api/review_token_fact_tests.rs::approving_a_proposal_whose_fact_was_decided_meanwhile_supersedes_it |
| E9 | T005 | crates/utopia-server/src/api/review_token_alignment_tests.rs::approving_an_alignment_proposal_answers_the_job_id_and_its_row_reads_queued |
| E10 | T009 | crates/utopia-server/src/api/review_token_table_tests.rs::one_bulk_answer_takes_1_to_500_proposals_of_any_mix_of_operations |
| E11 | T003 | crates/utopia-server/src/api/review_item_proposals_tests.rs::every_section_item_carries_its_open_proposals_and_an_item_without_one_keeps_its_keys |
| E12 | T003 | web/src/pages/TokenAgentRow.test.ts |
| E13 | T007 | crates/utopia-server/src/api/review_token_mapping_tests.rs::a_mapping_decision_equal_to_the_status_answers_already_decided_and_writes_no_row |
| E14 | T007 | crates/utopia-server/src/api/review_token_run_tests.rs::a_token_consistency_check_and_inference_run_each_write_one_applied_row |
| E15 | T007 | crates/utopia-server/src/api/review_token_run_tests.rs::the_call_after_the_200th_finalized_write_answers_429_and_changes_nothing |
| E16 | T007 | crates/utopia-server/src/api/review_token_kind_word_tests.rs::a_token_kind_word_decision_keeps_its_answers_and_gains_one_applied_row |
| E17 | T008 | crates/utopia-server/src/api/review_token_pending_tests.rs::a_token_proposal_about_a_pending_fact_leaves_the_pending_row_byte_equal |
| E18 | T008 | crates/utopia-server/src/api/review_token_pending_tests.rs::approving_a_pending_fact_proposal_records_the_nod_as_that_persons |
| E19 | T009 | crates/utopia-server/src/api/review_token_table_tests.rs::every_operation_of_the_ruled_table_gives_its_outcome_or_a_stable_refusal |
| E20 | T009 | crates/utopia-server/src/api/review_token_table_tests.rs::every_action_reads_owner_via_token_name_in_agent_history_and_activity |
| E22 | T009 | crates/utopia-server/src/api/review_token_table_tests.rs::a_signed_in_persons_decisions_answer_as_before_in_every_opened_section |

## Dependency notes

- Docs-audit queue items 39, 40, 41 and 42 (all fix-doc, driver D123) are absorbed by T010. No other queued ruling is absorbed by this plan.
- Before any task is activated the orchestrator records two corrections in INTENT.md through build recovery, and the briefs are already written to them: E19 as corrected by driver D131 (over MCP a write tool is not listed for a read-scope or below-editor caller, and a call to one returns the coded tool error `token_read_only` or `token_owner_below_editor`), and SC2's "its own section" for an ontology proposal read as the Ontology page card plus Agent History (driver D132).
- Open conflict for that same E19 correction: INTENT.md E19 says a token that does not reach the base "is not found" over REST, while the shipped `review_ops::authorize_write` answers a write token whose `kb_ids` exclude the base with 403 `token_kb_out_of_scope`, the order the program synthesis settles and M003 shipped. The briefs follow the shipped check (T002 contract, T009 E19). If the owner's reading is 404, T002 and T009 are repaired before T002 is activated.
- The tasks form one chain, T001 to T010, because every server task edits `review_ops.rs`, `review_routes.rs` and `review_tools.rs` and each needs the landed effect of the one before it. No two tasks run at the same time.
- T002 needs T001's `propose`, `target_state`, `target_label` and `fingerprint`; a reviewer sees them called from `review_ops::land_proposal`.
- T003 needs T002's landed defect proposal path for its walkthrough and T001's `open_proposals_for` and the optional Agent History keys; it seeds every other kind through the store, so it does not wait for the family tasks.
- T004, T005, T006 and T008 each add arms to the answer dispatch and tools to the list that T002 generalised, and each replaces the still-refused tests T002 moved out of the shared 401 function; T002 is the only task that edits that function.
- T006 opens the ontology proposal read together with its proposal, in wave 2 and not with the wave 3 reads: an agent cannot propose on an ontology proposal it cannot find. The mapping read stays in wave 3 with the mapping decision.
- T007 needs T001's `record_applied` and `token_audit_detail`; its web labels were delivered by T003.
- T008 follows T007 only because of the shared files; it needs T002's propose path and answer dispatch and T003's item attachment for the nod card.
- T009 needs every operation landed: its table walks all nineteen rows.
- T010 needs the shipped code of T001 to T009 as the source for the documents and carries every interface contract so the documents can be checked against them.
