# Archive — 002-token-review-reads

Milestone: token-review-reads
Shipped: 2026-11-19
Final verdict: all criteria met; project verify passed
Waves: 2  Tasks: 4 done / 4 total  Review cycles used: 1/1
Carried forward: 8 DOCS-AUDIT ruling(s)

## Success criteria at ship

| Criterion | Verdict | Evidence |
|-----------|---------|----------|
| Over the token API, a read-scope token of a base Viewer lists each of the thirteen sections and receives the same items a signed-in editor sees in that section of the Review tab, on a base seeded with at least one item per section. | met | crates/utopia-server/src/api/review_ops.rs:43-116`; `crates/utopia-server/src/api/review_routes.rs:801-819`; `crates/utopia-server/src/api/review_token_reads_tests.rs`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit f2a4edd2faa4035aadc9f18851883500b0e7a00c) |
| Over MCP, `review_list` returns the same items as the token API for each of the thirteen sections with the same token. | met | crates/utopia-server/src/api/review_tools.rs`; `crates/utopia-server/src/api/review_mcp_reads_tests.rs`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit f2a4edd2faa4035aadc9f18851883500b0e7a00c) |
| For one listed item of each section, the token API and MCP `review_item` return the sentences, facts or entities the Review tab shows for it, including the proof chain of an Axioms item. | met | crates/utopia-server/src/api/review_ops.rs:43-55`, `:192-277`; `crates/utopia-server/src/api/graph_routes.rs:542-576`; `crates/utopia-server/src/api/mcp.rs:96-98`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit f2a4edd2faa4035aadc9f18851883500b0e7a00c) |
| A token client that pages the Alignment section by the returned total reaches the last implication-rule item. | met | crates/utopia-store/src/alignment_queue.rs`; `crates/utopia-server/src/api/review_token_reads_tests.rs`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit f2a4edd2faa4035aadc9f18851883500b0e7a00c) |
| A token whose `kb_ids` exclude the base receives 404 on every Review read over the token API and cannot list or read it over MCP. | met | crates/utopia-server/src/api/review_ops.rs:43-55`; `crates/utopia-server/src/api/mcp.rs:96-98`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit f2a4edd2faa4035aadc9f18851883500b0e7a00c) |
| Agent History read by a token shows the M001 keep as a ledger row naming the owner and the token, beside governor rows. | met | crates/utopia-store/src/token_agent_actions.rs`; `crates/utopia-server/src/api/review_token_reads_tests.rs`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit f2a4edd2faa4035aadc9f18851883500b0e7a00c) |
| A signed-in person's Review tab lists every section as before. | met | crates/utopia-server/src/api/review_totals_tests.rs`; `crates/utopia-server/src/api/review_session_unchanged_tests.rs:257`; `web/src/api.ts`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit f2a4edd2faa4035aadc9f18851883500b0e7a00c) |

## Contents

- build/evidence.json
- build/verify-ledger.jsonl
- intent/INTENT.md
- plan/PLAN.md
- research/DOCS-AUDIT.md
- research/SYNTHESIS.md
- research/evidence-codebase.md
- review/FINAL.md
- review/final-gap-1.md
- review/wave-1.cycle1.md
- review/wave-2.cycle1.md
- tasks/T001-section-totals-alignment-tie-break-and-section-seed.md
- tasks/T002-token-api-reads-of-every-section-and-both-proofs.md
- tasks/T003-mcp-review-list-and-review-item-for-every-section.md
- tasks/T004-document-the-read-surface-and-amend-adr-0076.md

## Notes

- none
