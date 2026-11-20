# Archive — 004-remaining-review-writes

Milestone: remaining-review-writes
Shipped: 2026-11-20
Final verdict: all criteria met; project verify passed
Waves: 6  Tasks: 11 done / 11 total  Review cycles used: 1/1/1/1/1/1
Carried forward: 8 DOCS-AUDIT ruling(s)

## Success criteria at ship

| Criterion | Verdict | Evidence |
|-----------|---------|----------|
| Over the token API and over MCP, a write-scope token owned by an editor calls each propose-only operation on a seeded item: no fact, entity, review row, retrieval result or reasoning output changes, and one proposal appears for that target. | met | crates/utopia-server/src/api/review_ops.rs` (`land_proposal`); `crates/utopia-server/src/api/mod.rs` route table; scratchpad `rf5\walk1.log`, `rf5\walk2.log`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit eff87a30972e5acbb307632b09f8ba080d2c92a8) |
| In the Review tab, a signed-in editor sees each such proposal in the Agent History section and on the item's card in its own section, approves it, and the graph changes exactly as the same decision made by that person would change it; a declined proposal changes nothing. | met | web/src/pages/Review.tsx`; `web/src/pages/PendingFacts.tsx`; `web/src/pages/Ontology.tsx`; `web/src/pages/TokenAgentRow.tsx`; scratchpad `rf5\ab-02.out`..`ab-10.out`, `ab-16.out`, `ab-19.out`, `rf5\shots\`, `walk4.log`, `walk8.log`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit eff87a30972e5acbb307632b09f8ba080d2c92a8) |
| Over the token API and over MCP, a mapping decision and a consistency check run by a write token take effect at once, each with a ledger row of outcome `applied`. | met | crates/utopia-server/src/api/review_ops.rs` (`decide_mapping`, `run_check`); scratchpad `rf5\walk2.log`, `rf5\walk3.log`, `rf5\ab-05.out`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit eff87a30972e5acbb307632b09f8ba080d2c92a8) |
| A write token's kind-word decision behaves as it did before the program and now appears in Agent History and Activity as "owner via token name". | met | crates/utopia-server/src/api/review_ops.rs` (kind word); `web/src/pages/KbSettings.tsx`; scratchpad `rf5\walk2.log`, `rf5\walk3.log`, `rf5\ab-05.out`, `rf5\ab-12.out`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit eff87a30972e5acbb307632b09f8ba080d2c92a8) |
| A pending fact proposed through `remember` is still asserted or rejected only by a signed-in person; a token call that tries to answer it never changes the pending row. | met | crates/utopia-server/src/api/mod.rs` (pending routes); `web/src/pages/PendingFacts.tsx`; scratchpad `rf5\walk3.log`, `rf5\ab-03.out`, `rf5\ab-10.out`, `rf5\walk8.log`, `rf5\shots\02-nod.png`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit eff87a30972e5acbb307632b09f8ba080d2c92a8) |
| For every operation in the ruled authority table, a call with a write-scope token owned by an editor yields the outcome the table states, and the same call with a read-scope token or a below-editor owner is refused with a stable code, over the token API and over MCP. | met | crates/utopia-server/src/api/review_ops.rs`; `crates/utopia-store/src/execution_gate.rs`; scratchpad `rf5\walk1.log`, `rf5\walk2.log`, `rf5\walk3.log`, `rf5\walk4a.log`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit eff87a30972e5acbb307632b09f8ba080d2c92a8) |
| Every action taken in criteria 1 to 4 appears in Agent History and in Activity naming the token owner and the token. | met | web/src/pages/KbSettings.tsx:682-686`; `crates/utopia-store/src/audit.rs:195-207`; `crates/utopia-server/src/api/review_token_table_tests.rs`; scratchpad `rf5\ab-12.out`, `rf5\ab-18.out`, `rf5\shots\16-activity.png`, `rf5\shots\17-activity-zh.png`, `rf5\walk4.log`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit eff87a30972e5acbb307632b09f8ba080d2c92a8) |
| The API and MCP documentation list every opened Review route and tool with its ruling, and ADR 0076 matches the shipped behaviour. | met | web/src/docs/mcp.md:36-71`, `:321-585`; `docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md`; `crates/utopia-server/src/api/review_tools.rs`; `crates/utopia-server/src/api/mod.rs |
| All eight program success criteria of the charter hold on a build of this checkout, including an unchanged Review tab for a signed-in person. | met | .project/CHARTER.md` Program success criteria; scratchpad `rf5\walk1.log`..`walk8.log`, `rf5\ab-03.out`, `rf5\ab-12.out`; `D:\utopia\.project\build\verify-ledger.jsonl` (commit eff87a30972e5acbb307632b09f8ba080d2c92a8) |

## Contents

- build/evidence.json
- build/verify-ledger.jsonl
- intent/INTENT.md
- plan/PLAN.md
- research/DOCS-AUDIT.md
- research/RESEARCH.md
- research/SYNTHESIS.md
- research/evidence-codebase.md
- research/evidence-domain.md
- research/evidence-pitfalls.md
- research/evidence-stack.md
- review/FINAL.md
- review/PATCH-FINDINGS.md
- review/final-gap-1.md
- review/wave-1.cycle1.md
- review/wave-2.cycle1.md
- review/wave-3.cycle1.md
- review/wave-4.cycle1.md
- review/wave-5.cycle1.md
- review/wave-6.cycle1.md
- tasks/T001-store-proposals-for-every-review-operation.md
- tasks/T002-defect-proposal-end-to-end.md
- tasks/T003-proposals-on-cards-and-labels.md
- tasks/T004-fact-and-conflict-proposals.md
- tasks/T005-violation-errata-alignment-proposals.md
- tasks/T006-ontology-proposal-read-and-proposal.md
- tasks/T007-finalized-operations-and-mapping-read.md
- tasks/T008-pending-fact-and-governor-proposals.md
- tasks/T009-whole-table-proof.md
- tasks/T010-close-the-program-documentation.md
- tasks/T011-activity-names-the-proposing-token-on-a-persons-answer.md

## Notes

- none
