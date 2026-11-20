---
id: T003
title: Show every open token proposal on its item's card and name every operation in words
wave: 1
deps: [T002]
status: pending
agent: null
base: null
worktree: null
task_branch: null
files:
  - crates/utopia-server/src/api/review_ops.rs
  - crates/utopia-server/src/api/review_routes.rs
  - crates/utopia-server/src/api/mod.rs
  - crates/utopia-server/src/api/review_item_proposals_tests.rs
  - web/src/api.ts
  - web/src/pages/Review.tsx
  - web/src/pages/PendingFacts.tsx
  - web/src/pages/AgentProposal.tsx
  - web/src/pages/AgentProposal.test.ts
  - web/src/pages/TokenAgentRow.tsx
  - web/src/pages/TokenAgentRow.test.ts
  - web/src/i18n/en.ts
  - web/src/i18n/zh.ts
  - web/src/pages/KbSettings.tsx
  - web/src/pages/KbActivity.test.ts
---

# T003 — Show every open token proposal on its item's card and name every operation in words

## Context

After T002 a defect proposal exists but only Agent History shows it. This task carries the open proposal on the item in every section's list for session, token and MCP readers, shows it on each card of the Review tab, and gives every operation, reason, state and action of the milestone an English and a Chinese label, so later family tasks add no web code. In M003 `agent_proposal` is attached to Duplicates items and Merges rows only (pattern: `open_proposals` in `crates/utopia-store/src/token_agent_actions.rs` line 1259 and its use in the session list `queue_page`, `crates/utopia-server/src/api/review_routes.rs` lines 123 to 222, and in `review_ops::item`). In the web app the pattern is `AgentProposalChip` and `MergeActor` (`web/src/pages/AgentProposal.tsx` lines 14 and 31) as used by `MergeRow` (`web/src/pages/Review.tsx` line 489), and `web/src/pages/TokenAgentRow.tsx`, which today prints the two sides of a pair with a question mark for a missing side (line 118).

## Approach

- Server: write `attach_agent_proposals` once in `crates/utopia-server/src/api/review_ops.rs` and call it from the session list, the token list and the item read, for the sections the interface contract names. It works on JSON values, so no item struct gains a field. Seed the test's proposals with the store function `propose` for every target kind; the REST routes of most families are not open yet.
- Cards in `web/src/pages/Review.tsx`: `FactRow` (line 256), `ConflictRow` (321), `UnconfirmedRow` (415), `AgentRow` (564), `ErrataRow` (763), `AlignmentRuleRow` (806), `AlignmentPhraseRow` (847), `DefectRow` (1022), `ViolationRow` (1108), `ContradictionRow` (1206), and `PendingFactRow` in `web/src/pages/PendingFacts.tsx` (line 40). `MergeRow` and the Duplicates card keep the M003 chip. A card with no proposal renders exactly as today.
- N5 (judgment, for the nod card): the chip on `PendingFactRow` names the owner and the token, and the card's existing line that says who remembered the sentence (lines 125 to 132) stays. `NodCard` (line 143) and its chunk route are not touched.
- Rail badge: the Agent badge (`web/src/pages/Review.tsx` line 1850) shows `counts.agent` plus `counts.agent_token_proposals`.
- `web/src/pages/TokenAgentRow.tsx`: a row with `target_label` prints it; a row without `left` and `right` never prints a placeholder; an approved row with `queued` reads the queued wording, not the applied wording.
- Labels: extend `tokenOperations`, `tokenProposes`, `tokenReasons`, `tokenStates` and `tokenOutcome` in `web/src/i18n/en.ts` (lines 2139 to 2194) and `web/src/i18n/zh.ts` (lines 1873 to 1910), add `tokenActions` and `tokenQueued`, and extend the closed lists `OPERATIONS`, `PROPOSABLE`, `REASONS`, `STATES` and `OUTCOMES` of `web/src/pages/TokenAgentRow.test.ts` (lines 68 to 91) and the label test (lines 243 to 287). The Activity labels are read in `web/src/pages/KbSettings.tsx` (lines 668 to 671); a person's own Review actions get no new Activity label (scope out).
- The web tests render statically and click nothing; the browser walkthrough of the surface contract is done at final review.
- If an existing test outside this task's files fails because of this change, do not edit it: record `NEEDS-ORCHESTRATOR` in the Log with the test name. The only existing-test edits this milestone allows are a lifted token refusal, an exact tool list and a label list (edge E22).
- Conventions (bind this task): Chinese explanatory comments; sentence-named tests; English user-facing strings, each refusal with a stable code; one behaviour-sentence commit subject; DCO sign-off is deferred to the upstream submission; nothing is sent to `deeplethe/utopia` or a deployed instance. Tests seed through store functions or direct inserts, never through model jobs. No migration is written and `CURRENT_SCHEMA_VERSION` stays 115; no `AgentOperation` variant, no `AppError` variant and no item struct field is added. Line numbers in this brief are those of commit `d5d2356`.

## Interface contract

- **Ledger names of this milestone (delivered by T001).** No migration; `token_agent_actions` keeps its columns. A propose-only row has `outcome` `proposed` and `gate_reason` `propose_only`; a finalized row has `outcome` `applied`. Operation and `target_kind`: `fact.confirm`, `fact.reject`, `fact.close` (`fact`); `conflict.resolve` (`conflict`); `violation.decide` (`axiom_violation`); `defect.decide` (`ontology_defect`); `errata.decide` (`errata_action`); `alignment.phrase` (`phrase_binding`); `alignment.rule` (`implication_rule`); `pending.nod` (`pending_fact`); `governor.answer` (`agent_decision`); `mapping.decide` (`concept_mapping`) — each with the row's own id as `target_id` and as `target_key`; `ontology.proposal` (`ontology_proposal`, `target_id` the id of the `ontology_proposals` row, `target_key` the section and the key joined by `:`); `alignment.kind_word` (`type_binding`, `target_id` the base id, `target_key` the normalized kind word); `consistency.check` and `inference.run` (`knowledge_base`, `target_id` and `target_key` the base id). There is one open proposal per base, operation and `target_key`. `payload` keys: `request` (the full request body), `target_label` (words naming the target), `action` (when the operation has one), `why` (the rationale, when given) and `fingerprint` (conflict and ontology proposal only). `action` codes: `confirm`, `reject`, `close` (the three fact operations); `close`, `retime_new`, `keep`, `reject_new` (conflict); `fact_retracted`, `fact_closed`, `axiom_relaxed`, `accepted` (violation); `fixed`, `accepted` (defect); `approve`, `reject` (errata and rule); `bind`, `none` (phrase and kind word); `adopt`, `reject` (ontology proposal); `confirm`, `reject` (pending fact); `merge`, `keep`, `revert` (governor answer); `confirmed`, `rejected` (mapping); the two runs have none.
- **Store functions for proposals and finalized rows (delivered by T001; no later task changes a signature).** In `crates/utopia-store/src/token_agent_actions.rs`: `pub struct Proposal<'a> { pub operation: &'a str, pub target_kind: &'a str, pub target_id: Uuid, pub target_key: &'a str, pub payload: serde_json::Value }`; `pub async fn propose(pool: &PgPool, w: &TokenWrite, p: &Proposal<'_>) -> AppResult<TokenWriteOutcome>` writes in one transaction one `proposed` ledger row and its `agent.proposal` audit row (`target_kind` `agent_action`, detail `operation`, `outcome`, `reason`, `token_id`, `token_name`, `action_id`) and touches no other table; when an open proposal exists for that base, operation and `target_key`, or a second call arrives at the same moment, it writes nothing and returns the open row with `existing` true. `pub async fn record_applied(conn: &mut sqlx::PgConnection, w: &TokenWrite, p: &Proposal<'_>) -> AppResult<TokenAgentAction>` writes one `applied` ledger row on the caller's connection, so it commits or rolls back with the caller's change. `pub fn token_audit_detail(detail: serde_json::Value, action: &TokenAgentAction) -> serde_json::Value` adds `token_id`, `token_name` and `action_id` to an audit detail. `pub async fn open_proposals_for(pool: &PgPool, kb_id: Uuid, target_kind: &str, target_ids: &[Uuid]) -> AppResult<std::collections::HashMap<Uuid, Vec<AgentProposalView>>>` returns the open proposals of those targets, oldest first. `AgentProposalView` gains `action: Option<String>`, serialized only when set. `held_back`, `land_proposal`, the three merge functions, `keep_duplicate_pair`, `answer`, `proposal`, `target_gone` and `finalized_since` keep their signatures and behaviour.
- **Agent History row and the badge count (delivered by T001).** A token ledger row in Agent History keeps today's keys and gains, each only when it has a value: `target_label` and `action` (from the payload) and `queued` (`true` on an `approved` `alignment.phrase` or `alignment.rule` row), so the row of a keep or a merge has exactly the keys it has today. `review::counts` gains `agent_token_proposals`, the number of open token proposals of the base, serialized only when it is above zero; `agent` and every other count keep their values.
- **Open proposals on list items (delivered by T003).** For session, token and MCP readers alike, every list item of the sections Awaiting your nod, Conflicts, Unconfirmed, Low confidence, Axioms, Ontology defects and Errata, every phrase and rule item of Alignment, and every governor row of Agent History carries `agent_proposals`: an array, oldest first, of `{"id","operation","reason","owner_name","token_name","created_at"}` plus `"action"` when the proposal has one, present only while at least one proposal is open, so an item without one has exactly the keys it has today. The key is attached at the JSON layer by one helper in `crates/utopia-server/src/api/review_ops.rs`, `pub(crate) async fn attach_agent_proposals(state: &AppState, kb_id: Uuid, target_kind: &str, items: &mut [serde_json::Value]) -> Result<(), AppError>`, which reads each item's `id`. Duplicates items and Merges rows keep the M003 key `agent_proposal` unchanged; kind-word items and mapping rows carry no proposal. The `counts` object of every list answer carries `agent_token_proposals` only while a token proposal is open.
- **Web shapes and labels (delivered by T003).** `web/src/pages/AgentProposal.tsx` exports `AgentProposalChips` (props `proposals`, one chip per open proposal reading owner via token name, the proposal label of its operation and the label of its action). `web/src/api.ts`: `AgentProposal` gains optional `action`; the item types of the sections above gain optional `agent_proposals`; `TokenAgentAction` gains optional `target_label`, `action` and `queued`; `ReviewCounts` gains optional `agent_token_proposals`. `web/src/i18n/en.ts` and `web/src/i18n/zh.ts`, under `review`: `tokenOperations` has a label for every ledger operation of this milestone and for the token audit actions `mapping.decided`, `consistency.checked`, `inference.materialized`, `alignment.kind_word_decided` and `ontology_proposal.rejected`; `tokenProposes` has a label for every propose-only operation; `tokenReasons` has `propose_only`; `tokenActions` has a label for every operation and action pair, keyed by the operation, a colon and the action code (for example `conflict.resolve:keep`); `tokenQueued` is the wording of a queued alignment change. The Agent rail badge shows `counts.agent` plus `counts.agent_token_proposals`.

## Intent coverage

- SC2
- SC7

## Acceptance criteria

1. E11 (held-out; ruling: the open proposal is carried on its item in every section's list for session, token and MCP readers, the key is present only while a proposal is open, and the Agent rail badge counts open token proposals beside the governor's): for each section of the interface contract the item with a seeded proposal carries `agent_proposals` for a session reader, a token reader and `review_list`, and an item without one has exactly the keys it has at `d5d2356`.
2. `counts.agent_token_proposals` is absent with no open token proposal and equals their number otherwise, and the Agent rail badge shows the sum with the governor's count.
3. Each card listed in the Approach renders one chip per open proposal reading owner via token name, the operation's proposal label and the action's label; the nod card keeps its line naming who remembered the sentence.
4. E12 (held-out; ruling: every operation, reason and state this milestone writes has a label in English and Chinese, none is shown as a raw code, and an Agent History row whose target is not a pair names its target in words and never prints a placeholder for a missing side): the test lists cover all fifteen new operation strings, `propose_only`, every operation and action pair and the five token audit actions in both languages, and a rendered non-pair row contains its `target_label` and no question mark.
5. An approved alignment row with `queued` reads the queued wording in both languages.
6. Agent History and Activity read "owner via token name" for a proposal row of every operation, using the token name stored with the action.

## Verify

```bash
test -s crates/utopia-server/src/api/review_item_proposals_tests.rs && test -s web/src/pages/TokenAgentRow.test.ts && test -s web/src/pages/AgentProposal.test.ts && docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m004_t003 WITH (FORCE)" -c "CREATE DATABASE m004_t003" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m004_t003 && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- every_section_item_carries_its_open_proposals_and_an_item_without_one_keeps_its_keys the_counts_add_open_token_proposals_only_while_one_is_open agent_history_names_each_target_in_words_for_every_operation 2>&1 | tee /dev/stderr | grep -c -E "(every_section_item_carries_its_open_proposals_and_an_item_without_one_keeps_its_keys|the_counts_add_open_token_proposals_only_while_one_is_open|agent_history_names_each_target_in_words_for_every_operation) \.\.\. ok" | grep -qx 3 && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- review_session_unchanged_tests:: review_proposal_answer_tests:: review_token_reads_tests:: review_mcp_reads_tests:: review_mcp_tests:: review_totals_tests:: review_token_api_tests:: review_token_merge_tests:: review_token_revert_tests:: review_token_batch_tests:: review_token_guard_tests:: review_routes::attribution_tests:: review_routes::conflict_tests:: review_routes::phrase_tests:: review_token_defect_tests:: review_token_fact_tests:: review_token_conflict_tests:: review_token_violation_tests:: review_token_errata_tests:: review_token_alignment_tests:: review_token_mapping_tests:: 2>&1 | tee /dev/stderr | grep -c -E "test result: ok\. [1-9]" | grep -qvx 0 && (cd web && pnpm install --frozen-lockfile && pnpm test && pnpm build)
```
<!-- Runs in a fresh sidecar worktree at the landed commit with nothing installed. It creates and migrates its own database(s) on the compose Postgres (`utopia-db-1`, 127.0.0.1:1517). The orchestrator's environment supplies an empty `CARGO_TARGET_DIR` outside the worktree, PostgreSQL 16 client tools and the `tee` shim first on PATH. The leading `test -s` checks name the test files this task owns. -->

Heavy: yes

## Log

- 2026-11-20 — created by planner
