# Plan — utopia M001 token-review-skeleton

<!-- Written by the planner role. Executed wave-by-wave by /gsd-path-build. -->

Project verify: `docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m001_project WITH (FORCE)" -c "CREATE DATABASE m001_project" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m001_project && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test --workspace && (cd web && pnpm install --frozen-lockfile && pnpm test && pnpm build)`
<!-- Run once at ship, in a fresh sidecar worktree checked out at the
     reviewed commit: no node_modules, virtualenv, build output, or untracked
     env files exist there. It creates its own database `m001_project` on the
     compose `db` container (`utopia-db-1`, pgvector/pgvector:pg16, published
     on 127.0.0.1:1517), migrates it, runs the whole Rust workspace with
     database-backed tests required, then installs, tests and builds the web
     app. `#[ignore]`d delivery tests stay out. A task Verify must not copy
     this command. -->

## Config

- max_review_cycles: 3
- wave_budget: none
- review_panel: off
- finding_skeptics: off

## Wave 1 — risk burn-down

Goal: Prove the three things every later task stands on. (1) The verification ground: Windows-side `cargo` migrates and tests against the compose Postgres 16 + pgvector, the existing token read and token setup tests pass there with `UTOPIA_TEST_REQUIRE_DB=1`, and two builds with different migration sets keep to their own databases. If this fails, no database-backed Verify in this program is evidence and the Verification stack decision returns to the owner. (2) ADR 0076 exists with the owner's ruled authority table, numbers 0076 and 0128 are free upstream, and the "ontology adoption has no undo" reason is confirmed or corrected against the store. (3) The precedent isolation holds end to end at the store: a token keep, written atomically with its ledger and audit rows through migration 0128, changes no governor precedent, settles nothing as decided by people and queues no govern round. If this fails, no agent finalize is safe and wave 2's write slice does not start.
Review depth: full

## Wave 2 — walking skeleton

Goal: Duplicates and Overview end to end for a personal token, then one keep through the shared operation layer and the server-side policy, over the token API and over MCP, shown as "owner via token name" in the Review tab's Agent History section and in the Activity view. The wave touches authorization (token scope and owner role on Review routes and MCP tools) and concurrency (two keeps of one pair).
Review depth: full

## Surface contract

### token-authenticated HTTP API — T004

Criteria: SC2, SC6
Entry: `GET /api/v1/kbs/{kb_id}/review?queue=duplicates`, `GET /api/v1/kbs/{kb_id}/review/summary` and `POST /api/v1/kbs/{kb_id}/review/{review_id}` sent with `Authorization: Bearer utp_pat_…`
States: empty — `200` with `"items": []` and the same zero counts a signed-in editor gets; loading — none, each call is one JSON response; error — `401` for a dead token, `404` when the token's `kb_ids` exclude the base, `422` with code `unknown_queue` for a section other than Duplicates, `403` with code `token_read_only`, `token_kb_out_of_scope`, `token_owner_below_editor` or `token_operation_not_open`, `409` with code `already_decided`, `401` on the merge-revert, manual-merge and batch routes; success — `200` with `{"counts","queue","items"}`, the Overview summary object, or `{"ok": true, "outcome": "applied", "action_id": …}` for a keep.
Walkthrough:
1. Run a build of the reviewed commit against its own migrated database on the compose instance; sign in, create a base holding at least two pending duplicate pairs, and issue a `read` token as a base Viewer and a `write` token as a base Editor.
2. With the read token, `curl` the Duplicates list and the Overview summary; compare items and counts with what the Review tab shows the signed-in editor in Duplicates and Overview.
3. Issue a token whose base list excludes this base; repeat both reads and see `404`.
4. With the write token, `POST` `{"action":"merge"}` to one pair, then call the batch, merge-revert and manual-merge routes; each is refused, and the Duplicates queue, the entities and the Merges section are as before.
5. With the write token, `POST` `{"action":"keep"}` to one pair; see `outcome: applied` and the pair gone from the Duplicates list.

### MCP endpoint — T005

Criteria: SC3, SC4, SC5
Entry: `POST /api/v1/kbs/{kb_id}/mcp` (JSON-RPC 2.0) with a personal token; tools `review_list`, `review_item`, `review_keep`
States: empty — `review_list` for Duplicates returns `items: []` with zero counts; loading — none, each `tools/call` is one JSON-RPC reply; error — a tool result with `isError: true` whose `structuredContent.code` is `unknown_queue`, `not_found`, `token_read_only`, `token_owner_below_editor` or `already_decided`, and `review_keep` absent from `tools/list` for a caller who may not keep; success — `structuredContent` equal to the token API's body for the same token, or `{"ok": true, "outcome": "applied", "action_id": …}` for a keep.
Walkthrough:
1. On the running build from the HTTP API walkthrough, send `tools/list` with the read token of a Viewer and with the write token of an Editor; `review_list` and `review_item` are in both lists, `review_keep` only in the second.
2. Call `review_list` with `section: "duplicates"` and `section: "overview"`; compare with the token API's responses for the same token.
3. Call `review_item` with one listed pair's id and compare with that pair's card in the Review tab; call it with a made-up id and see the not-found tool error.
4. Call `review_keep` on one pair with the write token: the pair leaves the Duplicates queue. Call it again and see `already_decided`.
5. Call `review_keep` with the read token and with a write token of a Viewer; see `token_read_only` and `token_owner_below_editor` as tool errors.

### Review tab in the web app (Agent History section) — T006

Criteria: SC7
Entry: the web app's Review tab, section "Agent" in the left rail (`queue=agent`), opened by a signed-in editor of the base
States: empty — the existing "The agent has not looked at anything yet." panel when the base has neither governor rows nor token rows; loading — the page's existing loading state; error — the page's existing error state; success — governor rows drawn exactly as before, and each token keep as a row reading "owner via token name" with the action, the two names of the pair and the time, without answer buttons.
Walkthrough:
1. On the running build, after the keeps of the two walkthroughs above, sign in as an editor and open Review, then the Agent section.
2. Find the token keep: it reads "<owner's display name> via <token name>", names the pair, and offers no Merge, Keep or Revert button.
3. If the base has governor rows, see them unchanged beside it, in time order.
4. Revoke the token in the account's token page and reload: the row still shows the token's name.

### Activity view in the base settings of the web app — T006

Criteria: SC8
Entry: the base's settings page (`/kb/{kbId}/settings`), card "Activity", opened by a signed-in base Admin
States: empty — "Nothing recorded yet."; loading — the existing loading line, with no zero count; error — the existing error block with Retry; success — the token keep as "<owner> via <token name>" followed by a readable action label, and, when the call sent a User-Agent, the client shown and marked as declared; every other row drawn as before.
Walkthrough:
1. On the running build, sign in as a base Admin and open the base's settings, card Activity.
2. Find the token keep: it reads "<owner's display name> via <token name>" and a readable action label instead of `review.keep`.
3. The keep made with `curl` shows curl's User-Agent marked as declared; a keep sent without a User-Agent header shows no client label.
4. Rows of people, the governor and the engine look as they did before.

## Intent coverage

| Criterion | Task | Acceptance |
|-----------|------|------------|
| SC1 | T001 | AC1 |
| SC2 | T004 | AC1 |
| SC3 | T005 | AC1 |
| SC4 | T005 | AC4 |
| SC5 | T005 | AC5 |
| SC6 | T004 | AC5 |
| SC7 | T006 | AC1 |
| SC8 | T006 | AC4 |
| SC9 | T003 | AC4 |
| SC10 | T005 | AC8 |
| SC11 | T002 | AC1 |

## Held-out checks

| Edge | Task | Test |
|------|------|------|
| E1 | T001 | crates/utopia-store/tests/a_second_migration_run_changes_no_schema.rs::a_second_migration_run_changes_no_schema |
| E3 | T004 | crates/utopia-server/src/api/review_token_api_tests.rs::an_empty_duplicates_section_reads_the_same_for_a_token_and_a_session |
| E4 | T004 | crates/utopia-server/src/api/review_token_api_tests.rs::a_token_receives_duplicates_in_the_session_order |
| E6 | T005 | crates/utopia-server/src/api/review_mcp_tests.rs::review_list_serves_a_page_size_below_1_as_1_and_above_200_as_200 |
| E7 | T005 | crates/utopia-server/src/api/review_mcp_tests.rs::review_list_refuses_another_section_with_unknown_queue |
| E8 | T005 | crates/utopia-server/src/api/review_mcp_tests.rs::review_item_answers_one_not_found_for_an_unknown_id_and_another_bases_id |
| E10 | T005 | crates/utopia-server/src/api/review_mcp_tests.rs::two_keeps_of_one_pair_at_once_apply_exactly_one |
| E11 | T005 | crates/utopia-server/src/api/review_mcp_tests.rs::a_keep_whose_audit_row_fails_leaves_the_pair_in_the_queue |
| E13 | T005 | crates/utopia-server/src/api/review_mcp_tests.rs::a_read_token_of_a_viewer_is_refused_with_token_read_only |
| E15 | T004 | crates/utopia-server/src/api/review_token_api_tests.rs::a_refused_token_merge_revert_or_batch_leaves_the_graph_equal |
| E16 | T006 | crates/utopia-server/src/api/review_attribution_tests.rs::agent_history_keeps_the_token_name_it_had_after_a_rename_and_a_revoke |
| E17 | T006 | crates/utopia-server/src/api/review_attribution_tests.rs::agent_history_merges_ledger_rows_by_action_time_and_keeps_governor_order |
| E18 | T006 | web/src/pages/KbActivity.test.ts |
| E19 | T003 | crates/utopia-store/tests/a_token_keep_is_never_human_precedent.rs::a_persons_keep_still_counts_beside_a_token_keep |
| E20 | T005 | crates/utopia-store/tests/store/a_batch_decides_like_a_person.rs |

## Dependency notes

- T003 → T002 (same wave): T003's Verify runs `scripts/check-upstream-numbers.sh`, the script named in T002's Interface contract. Landed effect also required: ADR 0076 is in the tree before migration 0128 changes the data model (upstream convention, INTENT Constraints). A reviewer sees `docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md` in T003's recorded base.
- T004 → T003: T004 calls `utopia_store::token_agent_actions::keep_duplicate_pair` and `utopia_store::execution_gate::agent_policy`, both named in T003's Interface contract; the keep needs migration 0128's table and column.
- T005 → T004: T005 calls the `review_ops` functions and the token-open REST routes named in T004's Interface contract, and the two tasks share `review_ops.rs`, `api/mod.rs`, `review_session_unchanged_tests.rs` and `web/src/docs/mcp.md`.
- T006 → T004: the two tasks share `review_routes.rs`; T006's tests make a token keep through the REST route named in T004's Interface contract and re-run T004's session regression test.
- T006 → T003: T006 reads the `token_agent_actions` table and the `TokenAgentAction` rows named in T003's Interface contract, and extends `crates/utopia-store/src/token_agent_actions.rs`, so T003's Verify must also pass at T006's landing. T006 also edits the ADR's status line and its index row, files of T002, so T002's Verify must pass at T006's landing.
- T005 and T006 share no file and neither consumes the other; they run in the same round after T004.
- T001 has no edge: no task consumes its script or its test. Its failure blocks the wave through the wave review, not through a dependency.
