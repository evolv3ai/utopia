# Archive — 001-token-review-skeleton

Milestone: token-review-skeleton
Shipped: 2026-11-19
Final verdict: all criteria met; project verify passed
Waves: 3  Tasks: 7 done / 7 total  Review cycles used: 1/1/1
Carried forward: 8 DOCS-AUDIT ruling(s)

## Success criteria at ship

| Criterion | Verdict | Evidence |
|-----------|---------|----------|
| Against the compose Postgres 16 + pgvector instance with `UTOPIA_TEST_REQUIRE_DB=1`, the existing token read and token setup tests pass, and two builds with different migration sets each migrate and test their own database on that instance. | met | D:\utopia\.git\gsd-path\dispatch\gsd-path-M001\T001\manual-2026-11-19T155109+0000\verify.json`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit cbffeeb0f67b45f77d384ee891f71b95a1516d9b); `crates/utopia-store/tests/a_second_migration_run_changes_no_schema.rs |
| Over the token API, a read-scope token of a base Viewer lists Duplicates and Overview and receives the same items and counts a signed-in editor sees in those sections of the Review tab; a token whose `kb_ids` exclude the base receives 404. | met | crates/utopia-server/src/api/review_ops.rs:42-54`; `crates/utopia-server/src/api/review_token_api_tests.rs`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit cbffeeb0f67b45f77d384ee891f71b95a1516d9b) |
| Over MCP, `review_list` returns the same Duplicates items and Overview summary as the token API for the same token, and `review_item` returns the evidence the Review tab shows on that pair's card. | met | crates/utopia-server/src/api/review_tools.rs:257-295`; `crates/utopia-server/src/api/review_ops.rs:42-54`; `crates/utopia-server/src/api/mcp.rs:96-98`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit cbffeeb0f67b45f77d384ee891f71b95a1516d9b) |
| Over the token API and over MCP, a write-scope token owned by an editor keeps one duplicate pair: the pair leaves the Duplicates queue, no fact or entity changes, and one ledger row with outcome `applied` and one audit row with `token_id` and `token_name` exist. | met | crates/utopia-server/src/api/review_ops.rs:142-148`; `crates/utopia-store/src/token_agent_actions.rs`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit cbffeeb0f67b45f77d384ee891f71b95a1516d9b) |
| The same keep is refused with `token_read_only` for a read-scope token and with `token_owner_below_editor` for a write token whose owner is below Editor, over the token API and as an MCP tool error carrying the same code; a repeat of a decided keep answers `already_decided`; the MCP write tool is not listed for either refused caller. | met | crates/utopia-server/src/api/review_ops.rs:59-76`; `crates/utopia-server/src/api/review_tools.rs`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit cbffeeb0f67b45f77d384ee891f71b95a1516d9b) |
| A token call to merge, revert or batch changes nothing in the graph and is refused. | met | crates/utopia-server/src/api/review_token_api_tests.rs`; `web/src/docs/mcp.md` (section "Review over the token API"); `D:\utopia\.project\build\verify-ledger.jsonl` (commit cbffeeb0f67b45f77d384ee891f71b95a1516d9b) |
| In the Review tab, a signed-in editor sees the token keep in the Agent History section as "owner via token name", beside unchanged governor rows. | met | web/src/pages/TokenAgentRow.tsx`; `crates/utopia-server/src/api/review_attribution_tests.rs`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit cbffeeb0f67b45f77d384ee891f71b95a1516d9b) |
| In the Activity view, a signed-in base Admin sees the token keep as "owner via token name" with a readable action label. | met | web/src/pages/KbSettings.tsx:681-684`; `web/src/pages/KbActivity.test.ts:98`; `crates/utopia-store/src/audit.rs:189-197`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit cbffeeb0f67b45f77d384ee891f71b95a1516d9b) |
| A database-backed test shows a token keep does not lower the governor's bar, does not settle the pair as decided by people, and starts no govern round. | met | crates/utopia-store/tests/a_token_keep_is_never_human_precedent.rs`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit cbffeeb0f67b45f77d384ee891f71b95a1516d9b) |
| A signed-in person's Review tab requests and responses for Duplicates, Overview, keep, merge and batch are unchanged, and `remember` still lands a pending fact that waits for a nod. | met | crates/utopia-server/src/api/review_session_unchanged_tests.rs`; `crates/utopia-store/tests/store/a_batch_decides_like_a_person.rs`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit cbffeeb0f67b45f77d384ee891f71b95a1516d9b) |
| ADR 0076 exists in the decision records with the ruled authority table, and its number and migration 0128 do not collide with upstream at landing. | met | docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md:59-79`; `.project/SYNTHESIS.md:115-135`; `D:\utopia\.project\build\verify-ledger.jsonl` (commits d9f67d16990a649142b99dbf3f7d397b8488a793, 178de2cedf25647c8336d003285b4758dd641717) |

## Contents

- build/evidence.json
- build/verify-ledger.jsonl
- intent/INTENT.md
- plan/PLAN.md
- research/DOCS-AUDIT.md
- research/RESEARCH.md
- research/SYNTHESIS.md
- research/evidence-capability-map.md
- research/evidence-codebase.md
- research/evidence-domain.md
- research/evidence-pitfalls.md
- research/evidence-similar.md
- research/evidence-stack.md
- review/FINAL.md
- review/PATCH-FINDINGS.md
- review/final-gap-1.md
- review/wave-1.cycle1.md
- review/wave-2.cycle1.md
- review/wave-3.cycle1.md
- tasks/T001-prove-the-verification-ground.md
- tasks/T002-record-adr-0076.md
- tasks/T003-ledger-token-keep-and-precedent-isolation.md
- tasks/T004-token-review-api-and-shared-operations.md
- tasks/T005-mcp-review-tools.md
- tasks/T006-show-token-keeps-in-agent-history-and-activity.md
- tasks/T007-count-migration-0128-in-the-schema-version.md

## Notes

- none
