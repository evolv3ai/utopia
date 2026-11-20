# Archive — 003-gated-merge-proposals-revert-batch

Milestone: gated-merge-proposals-revert-batch
Shipped: 2026-11-20
Final verdict: all criteria met; project verify passed
Waves: 4  Tasks: 7 done / 7 total  Review cycles used: 1/1/1/1
Carried forward: 8 DOCS-AUDIT ruling(s)

## Success criteria at ship

| Criterion | Verdict | Evidence |
|-----------|---------|----------|
| Over the token API and over MCP, a write-scope token owned by an editor merges a duplicate pair the gate passes: the entities merge, the response names the merge, and a ledger row with outcome `applied` exists. | met | crates/utopia-server/src/api/review_ops.rs:489-512`, `:687-707`; `crates/utopia-server/src/api/review_routes.rs:355-374`; `crates/utopia-store/src/token_agent_actions.rs:526-690`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit 8f8d81b7b6087c149e720ac186e29eacdf32defe) |
| The same call on a pair the gate holds (a functional contradiction, a live derivation, an entity named in an answer, an identity source, a pair a person kept, or a pair with a reverted merge) changes nothing in the graph and lands one proposal with the gate's reason; a repeat answers `already_proposed`. | met | crates/utopia-store/src/token_agent_actions.rs:583-590`, `:761-768`; `crates/utopia-server/src/api/review_token_merge_tests.rs`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit 8f8d81b7b6087c149e720ac186e29eacdf32defe) |
| In the Review tab, a signed-in editor sees that proposal in the Agent History section and on the pair's card in Duplicates, approves it, and the merge is applied with the person as actor; a declined proposal changes nothing; the editor answers several proposals in one action. | met | web/src/pages/Review.tsx`; `web/src/pages/TokenAgentRow.tsx`; `crates/utopia-server/src/api/review_routes.rs:921-993`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit 8f8d81b7b6087c149e720ac186e29eacdf32defe) |
| A write token that calls the proposal answer is refused, over the token API and over MCP. | met | crates/utopia-server/src/api/review_routes.rs:923`, `:961`; `crates/utopia-server/src/api/review_tools.rs`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit 8f8d81b7b6087c149e720ac186e29eacdf32defe) |
| Over the token API and over MCP, the agent reverts its own merge and the graph returns to its state before the merge (facts, entities and evidence compared before and after); a second revert answers `already_decided`. | met | crates/utopia-server/src/api/review_tools.rs:271-276`, `:599-634`; `crates/utopia-server/src/api/review_ops.rs:720-745`; `web/src/docs/mcp.md:369-372`; `docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md:260-272`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit 8f8d81b7b6087c149e720ac186e29eacdf32defe) |
| A token revert of a merge made by a person or by the governor lands as a proposal, changes nothing, and does not count toward the governance fuse. | met | crates/utopia-store/src/token_agent_actions.rs:891-1060`; `crates/utopia-server/src/api/review_proposal_answer_tests.rs`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit 8f8d81b7b6087c149e720ac186e29eacdf32defe) |
| In the Review tab, a signed-in editor reverts an agent merge from the Merges section and the graph returns to its state before the merge. | met | web/src/pages/Review.tsx`; `web/src/pages/AgentProposal.tsx:30-44`; `crates/utopia-server/src/api/review_proposal_answer_tests.rs`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit 8f8d81b7b6087c149e720ac186e29eacdf32defe) |
| A token batch of 51 ids is refused; a batch of mixed pairs returns per-item outcomes with gated members as proposals and a batch id; the unit revert of that batch reverts every merge it made. | met | crates/utopia-server/src/api/review_ops.rs:536-612`, `:664-682`; `crates/utopia-server/src/api/review_routes.rs:1015-1034`, `:1163-1207`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit 8f8d81b7b6087c149e720ac186e29eacdf32defe) |
| A token that exceeds the finalized-write limit receives `agent_rate_limited`; after two reverts by a person of one token's merges within seven days that token's next merge on the base lands as a proposal. | met | crates/utopia-server/src/api/review_ops.rs:290-344`; `crates/utopia-store/src/token_agent_actions.rs:1170-1225`; `crates/utopia-server/src/api/review_token_guard_tests.rs`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit 8f8d81b7b6087c149e720ac186e29eacdf32defe) |
| Every merge, revert, batch member and proposal appears in Agent History and in Activity as "owner via token name", batch members carrying the batch id. | met | web/src/pages/TokenAgentRow.tsx`; `web/src/pages/KbSettings.tsx:655-690`; `crates/utopia-server/src/api/review_token_batch_tests.rs`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit 8f8d81b7b6087c149e720ac186e29eacdf32defe) |
| A signed-in person's merge, 500-id batch and revert behave as before, and a token merge does not lower the governor's bar or start a govern round. | met | crates/utopia-server/src/api/review_session_unchanged_tests.rs`; `crates/utopia-server/src/api/review_token_guard_tests.rs`; `crates/utopia-store/src/resolution.rs:2654`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit 8f8d81b7b6087c149e720ac186e29eacdf32defe) |

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
- review/wave-3.cycle1.md
- review/wave-4.cycle1.md
- tasks/T001-ledger-gate-and-atomic-token-merge-in-the-store.md
- tasks/T002-token-merge-over-rest-and-mcp-with-held-calls-as-proposals.md
- tasks/T003-token-revert-of-a-merge-over-rest-and-mcp.md
- tasks/T004-proposal-answer-and-review-tab-controls.md
- tasks/T005-token-batch-unit-revert-and-attribution.md
- tasks/T006-rate-limit-breaker-and-the-unchanged-session-path.md
- tasks/T007-document-the-token-write-surface-and-amend-adr-0076.md

## Notes

- none
