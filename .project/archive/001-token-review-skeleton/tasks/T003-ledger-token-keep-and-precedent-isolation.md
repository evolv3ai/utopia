---
id: T003
title: Add the token-agent ledger, the atomic token keep, the authority policy and the precedent isolation in the store
wave: 1
deps: [T002]
status: done
agent: build_T003
base: 49a8dd6bac87d2497d013a458bb93baa54b29000
worktree: null
task_branch: null
files:
  - migrations/0128_a_token_agent_action_is_kept_in_its_own_ledger.sql
  - crates/utopia-store/src/token_agent_actions.rs
  - crates/utopia-store/src/lib.rs
  - crates/utopia-store/src/governance.rs
  - crates/utopia-store/src/execution_gate.rs
  - crates/utopia-store/tests/a_token_keep_is_never_human_precedent.rs
---

# T003 — Add the token-agent ledger, the atomic token keep, the authority policy and the precedent isolation in the store

## Context

A personal token acts as its owner, so a keep made with a token would be stored exactly like a person's keep and the governor would read it as human precedent: its precedent queries select `audit_events` rows with `actor_id IS NOT NULL`, and `decided_before` and `type_pair_stats` select `resolution_reviews` rows with `decided_by IS NOT NULL` (`crates/utopia-store/src/governance.rs`). The settled decision is that token-agent decisions are never human precedent: review rows carry the deciding token in a new nullable column `resolution_reviews.decided_token`, the audit row carries `detail.token_id` and a `detail.token_name` snapshot, and the precedent readers exclude both. Every token-agent Review action also gets one row in a new ledger table, `token_agent_actions`. The policy that says what a token agent may finalize is a compiled-in table living with the `execution_gate` module (ADR 0027 decision 3); the owner ruled it (the table is in `.project/SYNTHESIS.md` `### Agent-authority policy` and in ADR 0076). The precedent isolation is medium confidence and was never exercised end to end; this task proves it against the database before any route uses it.

## Approach

- New module `crates/utopia-store/src/token_agent_actions.rs` (declared in `crates/utopia-store/src/lib.rs`) holds the ledger and the token keep. Do not change `resolution::decide_review` or `resolution::decide_reviews` in `crates/utopia-store/src/resolution.rs`: the person's path stays byte for byte as it is, and that file is not in this task's `files`.
- The token keep is one transaction: lock or conditionally update the `resolution_reviews` row (`... WHERE id = $1 AND kb_id = $2 AND status = 'pending'`), write the ledger row, write the audit row with `utopia_store::audit::record_on` on the same connection, commit. Pitfalls this must dodge: today's `decide_review` reads then updates without a status guard (two calls can both succeed); today's handlers discard `audit::record` results and take snapshots with `unwrap_or_default`. Here a failed token-name read, ledger insert or audit insert fails the whole keep and leaves the pair pending.
- The token name snapshot is read from `personal_tokens.name` by `token_id` inside the transaction; a missing row is an error, not an empty name. The pair's names and score are snapshotted before the update, as the session handler does (`left`, `right`, `score`, `why`).
- A pair the governor is deciding right now keeps today's refusal: reuse `crate::governance::locked_by_agent` and return the same `AppError::Conflict` text `decide_review` returns. It is not `already_decided`: nothing was decided.
- The keep must not call `governance::answer_open` or `governance::supersede_siblings`, and must not enqueue a `govern` job: a token decision starts no govern round and settles no governor proposal. An open governor proposal on the kept pair stays `proposed`.
- Precedent readers to change in `crates/utopia-store/src/governance.rs`: the three queries of `precedents_for`, `ledger_search`, `type_pair_stats` and `decided_before`. Audit readers exclude rows whose `detail` has the key `token_id`; review-row readers exclude rows with `decided_token IS NOT NULL`. Check the whole file for any other reader that treats `actor_id IS NOT NULL` or `decided_by IS NOT NULL` as "a person decided" and apply the same exclusion. `settled_by_people` and `gate` are pure functions and need no change; the test shows their results do not move.
- Migration: number 0128, sentence-named, idempotent SQL (`CREATE TABLE IF NOT EXISTS`, `ADD COLUMN IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`) so a later renumbering replays safely on a database that ran the old number. Follow `agent_decisions` in `migrations/0035_governance_reads_the_ledger.sql` for how `kb_id` is tied to the base. Ledger rows must survive the revocation or removal of the token and the removal of the owner's account: no cascading delete from `personal_tokens` or `users`. Add an index for listing a base's rows newest first. Create only the columns in the Interface contract; later milestones add theirs.
- The policy in `crates/utopia-store/src/execution_gate.rs` is data plus two small pure functions. It holds all 19 rows of the owner's ruling even though only `Keep` is called in this milestone; do not build any path for the other rows.
- The test file is a top-level integration test (its own binary), because one of its tests replays the migration's SQL text, which touches schema outside any one base: see the rule in the header of `crates/utopia-store/tests/store/main.rs`. Use `crates/utopia-store/tests/store/a_governor_reads_the_ledger.rs` as the template for seeding a base with pairs, people's decisions and `agent_decisions` rows, and `utopia_store::tokens::issue` for tokens. Tests build and remove their own base.
- Re-check the numbers with `bash scripts/check-upstream-numbers.sh 0076 0128` before landing. If 0128 is taken, stop with `NEEDS-ORCHESTRATOR` naming the next lowest free number; do not rename on your own (INTENT edge E21).
- Conventions (bind this task): explanatory code comments in Chinese; tests and migrations named as sentences; user-facing error strings in English with a stable `code`; an ADR precedes any public-API or data-model change (ADR 0076 from T002 is in your base; do not start if it is not); new migration numbers start at 0128 and ADR numbers at 0076, re-checked against upstream `deeplethe/utopia` `dev` and its open pull requests before landing; migration SQL idempotent; one behaviour-sentence commit subject with a DCO `Signed-off-by`; nothing is sent to `deeplethe/utopia` or a deployed instance.

## Interface contract

- `scripts/check-upstream-numbers.sh <adr-number> <migration-number>` — run with bash from the repository root. It only reads `deeplethe/utopia` (branch `dev` and its open pull requests) and sends nothing. Exit 0 when no file `docs/decisions/<adr-number>-*.md` and no file `migrations/<migration-number>_*.sql` exists on upstream `dev` or is added by an open upstream pull request; exit 1, naming the colliding file, on a collision; exit 2 when upstream cannot be read. Numbers are four digits (`0076`, `0128`).
- `utopia_store::execution_gate` (file `crates/utopia-store/src/execution_gate.rs`): `pub enum AgentOperation { ConfirmFact, RejectFact, CloseFact, Merge, Keep, RevertMerge, ResolveConflict, PendingNod, Batch, AnswerGovernor, AlignmentPhrase, AlignmentRule, AlignmentKindWord, Errata, OntologyProposal, Mapping, Violation, Defect, ConsistencyRun }`; `pub enum AgentRuling { Finalize, FinalizeBehindGate, ProposeOnly }`; `pub enum AgentOutcome { Applied, Held, Proposed }` with `pub fn as_str(&self) -> &'static str` returning `"applied"`, `"held"`, `"proposed"`; `pub fn agent_ruling(op: AgentOperation) -> AgentRuling`; `pub fn agent_policy(op: AgentOperation, hold: Option<&Hold>) -> AgentOutcome`, where `Finalize` gives `Applied`, `FinalizeBehindGate` gives `Applied` when `hold` is `None` and `Held` otherwise, and `ProposeOnly` gives `Proposed`.
- `utopia_store::token_agent_actions::TokenAgentAction` (file `crates/utopia-store/src/token_agent_actions.rs`): `pub struct TokenAgentAction { pub id: Uuid, pub kb_id: Uuid, pub operation: String, pub target_kind: String, pub target_id: Uuid, pub owner_id: Uuid, pub owner_name: Option<String>, pub token_id: Uuid, pub token_name: String, pub outcome: String, pub payload: serde_json::Value, pub created_at: DateTime<Utc> }`, deriving `Debug, Clone, Serialize, sqlx::FromRow`; `owner_name` is `users.display_name` joined at read time, `token_name` is the stored snapshot, `payload` is `{"left","right","score","why"}` for a keep.
- `utopia_store::token_agent_actions::keep_duplicate_pair`: `pub async fn keep_duplicate_pair(pool: &PgPool, kb_id: Uuid, review_id: Uuid, owner_id: Uuid, token_id: Uuid, rationale: Option<&str>) -> AppResult<TokenAgentAction>`. In one transaction it sets the `resolution_reviews` row to `status = 'kept'`, `decided_at = now()`, `decided_by = owner_id`, `decided_token = token_id`, `rationale`; inserts one `token_agent_actions` row with `operation = 'review.keep'`, `target_kind = 'review'`, `target_id = review_id`, `outcome = 'applied'`; and inserts one `audit_events` row with action `review.keep`, target kind `review`, target id `review_id`, actor `owner_id` and detail `{"left","right","score","why","token_id","token_name"}`. Errors: `AppError::NotFound` when the base has no review row with that id; `AppError::CodedConflict { code: "already_decided", .. }` when the row is not `pending`; the `AppError::Conflict` of `resolution::decide_review` when the governor holds the pair; any failure to read the token's name or to write the ledger or audit row is an error and nothing is committed.
- `utopia_store::token_agent_actions::list` and `count`: `pub async fn list(pool: &PgPool, kb_id: Uuid, limit: i64, offset: i64) -> AppResult<Vec<TokenAgentAction>>`, newest first (`created_at DESC, id DESC`); `pub async fn count(pool: &PgPool, kb_id: Uuid) -> AppResult<i64>`.
- Migration `migrations/0128_a_token_agent_action_is_kept_in_its_own_ledger.sql`: table `token_agent_actions` with columns `id UUID PRIMARY KEY`, `kb_id UUID NOT NULL`, `operation TEXT NOT NULL`, `target_kind TEXT NOT NULL`, `target_id UUID NOT NULL`, `owner_id UUID NOT NULL`, `token_id UUID NOT NULL`, `token_name TEXT NOT NULL`, `outcome TEXT NOT NULL` (one of `applied`, `held`, `proposed`), `payload JSONB NOT NULL`, `created_at TIMESTAMPTZ NOT NULL DEFAULT now()`; and column `resolution_reviews.decided_token UUID`, NULL for every decision not made through a token.

## Intent coverage

- SC9

## Acceptance criteria

1. Migration `migrations/0128_a_token_agent_action_is_kept_in_its_own_ledger.sql` creates the table and the column of the Interface contract; test `replaying_migration_0128_changes_nothing` executes the file's SQL text a second time against the migrated database and it succeeds without changing the schema. `bash scripts/check-upstream-numbers.sh 0076 0128` exits 0 at landing.
2. Test `a_token_keep_writes_the_review_row_the_ledger_row_and_the_audit_row_together`: after `keep_duplicate_pair`, the pair is `kept` with `decided_by` the owner and `decided_token` the token; exactly one `token_agent_actions` row with outcome `applied` and exactly one `review.keep` audit row whose detail has `token_id` and `token_name` exist for the pair; no fact, entity or `entity_merges` row of the base changed.
3. Test `a_token_keep_with_no_token_to_name_changes_nothing`: with a `token_id` that names no `personal_tokens` row the call is an error, the pair is still `pending`, and no ledger row and no audit row for the pair exist. Test `a_second_keep_of_a_decided_pair_answers_already_decided`: a repeat, and a keep of a pair a person already decided, return `CodedConflict` with code `already_decided` and write nothing; an id of another base returns `NotFound`.
4. Test `a_token_keep_changes_no_precedent_no_settlement_and_queues_no_govern_round` (SC9), on a base with governance on: for the kept pair and for a same-name sibling pair, the values of `precedents_for`, `ledger_search`, `decided_before`, `settled_by_people` and `gate` (for a fixed verdict and confidence) are equal before and after the token keep; `type_pair_stats` for the pair's type pair does not move; no `jobs` row of kind `govern` exists for the base; an open `agent_decisions` proposal on the kept pair and one on the sibling pair are still `proposed`.
5. E19 (held-out; ruling: on a base holding both a person's keep and a token's keep, the person's keep still counts as human precedent exactly as today; only the token's keep is excluded): test `a_persons_keep_still_counts_beside_a_token_keep` shows that after a person's keep through `resolution::decide_review` plus its `review.keep` audit row, `precedents_for`, `decided_before` and `settled_by_people` report that keep as today, while the token keep on the same base is absent from all of them.
6. Test `the_compiled_table_holds_the_owners_ruling`: `agent_ruling` returns `Finalize` for `Keep`, `AlignmentKindWord`, `Mapping` and `ConsistencyRun`; `FinalizeBehindGate` for `Merge`, `RevertMerge` and `Batch`; `ProposeOnly` for the other twelve operations; and `agent_policy` maps as the Interface contract says, including `Held` for a gated operation with a hold.
7. The existing store tests `a_governor_reads_the_ledger`, `a_decision_keeps_its_reason_and_the_precedent_carries_it` and `a_batch_decides_like_a_person` pass unmodified.
8. `crates/utopia-store/src/resolution.rs` is unchanged.

## Verify

```bash
T="replaying_migration_0128_changes_nothing a_token_keep_writes_the_review_row_the_ledger_row_and_the_audit_row_together a_token_keep_with_no_token_to_name_changes_nothing a_second_keep_of_a_decided_pair_answers_already_decided a_token_keep_changes_no_precedent_no_settlement_and_queues_no_govern_round a_persons_keep_still_counts_beside_a_token_keep the_compiled_table_holds_the_owners_ruling a_governor_reads_the_ledger a_decision_keeps_its_reason_and_the_precedent_carries_it a_batch_decides_like_a_person" && test -s migrations/0128_a_token_agent_action_is_kept_in_its_own_ledger.sql && test -s crates/utopia-store/tests/a_token_keep_is_never_human_precedent.rs && bash scripts/check-upstream-numbers.sh 0076 0128 && docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m001_t003 WITH (FORCE)" -c "CREATE DATABASE m001_t003" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m001_t003 && cargo run -p utopia-store --example migrate && { out=$( { UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-store --test a_token_keep_is_never_human_precedent && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-store --test store -- a_governor_reads_the_ledger a_decision_keeps_its_reason_and_the_precedent_carries_it a_batch_decides_like_a_person; } 2>&1); rc=$?; printf '%s\n' "$out"; [ "$rc" -eq 0 ]; } && for t in $T; do printf '%s\n' "$out" | grep -Eq "$t \.\.\. ok" || { echo "MISSING $t"; exit 1; }; done
```

Heavy: yes

## Log

- 2026-11-18 — created by planner
- 2026-11-19 — build_T003 preflight clean: every Context/Approach/Interface path exists or is declared, SC9 is INTENT success criterion 9, the Interface contract lines appear verbatim in T002/T004/T006 where exchanged, Verify paths exist.
- 2026-11-19 — build_T003 implementation: migration 0128 (idempotent: `token_agent_actions` with `kb_id` FK cascade to `knowledge_bases` as `agent_decisions`, bare `owner_id`/`token_id` with no FK, CHECK on `outcome`, index `(kb_id, created_at DESC, id DESC)`; `resolution_reviews.decided_token UUID`). `execution_gate.rs`: `AgentOperation` (19), `AgentRuling`, `AgentOutcome::as_str`, `agent_ruling` (exhaustive match, no wildcard), `agent_policy`. `token_agent_actions.rs` (declared in `lib.rs`): `TokenAgentAction`, `keep_duplicate_pair` (one transaction: `SELECT … FOR UPDATE OF rr` with name/score snapshot, token name from `personal_tokens` by id — missing row is an error, guarded `UPDATE … AND status = 'pending'`, ledger insert, `audit::record_on` on the same connection, commit; no `answer_open`, no `supersede_siblings`, no govern job), `list`, `count`. `governance.rs`: the three `precedents_for` queries and `ledger_search` exclude audit rows whose detail has the key `token_id`; `type_pair_stats` and `decided_before` exclude `decided_token IS NOT NULL`; no other reader in the file treats `actor_id`/`decided_by` on review or audit rows as a person. `resolution.rs` untouched. New top-level test binary `a_token_keep_is_never_human_precedent.rs` with the seven named tests plus `a_pair_the_governor_is_deciding_is_refused_to_a_token_as_to_a_person`; its database tests run serially (the replay takes table locks).
- 2026-11-19 — build_T003 decision inside Approach bounds: `governance::locked_by_agent` (reused, same `AppError::Conflict` text as `decide_review`) is called before the transaction opens, not inside it — it acquires its own pool connection, and calling it while holding the transaction's connection can starve the pool under concurrent keeps. It returns true only for a pending row of this base, so `NotFound`/`already_decided` precedence is unchanged; a governor lock taken after the check is caught by the status guard, as on the person's path.
- 2026-11-19 — build_T003 mutation check (temporary, reverted): with the `governance.rs` exclusions disabled, `a_token_keep_changes_no_precedent_no_settlement_and_queues_no_govern_round` and `a_persons_keep_still_counts_beside_a_token_keep` both fail (token keep appears as same-pair precedent, `decided_before` = false, gate keep@0.80 Propose→Apply, type-pair kept +1); with them restored both pass.
- 2026-11-19 — build_T003 Verify: the guard hook refused the whole command while it contained `bash scripts/check-upstream-numbers.sh 0076 0128` ("bash reads commands from a file or stdin the guard cannot inspect"); that one part was NOT run by me. Every other part ran exactly as written from the worktree root in Git Bash, on database `m001_t003`: both `test -s` pass; `DROP DATABASE IF EXISTS m001_t003 WITH (FORCE)` / `CREATE DATABASE m001_t003` ok; `cargo run -p utopia-store --example migrate` ok; `cargo test -p utopia-store --test a_token_keep_is_never_human_precedent` 8 passed, 0 failed; `cargo test -p utopia-store --test store -- a_governor_reads_the_ledger a_decision_keeps_its_reason_and_the_precedent_carries_it a_batch_decides_like_a_person` 3 passed, 0 failed; the `for t in $T` loop found `... ok` for all ten names; exit 0. After that run one lint-only change (`&mut *tx` → `&mut tx` in the `record_on` call) was made and both `cargo test` commands were rerun on the same database: 8 passed and 3 passed. `cargo clippy -p utopia-store --tests` clean. Result: ready, pending the orchestrator's isolated Verify (which runs the upstream number check).
- 2026-11-19 — orchestrator Verify (sidecar gsd-path-verify/task-t003-verify): pass, exit 0; output tail:
  ```
     Compiling utopia-reason v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\verify\task-t003-verify\crates\utopia-reason)
     Compiling strum v0.27.2
     Compiling hyper-util v0.1.20
     Compiling cron v0.12.1
     Compiling sqlx-macros-core v0.8.6
     Compiling sqlx-macros v0.8.6
     Compiling pdf-extract v0.12.0
     Compiling utopia-ingest v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\verify\task-t003-verify\crates\utopia-ingest)
     Compiling tokio-rustls v0.26.5
     Compiling rustls-platform-verifier v0.6.2
     Compiling hyper-rustls v0.27.9
     Compiling reqwest v0.13.1
     Compiling utopia-llm v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\verify\task-t003-verify\crates\utopia-llm)
     Compiling utopia-extract v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\verify\task-t003-verify\crates\utopia-extract)
     Compiling sqlx v0.8.6
     Compiling pgvector v0.4.2
     Compiling utopia-core v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\verify\task-t003-verify\crates\utopia-core)
     Compiling utopia-store v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\verify\task-t003-verify\crates\utopia-store)
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 57.67s
       Running `C:/Users/Owner/.gsd-path/cargo-target/utopia\debug\examples\migrate.exe`
  ```
