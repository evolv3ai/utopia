---
id: T011
title: Name the proposing token on a person's approval, decline and supersession rows in Activity
wave: 6
deps: [T003, T009, T010]
status: pending
agent: null
base: null
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
