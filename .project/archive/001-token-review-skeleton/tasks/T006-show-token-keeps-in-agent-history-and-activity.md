---
id: T006
title: Show a token keep as "owner via token name" in Agent History and in Activity
wave: 2
deps: [T003, T004]
status: done
agent: build_T006
base: 2557a1bba93a94acf119957037c1784cb484fa1a
worktree: null
task_branch: null
files:
  - crates/utopia-core/src/models.rs
  - crates/utopia-store/src/audit.rs
  - crates/utopia-store/src/token_agent_actions.rs
  - crates/utopia-store/src/review.rs
  - crates/utopia-server/src/api/review_routes.rs
  - crates/utopia-server/src/api/kbs.rs
  - crates/utopia-server/src/api/review_attribution_tests.rs
  - web/src/api.ts
  - web/src/pages/Review.tsx
  - web/src/pages/TokenAgentRow.tsx
  - web/src/pages/TokenAgentRow.test.ts
  - web/src/pages/KbSettings.tsx
  - web/src/pages/KbActivity.test.ts
  - web/src/i18n/en.ts
  - web/src/i18n/zh.ts
  - docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md
  - docs/decisions/README.md
---

# T006 — Show a token keep as "owner via token name" in Agent History and in Activity

## Context

An agent's keep is recorded as owner plus token: one row in the ledger `token_agent_actions` (owner, token id, a snapshot of the token's name) and one `review.keep` audit row whose actor is the owner and whose detail carries `token_id` and `token_name` (T003). Nobody can see that yet. The settled display: the Review tab's Agent History section (`GET /kbs/{id}/review?queue=agent`, the section titled "Agent") returns the governor's `agent_decisions` rows unchanged plus the ledger rows, told apart by a `source` field, and shows a ledger row as "owner via token name"; the Activity view (`GET /kbs/{id}/audit`, Admin-only session route, `KbActivity` in `web/src/pages/KbSettings.tsx`) gains an optional token id, token name and declared client on its events and renders token rows as "owner via token name" with a readable action label. The declared client is the HTTP `User-Agent` already stored on every audit row; it is self-reported. "owner via token name" is the wording the Awaiting-your-nod card already uses for `remember` (`S.review.pendingSaidVia`).

## Approach

- Agent History, server side. Today the `"agent"` arm of `list` in `crates/utopia-server/src/api/review_routes.rs` returns `utopia_store::governance::list`. It must return one page over both tables, newest first by action time (`created_at DESC, id DESC`): governor rows serialized exactly as today (same keys, same values, no added key), ledger rows as objects carrying `"source": "token"`. Put the merged read in `crates/utopia-store/src/token_agent_actions.rs` (for example one query that pages the union of both tables' ids and times, then loads each side); do not edit `crates/utopia-store/src/governance.rs`. `limit` and `offset` apply to the merged sequence.
- The pager total of the section is `counts.agent_rows` (`utopia_store::review::counts`, `crates/utopia-store/src/review.rs`; the web reads `c?.agent_rows`). It must count governor rows plus ledger rows, or the last ledger rows cannot be reached. `counts.agent` (the badge: governor proposals waiting for an answer) does not change.
- Ledger row shape in the `queue=agent` response: `{"source":"token","id","operation","target_kind","target_id","owner_id","owner_name","token_id","token_name","outcome","left","right","rationale","created_at"}`, with `left`, `right` and `rationale` taken from the ledger payload (`left`, `right`, `why`), and `token_name` the stored snapshot — never a fresh read of `personal_tokens`.
- Activity, server side. `AuditEventView` (`crates/utopia-core/src/models.rs`) gains optional `token_id`, `token_name` and `declared_client`, filled by `utopia_store::audit::list_for_kb` from `detail->>'token_id'`, `detail->>'token_name'` and the `user_agent` column; `declared_client` is returned only on rows that carry a token id. Serialize the three only when present, so every other event, and the Decisions history that shares the struct (`audit::review_history`), keeps its exact shape. The route stays `AuthUser` and Admin-only (`kbs::audit_log`).
- N8 (judgment prohibition). Must not: present the declared client (the caller's User-Agent) as a verified identity, or read it for authorization. Detail: the reviewer checks that Activity labels the client as declared wherever it is shown and that no authorization or policy code reads it. So the field is named `declared_client`, the interface words it as declared (for example "client (declared): curl/8.5"), it is shown only on token rows, and nothing in `auth.rs`, `review_ops.rs`, the store's `access` module or the policy reads `user_agent` or `declared_client`.
- Web. Token rows only: add a small exported component and its pure label helpers in `web/src/pages/TokenAgentRow.tsx`, used by the Agent section of `web/src/pages/Review.tsx` for rows with `source === "token"`; governor rows keep `AgentRow` untouched. A token row shows "owner via token name", the action in words, the pair, the time and the rationale when there is one; it has no Merge, Keep or Revert button (the answer route is for governor decisions and stays session-only for them). In `KbActivity`, a row with `token_name` reads "owner via token name" followed by a readable action label from a map used for token rows only; do not add `review.keep` to the shared `S.kbset.auditActions` map, because that would relabel people's and the governor's rows, and do not change the labels of the governor, the adjudicator, the engine or a removed user. When the owner's name is gone, use the existing removed-user wording before "via".
- Strings go in both `web/src/i18n/en.ts` and `web/src/i18n/zh.ts`. Types go in `web/src/api.ts` (`AuditEvent` gains the three optional fields; the agent queue's row type becomes a union told apart by `source`).
- Web tests are vitest files in a node environment that render to static markup: `web/src/pages/KbActivity.test.ts` shows the pattern (a `QueryClient` seeded with query data, `renderToStaticMarkup`). The build runs a style guard (`web/scripts/style-guard.mjs`) and `tsc --noEmit`.
- Server tests go in `review_attribution_tests.rs`, wired with `#[cfg(test)] #[path = "review_attribution_tests.rs"] mod ...;` in `review_routes.rs`; do not edit `crates/utopia-server/src/api/mod.rs`, `mcp.rs` or `review_ops.rs` (T005 works on them in the same round). Make token keeps through the REST route of T004; seed governor rows with `utopia_store::governance::record`. To send a request without a User-Agent, build it without that header; to send one, set the header — the request layer in `crates/utopia-server/src/client_ctx.rs` stores it.
- This task changes nothing else a signed-in person sees: Duplicates, Overview, Decisions, Merges and the governor's rows render as before. Tokens still cannot read `queue=agent` in this milestone.
- ADR 0076: this is the last task of the milestone that changes the record's subject, so update its one-line status line to say what is built (the keep slice, with migration 0128) and what is `open:`, and the first word in the README index if it changed. Do not change the authority table.
- Conventions (bind this task): explanatory code comments in Chinese; tests and migrations named as sentences; user-facing error strings in English with a stable `code`; an ADR precedes any public-API or data-model change (ADR 0076 from T002 covers these response fields; if a shape must differ from it, stop with `NEEDS-ORCHESTRATOR`); new migration numbers start at 0128 and ADR numbers at 0076, re-checked against upstream `deeplethe/utopia` `dev` and its open pull requests before landing — this task adds no numbered file; migration SQL idempotent; one behaviour-sentence commit subject with a DCO `Signed-off-by`; nothing is sent to `deeplethe/utopia` or a deployed instance.

## Interface contract

- `utopia_store::token_agent_actions::TokenAgentAction` (file `crates/utopia-store/src/token_agent_actions.rs`): `pub struct TokenAgentAction { pub id: Uuid, pub kb_id: Uuid, pub operation: String, pub target_kind: String, pub target_id: Uuid, pub owner_id: Uuid, pub owner_name: Option<String>, pub token_id: Uuid, pub token_name: String, pub outcome: String, pub payload: serde_json::Value, pub created_at: DateTime<Utc> }`, deriving `Debug, Clone, Serialize, sqlx::FromRow`; `owner_name` is `users.display_name` joined at read time, `token_name` is the stored snapshot, `payload` is `{"left","right","score","why"}` for a keep.
- `utopia_store::token_agent_actions::list` and `count`: `pub async fn list(pool: &PgPool, kb_id: Uuid, limit: i64, offset: i64) -> AppResult<Vec<TokenAgentAction>>`, newest first (`created_at DESC, id DESC`); `pub async fn count(pool: &PgPool, kb_id: Uuid) -> AppResult<i64>`.
- Migration `migrations/0128_a_token_agent_action_is_kept_in_its_own_ledger.sql`: table `token_agent_actions` with columns `id UUID PRIMARY KEY`, `kb_id UUID NOT NULL`, `operation TEXT NOT NULL`, `target_kind TEXT NOT NULL`, `target_id UUID NOT NULL`, `owner_id UUID NOT NULL`, `token_id UUID NOT NULL`, `token_name TEXT NOT NULL`, `outcome TEXT NOT NULL` (one of `applied`, `held`, `proposed`), `payload JSONB NOT NULL`, `created_at TIMESTAMPTZ NOT NULL DEFAULT now()`; and column `resolution_reviews.decided_token UUID`, NULL for every decision not made through a token.
- Token-open REST routes, all under `/api/v1` with `Authorization: Bearer utp_pat_…`: `GET /kbs/{kb_id}/review?queue=duplicates` (optional `types`, `limit`, `offset`), `GET /kbs/{kb_id}/review/summary`, and `POST /kbs/{kb_id}/review/{review_id}` with body `{"action":"keep","rationale":<string or null>}` answering `{"ok": true, "outcome": "applied", "action_id": "<uuid>"}`. Refusals: 404 for a base outside the token's `kb_ids` on the reads; 422 `unknown_queue`; 403 `token_read_only`, `token_kb_out_of_scope`, `token_owner_below_editor`, `token_operation_not_open`; 409 `already_decided`.
- Session regression test: file `crates/utopia-server/src/api/review_session_unchanged_tests.rs`, test function `a_session_editor_gets_todays_duplicates_overview_keep_merge_and_batch_responses`.

## Intent coverage

- SC7
- SC8

## Acceptance criteria

1. SC7: test `agent_history_shows_a_token_keep_beside_unchanged_governor_rows` — after a token keep, a signed-in editor's `GET /review?queue=agent` holds one row with `source: "token"`, the owner's display name, the token's name, `operation` `review.keep`, `outcome` `applied` and the pair's two names; every governor row in the same response is equal, key for key, to what `utopia_store::governance::list` serializes for it; `counts.agent_rows` equals the number of governor rows plus ledger rows and `counts.agent` is unchanged. `web/src/pages/TokenAgentRow.test.ts` renders such a row and finds "<owner> via <token name>", the action in words and both names, and no Merge, Keep or Revert button.
2. E16 (held-out; ruling: Agent History shows the name the token had when the keep was made, the stored `token_name` snapshot, also after the token is renamed or revoked): test `agent_history_keeps_the_token_name_it_had_after_a_rename_and_a_revoke` renames the token's row and then revokes it, and the row in `queue=agent` still carries the name of the time of the keep.
3. E17 (held-out; ruling: ledger rows are merged into the section's existing sort order by their action time; governor rows keep the relative order they have today): test `agent_history_merges_ledger_rows_by_action_time_and_keeps_governor_order` seeds governor rows and token keeps at interleaved times and shows the response ordered by time, newest first, with the governor rows in the same relative order as `governance::list` returns them, also across a page boundary (`limit` and `offset`).
4. SC8: test `activity_returns_token_id_token_name_and_declared_client_for_a_token_keep` — a signed-in base Admin's `GET /audit` returns the keep's event with `actor_name` the owner, `token_id`, `token_name` and, for a call that sent a User-Agent, `declared_client` equal to it; an event of a signed-in person carries none of the three keys; an editor who is not Admin is still refused. `web/src/pages/KbActivity.test.ts` renders a token event and finds "<owner> via <token name>", a readable action label instead of `review.keep`, and the client marked as declared.
5. E18 (held-out; ruling: a token row whose call sent no User-Agent still reads "owner via token name" with its action label and shows no client label): a test in `web/src/pages/KbActivity.test.ts` renders a token event without `declared_client` and finds "<owner> via <token name>" and the action label and no client label; server test `activity_omits_the_declared_client_when_no_user_agent_was_sent` shows the event without the `declared_client` key.
6. In `KbActivity`, events without `token_name` render exactly as before: the existing tests of `web/src/pages/KbActivity.test.ts` pass unmodified, and `S.kbset.auditActions` has no new key.
7. The session regression test `a_session_editor_gets_todays_duplicates_overview_keep_merge_and_batch_responses` passes, and `web/src/pages/reviewDuplicatesPagination.test.ts` passes unmodified.
8. `pnpm build` in `web/` passes (style guard, `tsc --noEmit`, `vite build`).
9. The status line of ADR 0076 names what this milestone built and what is open; the authority table is unchanged.

## Verify

```bash
T="agent_history_shows_a_token_keep_beside_unchanged_governor_rows agent_history_keeps_the_token_name_it_had_after_a_rename_and_a_revoke agent_history_merges_ledger_rows_by_action_time_and_keeps_governor_order activity_returns_token_id_token_name_and_declared_client_for_a_token_keep activity_omits_the_declared_client_when_no_user_agent_was_sent a_session_editor_gets_todays_duplicates_overview_keep_merge_and_batch_responses" && test -s crates/utopia-server/src/api/review_attribution_tests.rs && test -s web/src/pages/KbActivity.test.ts && test -s web/src/pages/TokenAgentRow.test.ts && docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m001_t006 WITH (FORCE)" -c "CREATE DATABASE m001_t006" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m001_t006 && cargo run -p utopia-store --example migrate && { out=$(UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- $T 2>&1); rc=$?; printf '%s\n' "$out"; [ "$rc" -eq 0 ]; } && for t in $T; do printf '%s\n' "$out" | grep -Eq "$t \.\.\. ok" || { echo "MISSING $t"; exit 1; }; done && pnpm --dir web install --frozen-lockfile && web/node_modules/.bin/vitest run --root web web/src/pages/TokenAgentRow.test.ts web/src/pages/KbActivity.test.ts web/src/pages/reviewDuplicatesPagination.test.ts && pnpm --dir web build
```

Heavy: yes

## Log

- 2026-11-18 — created by planner
- 2026-11-19 — build_T006 preflight clean: every named path exists or is declared, SC7 and SC8 are in INTENT.md, each Interface contract bullet appears verbatim in the sibling that exchanges it (T003, T004, T005).
- 2026-11-19 — build_T006 implementation. Store: `token_agent_actions::agent_history` pages the union of `agent_decisions` and `token_agent_actions` by `created_at DESC, id DESC`, takes the governor rows of the page as the matching contiguous slice of `governance::list` (serialized unchanged, no added key; `governance.rs` untouched) and loads ledger rows by id as `{"source":"token",...}` from the stored payload and `token_name` snapshot; `review::counts.agent_rows` now counts both tables (`agent` unchanged); `audit::list_for_kb` fills `token_id`, `token_name` and `declared_client` (the stored User-Agent, only on rows with a token id). Core: `AuditEventView` gains the three optional fields (`sqlx(default)`, serialized only when present; `token_id` is the string stored in detail), so `review_history` keeps its shape. Server: the `agent` arm of `review_routes::list` calls the merged read; tests in `review_attribution_tests.rs` wired by `#[path]` in `review_routes.rs`. `kbs.rs` needed no change (route stays `AuthUser` + Admin). Web: `TokenAgentRow.tsx` (component plus `viaToken`, `tokenOperationLabel`, `declaredClientLabel`), used by the Agent section of `Review.tsx` for `source === "token"` and by `KbActivity` for events with `token_name`; `S.kbset.auditActions` has no new key; strings in en and zh; types in `api.ts`. ADR 0076 status line now `In progress` (keep slice built, migration 0128, open items listed) and README index word updated; authority table unchanged. The status line names the MCP tools `review_list`, `review_item`, `review_keep`, which T005 lands in this same wave.
- 2026-11-19 — build_T006 Verify, run 1 (exact command, Git Bash, `CARGO_TARGET_DIR=C:/Users/Owner/.gsd-path/cargo-target/utopia-T006`): fail at `pnpm --dir web build` — style guard `raw-control` on two string literals in `TokenAgentRow.test.ts`; the six cargo tests and the three vitest files (17 tests) had passed. Fixed in the test file (regex instead of literals).
- 2026-11-19 — build_T006 Verify, run 2 (exact command, same environment): pass, exit 0. Database `m001_t006` recreated and migrated; `cargo test -p utopia-server` 6 passed, 0 failed (the five new tests and `a_session_editor_gets_todays_duplicates_overview_keep_merge_and_batch_responses`); `web/node_modules/.bin/vitest run` 3 files, 17 tests passed; `pnpm --dir web build` passed (style guard 156 files, `tsc --noEmit`, `vite build`). No part of Verify was refused by the guard.
- 2026-11-19 — build_T006 environment note: the heavy lock `C:/Users/Owner/.gsd-path/cargo-target/heavy.lock` was created before run 1, but the pre-tool guard refused its removal (`rmdir`, run alone with the literal path: "destructive target is pipeline state"). It was not worked around, so the lock directory is still present and the orchestrator must remove it.
- 2026-11-19 — orchestrator Verify (isolate gsd-path-task/T006): fail, exit 1; output tail:
  ```
  
  failures:
  
  ---- api::review_routes::attribution_tests::activity_omits_the_declared_client_when_no_user_agent_was_sent stdout ----
  Error: {"action":"review.keep","actor_id":"01a1f4bd-1e31-7f92-9156-bdf4fc163ff5","actor_name":"Editor","created_at":"2026-11-19T16:56:38.056013Z","detail":{"left":"Ada Lovelace","right":"Ada King","score":0.5,"token_id":"01a1f4bd-2160-7051-b0eb-4cdd867789c5","token_name":"curator","why":"different people"},"id":"01a1f4bd-21ad-7ca2-af09-9aef761ea7ee","target_id":"01a1f4bd-2140-7312-8376-0328c719f4dd","target_kind":"review"}
  
  ---- api::review_routes::attribution_tests::activity_returns_token_id_token_name_and_declared_client_for_a_token_keep stdout ----
  Error: {"action":"review.keep","actor_id":"01a1f4bd-1e2c-7513-bca2-6cfb44917c9c","actor_name":"Editor","created_at":"2026-11-19T16:56:38.201934Z","detail":{"left":"Ada Lovelace","right":"Ada King","score":0.5,"token_id":"01a1f4bd-21f7-7c43-be66-8bd24417095b","token_name":"curator","why":"different people"},"id":"01a1f4bd-223e-7ed3-b76d-6308b379d262","target_id":"01a1f4bd-2190-7583-b70b-b579c313c36f","target_kind":"review"}
  
  
  failures:
      api::review_routes::attribution_tests::activity_omits_the_declared_client_when_no_user_agent_was_sent
      api::review_routes::attribution_tests::activity_returns_token_id_token_name_and_declared_client_for_a_token_keep
  
  test result: FAILED. 4 passed; 2 failed; 0 ignored; 0 measured; 833 filtered out; finished in 2.08s
  
  error: test failed, to rerun pass `-p utopia-server --bin utopia-server`
     Compiling utopia-store v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\task\T006\crates\utopia-store)
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.96s
       Running `C:/Users/Owner/.gsd-path/cargo-target/utopia\debug\examples\migrate.exe`
  ```
- 2026-11-19 — orchestrator Verify (isolate gsd-path-task/T006): pass, exit 0; output tail:
  ```
     Compiling reqwest v0.13.1
     Compiling utopia-llm v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\task\T006\crates\utopia-llm)
     Compiling utopia-extract v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\task\T006\crates\utopia-extract)
     Compiling sqlx v0.8.6
     Compiling pgvector v0.4.2
     Compiling utopia-core v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\task\T006\crates\utopia-core)
     Compiling utopia-store v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\task\T006\crates\utopia-store)
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 05s
       Running `C:/Users/Owner/.gsd-path/cargo-target/finish-T006\debug\examples\migrate.exe`
  (node:37300) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:5424) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  (node:28468) ExperimentalWarning: localStorage is not available because --localstorage-file was not provided.
  (Use `node --trace-warnings ...` to show where the warning was created)
  [33m
  (!) Some chunks are larger than 500 kB after minification. Consider:
  - Using dynamic import() to code-split the application
  - Use build.rollupOptions.output.manualChunks to improve chunking: https://rollupjs.org/configuration-options/#output-manualchunks
  - Adjust chunk size limit for this warning via build.chunkSizeWarningLimit.[39m
  ```
- 2026-11-19 — orchestrator regression Verify of T002 (isolate gsd-path-task/T006): pass, exit 0; output tail:
  ```
  free: decision record 0076 and migration 0128 (upstream dev and 22 open pull requests checked)
  collision: docs/decisions/0075-a-ticket-source-says-where-its-reading-stopped.md (upstream dev)
  collision: migrations/0126_a_superseded_chunk_is_found_by_its_age.sql (upstream dev)
  ```
- 2026-11-19 — orchestrator regression Verify of T003 (isolate gsd-path-task/T006): pass, exit 0; output tail:
  ```
  test a_token_keep_with_no_token_to_name_changes_nothing ... ok
  test a_token_keep_changes_no_precedent_no_settlement_and_queues_no_govern_round ... ok
  test a_pair_the_governor_is_deciding_is_refused_to_a_token_as_to_a_person ... ok
  test a_token_keep_writes_the_review_row_the_ledger_row_and_the_audit_row_together ... ok
  test replaying_migration_0128_changes_nothing ... ok
  
  test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.55s
  
     Compiling utopia-store v0.1.0 (C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\task\T006\crates\utopia-store)
      Finished `test` profile [unoptimized + debuginfo] target(s) in 14.51s
       Running tests\store\main.rs (C:/Users/Owner/.gsd-path/cargo-target/finish-T006\debug\deps\store-97773e81a9073f97.exe)
  
  running 3 tests
  test a_decision_records_why::a_decision_keeps_its_reason_and_the_precedent_carries_it ... ok
  test a_batch_decides_like_a_person::a_batch_decides_like_a_person ... ok
  test a_governor_reads_the_ledger::a_governor_reads_the_ledger ... ok
  
  test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 358 filtered out; finished in 1.66s
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.35s
       Running `C:/Users/Owner/.gsd-path/cargo-target/finish-T006\debug\examples\migrate.exe`
  ```
