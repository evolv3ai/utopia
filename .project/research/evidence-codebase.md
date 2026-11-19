# Evidence — codebase

Repo root: D:\utopia
Scanned: 2026-11-19
Checks run: all in the disposable sidecar `C:\Users\Owner\.gsd-path\projects\bb0ebb333ad529af\e1cb4cacbe546d13\verify\inspect-codebase` at recorded HEAD `904b2a20380719553896160aa05d0af4f3a561ce` (branch `gsd-path-verify/inspect-codebase`):
- `git rev-parse HEAD` → `904b2a20380719553896160aa05d0af4f3a561ce`; `git status --short` before the scan → clean
- `git diff --stat cbffeeb HEAD -- . ":(exclude).project"` → no output (the product tree at HEAD is identical to commit `cbffeeb`)
- `git log --oneline 3d307f6..HEAD | wc -l` → 22; `git diff --stat 3d307f6 HEAD -- . ":(exclude).project"` → 35 files changed, 6165 insertions, 68 deletions
- `ls migrations | wc -l` → 114; `grep -c "\.route(" crates/utopia-server/src/api/mod.rs` → 162; `grep -rE "#\[(tokio::)?test" crates --include=*.rs | wc -l` → 1913; `find web/src -name "*.test.ts" | wc -l` → 55
- Static reads (Read/grep/sed) of the files cited below
- Not run here: `cargo fmt`, `cargo clippy`, `cargo build`, `cargo test`, `pnpm test`, `pnpm build`, any migration, any Postgres query. Cited instead: the last entry of `D:\utopia\.project\archive\001-token-review-skeleton\build\verify-ledger.jsonl` records, at commit `cbffeeb0f67b45f77d384ee891f71b95a1516d9b`, exit code 0 for: create databases `m001_project` and `m001_project_store` on container `utopia-db-1`, `cargo run -p utopia-store --example migrate`, `UTOPIA_TEST_REQUIRE_DB=1 cargo test --workspace --exclude utopia-store`, migrate the second database, `UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-store`, then `pnpm install --frozen-lockfile && pnpm test && pnpm build` in `web/`. Lint (`cargo clippy`) status at HEAD is `unverifiable` here.

## Map

- **Stack**: Rust 2021 Cargo workspace (resolver 2), 9 crates at `0.1.0`, Apache-2.0. Backend: tokio 1, axum 0.8, tower-http 0.6, sqlx 0.8 (compile-time `sqlx::migrate!`), pgvector 0.4, tantivy 0.26, reqwest 0.13, jsonwebtoken 10, argon2 0.5. Database: PostgreSQL 16 + pgvector (`docker-compose.yml`, host port 1517). 114 SQL migrations numbered 0001–0128 with gaps (0127 unused; 0128 added by M001). Web: React 19, TypeScript 5.7, Vite 6, Tailwind 4, TanStack Router/Query, vitest. Unchanged since the previous map except migration 0128, ADR 0076 and two scripts (`scripts/prove-db-isolation.sh`, `scripts/check-upstream-numbers.sh`).
- **Entry points**: `utopia-server` binary (`crates/utopia-server/src/main.rs`); `utopia` CLI (`crates/utopia-cli/src/main.rs`, `backup`/`restore`, `CURRENT_SCHEMA_VERSION = 114` at line 85); HTTP router `crates/utopia-server/src/api/mod.rs` (`pub fn router`, 162 `.route(` calls); MCP endpoint `POST /api/v1/kbs/{id}/mcp` → `api/mcp.rs::handle` (`mod.rs:240`); web SPA `web/src/main.tsx`. Review routes are registered at `api/mod.rs:534-617`: `GET /kbs/{id}/review` (`review_routes::list`), `GET .../review/history`, `GET .../review/summary`, `POST .../review/batch`, `POST .../review/agent/{decision_id}`, `GET .../review/pending`, `POST .../review/pending/{pending_id}`, `POST .../review/{review_id}`, and the per-section decision routes. Proof reads: `GET /kbs/{id}/derived/{derived_id}/proof` and `GET /kbs/{id}/violations/{violation_id}/proof` (`mod.rs:459-465`); fact evidence `GET /kbs/{id}/facts/{fact_id}/evidence` (`mod.rs:449`).
- **Architecture**: Layered crates `utopia-core` ← `utopia-store` (all Postgres access) ← `utopia-server` (axum handlers, job dispatch); `utopia-server` has no `lib.rs`, so its tests live in `src/`. For Review, M001 added a three-file token path in the server: `api/review_ops.rs` (shared operation layer: `ReviewCaller`, `authorize_read`, `authorize_write`, `list`, `item`, `summary`, `keep`, `not_open`), `api/review_tools.rs` (MCP tool definitions and dispatch for `review_list`, `review_item`, `review_keep`), and token branches at the top of three handlers in `api/review_routes.rs` (`list`, `decide`, `summary`) that take the `SessionOrToken` extractor from `auth.rs` and return early into `review_ops` when `caller.pat` is `Some`. The session branch of each handler is the pre-M001 code. `api/mcp.rs::handle` re-authenticates every POST, then routes `tools/call` names matched by `review_tools::serves` to `review_tools::call` before the `is_exposed` check used for all other tools. Token writes go through `utopia_store::execution_gate::agent_policy` (compiled-in table `agent_ruling`) and `utopia_store::token_agent_actions::keep_duplicate_pair` (one transaction: review row, ledger row, audit row).
- **Conventions**: Code comments are Chinese and explain why, citing ADR/issue numbers; user-facing error strings are English with stable `code` values (`AppError::invalid(code, msg)`, `AppError::CodedForbidden`, `AppError::CodedConflict`). Server tests are sibling `*_tests.rs` files with sentence-named test functions (`a_token_receives_duplicates_in_the_session_order`); store integration tests are one sentence-named file per behaviour under `crates/utopia-store/tests/` (17 top-level files plus 141 under `tests/store/`). Migrations are sentence-named and, for 0128, written replayable (`IF NOT EXISTS`). Upstream commit style is one behaviour sentence ending `(#NNNN)`; the 22 commits since `3d307f6` are pipeline commits (`T00N: <sentence>`, `build: ...`, `ship: ...`, `integrate: ...`).
- **Maturity**: Working product. 1913 `#[test]`/`#[tokio::test]` attributes in `crates/` (1871 at the previous map), 55 vitest files (54 before). M001's Review token surface is covered by `review_token_api_tests.rs` (9 tests), `review_mcp_tests.rs` (13), `review_attribution_tests.rs` (5), `review_session_unchanged_tests.rs` (1), `crates/utopia-store/tests/a_token_keep_is_never_human_precedent.rs` (9) and `a_second_migration_run_changes_no_schema.rs` (1). Database-backed tests still return early (pass) without `UTOPIA_DATABASE_URL` unless `UTOPIA_TEST_REQUIRE_DB` is set (`crates/utopia-store/src/test_db.rs:17-31`).
- **Recent activity**: The 22 commits since `3d307f6` are M001 only: ADR 0076 (`docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md`), migration 0128, the store ledger and precedent isolation (`token_agent_actions.rs`, `governance.rs`, `resolution.rs`, `execution_gate.rs`, `audit.rs`, `review.rs`), the server token path (`review_ops.rs`, `review_tools.rs`, `review_routes.rs`, `mcp.rs`, `auth.rs`), web attribution (`web/src/pages/TokenAgentRow.tsx`, `Review.tsx`, `KbSettings.tsx`, `api.ts`, i18n), `web/src/docs/mcp.md`, and the CLI schema-version bump.

## Finding: The Review tab has exactly thirteen selectable sections; the server list handler knows twelve queue keys, and they are not the same set

- **Claim**: `web/src/pages/Review.tsx` type `Sel` has 13 members: `overview`, `pending`, `duplicates`, `conflicts`, `unconfirmed`, `lowconf`, `violations`, `defects`, `alignment`, `errata`, `agent`, `decisions`, `merges`. The session branch of `review_routes::list` accepts 12 `queue` values: `pending`, `duplicates`, `conflicts`, `unconfirmed`, `lowconf`, `mappings`, `alignment`, `errata`, `violations`, `defects`, `merges`, `agent`; anything else is `unknown_queue`. `overview` is served by `review_routes::summary` and `decisions` by `review_routes::history`, not by the list route. `mappings` is a list queue with a count but is not a Review section: the rail item navigates to `/kb/$kbId/mappings`.
- **Source**: `web/src/pages/Review.tsx:1301-1340`, `1834-1845`; `crates/utopia-server/src/api/review_routes.rs:113-177`, `760-791`
- **Confidence**: high
- **Why it matters here**: The roadmap's "Axioms" and "Ontology" sections correspond to queue keys `violations` and `defects` (labels `S.review.violations` / `S.review.defects`); "Agent History" is `agent`; "Awaiting your nod" is `pending`; "Low confidence" is `lowconf`. Thirteen sections means eleven list-route queues plus two other routes, and the twelfth list queue (`mappings`) is outside the thirteen.

## Finding: The token list path serves only `duplicates`; every other section is refused before any store read

- **Claim**: `review_ops::list` calls `authorize_read`, then returns `unknown_queue` for any `section != "duplicates"`. It duplicates the session branch's clamp (`limit.unwrap_or(REVIEW_PAGE).clamp(1, 200)`, `offset.max(0)`, `REVIEW_PAGE = 10`) and response shape `{counts, queue, items}` rather than sharing the session `match`. `review_tools::call` sends `section == "overview"` to `review_ops::summary` and everything else to `review_ops::list`, passing `types = None`; `review_item` only calls `review_ops::item`, which reads `utopia_store::resolution::pending_review` (a pending duplicate pair or `NotFound`). The MCP tool descriptions and `web/src/docs/mcp.md:208` state that only two sections are served.
- **Source**: `crates/utopia-server/src/api/review_ops.rs:78-126`; `crates/utopia-server/src/api/review_tools.rs:38-121`, `252-297`; `crates/utopia-server/src/api/review_routes.rs:29`, `84-112`
- **Confidence**: high
- **Why it matters here**: The eleven-queue `match` exists only inside the session branch of the handler. Tests `a_token_asking_for_another_section_is_refused_with_unknown_queue` (`review_token_api_tests.rs:295`) and `review_list_refuses_another_section_with_unknown_queue` (`review_mcp_tests.rs:419`) assert today's refusal and the tool descriptions and docs state it.

## Finding: Which Review reads accept a token today

- **Claim**: `SessionOrToken` is the extractor on `review_routes::list`, `decide`, `summary`, `decide_alignment_kind_word`, and on `graph_routes::fact_evidence`, `entity_detail`, `entity_history` and four other graph reads. `AuthUser` (every Bearer is decoded as a JWT, so a personal token gets 401) is the extractor on `review_routes::history` (Decisions), `pending_for_chunk`, `agent_answer`, `batch`, and on `graph_routes::derived_proof` and `graph_routes::blocked_proof`.
- **Source**: `crates/utopia-server/src/auth.rs:156-217`; `crates/utopia-server/src/api/review_routes.rs:86`, `315`, `396`, `531`, `762`, `779`, `1109`, `1422`; `crates/utopia-server/src/api/graph_routes.rs:74`, `108`, `145`, `179`, `524-556`, `634`
- **Confidence**: high
- **Why it matters here**: The Decisions history and both proof reads return 401 to a token at this revision; fact evidence and entity reads already work for a token through `documents_routes::require_reader_kb` (Viewer role, 404 when `kb_ids` exclude the base).

## Finding: A token whose `kb_ids` exclude the base gets 404 over REST and HTTP 403 over MCP

- **Claim**: `review_ops::authorize_read` maps both a missing role and `!token.covers(kb_id)` to `AppError::NotFound`. `mcp::authorize` returns `AppError::Forbidden` when `!auth.covers(kb_id)`, before any JSON-RPC method runs; a dead token is `Unauthorized` from `tokens::authenticate` on both paths. The MCP docs state the 403.
- **Source**: `crates/utopia-server/src/api/review_ops.rs:42-54`; `crates/utopia-server/src/api/mcp.rs:84-112`; `web/src/docs/mcp.md:212`; test `a_token_whose_kb_ids_exclude_the_base_reads_no_review_data_over_mcp` (`review_mcp_tests.rs:548`)
- **Confidence**: high
- **Why it matters here**: The two surfaces answer the same out-of-scope token with different statuses, and over MCP `review_ops::authorize_read` is never reached for that token.

## Finding: List responses carry no per-section total; totals come from `counts`, whose keys do not all match the listed rows

- **Claim**: `GET /kbs/{id}/review` returns `{counts, queue, items}` with no `total`. `utopia_store::review::counts` returns one `ReviewCounts` row (17 fields) computed in a single query. The web pager uses `counts[active]` for every queue except: `agent` uses `counts.agent_rows` (`counts.agent` is the badge: proposed governor decisions only), and `duplicates` uses `duplicates`, `duplicates_same_type` or `duplicates_type_conflict` by the `types` filter. Decisions uses `total` from `GET .../review/history`, which pages by `page`/`per` (default 20, clamp 1–100), not `limit`/`offset`, and returns `{events, total}`.
- **Source**: `crates/utopia-server/src/api/review_routes.rs:179-183`, `747-772`; `crates/utopia-store/src/review.rs:62-114`; `crates/utopia-core/src/models.rs:1567-1594`; `web/src/pages/Review.tsx:1638-1657`, `2324-2336`; `web/src/api.ts:2604-2619`, `2832-2834`
- **Confidence**: high
- **Why it matters here**: A client that pages by a returned number must pick a different `counts` key per section, and for two sections the key named after the queue is not the row total.

## Finding: The Alignment count omits implication rules that the Alignment list returns

- **Claim**: `alignment_queue::list` returns three item kinds (`phrase`, `kind_word`, `rule`), where `rule` rows are `implication_rules` with `status = 'proposed'`. `review::counts` computes `alignment` from `phrase_bindings` (waiting) plus `type_bindings` (undecided) only. `alignment_queue::waiting` (used by the Overview summary) does include `implication_rules`. The list sorts all three kinds by `decided_at` ascending in memory after fetching every waiting row, then applies `skip(offset).take(limit)`.
- **Source**: `crates/utopia-store/src/alignment_queue.rs:116-224` (list), `227-241` (waiting); `crates/utopia-store/src/review.rs:101-102`
- **Confidence**: high
- **Why it matters here**: With N proposed rules, `counts.alignment` is N lower than the number of listed items, so paging by it stops before the last items; the Overview's alignment count and the badge count can differ on the same base. The list has no SQL `LIMIT`, and its sort key has no id tie-break.

## Finding: The Low confidence count excludes derived facts; the Low confidence list does not

- **Claim**: `review::counts` computes `lowconf` with `confidence < $2 AND derived_by_rule IS NULL`. `graph::low_confidence_facts` lists `f.invalidated_at IS NULL AND f.confidence < $2` with no `derived_by_rule` condition, ordered `f.confidence, f.recorded_at DESC, f.id DESC`. `review_summary` uses the same condition as `counts`.
- **Source**: `crates/utopia-store/src/review.rs:78-80`; `crates/utopia-store/src/graph.rs:2073-2104`; `crates/utopia-store/src/review_summary.rs:92-95`
- **Confidence**: high for the SQL difference; low for whether any live row has `confidence < 0.75` and `derived_by_rule IS NOT NULL` (not queried)
- **Why it matters here**: If such rows exist, the list returns more items than `counts.lowconf`. `LOW_CONFIDENCE_BELOW` is `0.75` (`review.rs:20`).

## Finding: Store function, order and count source for each list queue

- **Claim**: Each queue is one store call taking `(pool, kb_id, limit, offset)` with SQL `LIMIT/OFFSET` (except alignment) and a deterministic order with an id tie-break (except alignment):
  - `pending` → `pending::list` → `Vec<PendingFactView>`, `created_at DESC, id DESC`; count `pending_facts` by `kb_id`. List inner-joins `chunks`.
  - `duplicates` → `resolution::list_reviews(.., TypeFilter, ..)` → `Vec<ReviewItem>` via `assemble_reviews` (sides from `review_sides`, open governor proposal), `created_at DESC, id DESC`; count `resolution_reviews` `status = 'pending'`.
  - `conflicts` → `temporal::list_conflicts` → `Vec<ConflictView>`, `created_at DESC, id DESC`; count `fact_conflicts` `status = 'open'`. List inner-joins both facts, both subjects and `relation_types` on the old fact's predicate.
  - `unconfirmed` → `graph::stale_facts` → `Vec<FactReviewItem>`, `recorded_at DESC, id DESC`; count uses `review::unconfirmed_fact()` (the list restates the same conditions inline).
  - `lowconf` → `graph::low_confidence_facts` → `Vec<FactReviewItem>`.
  - `violations` → `reasoning::open_violations` → `Vec<AxiomViolation>`, `detected_at DESC, id DESC`; count `axiom_violations` `status = 'open'`.
  - `defects` → `reasoning::open_defects` → `Vec<OntologyDefect>`, `detected_at DESC, id DESC`; count `ontology_defects` `status = 'open'`.
  - `alignment` → `alignment_queue::list` → `Vec<AlignmentItem>` (serde tag `kind`).
  - `errata` → `errata::held` → `Vec<HeldItem>`, `created_at, id` ascending; count `errata_actions` `status = 'held'`.
  - `merges` → `resolution::list_merges` → `Vec<MergeLogView>`, `created_at DESC, id DESC`; count all `entity_merges` of the base. List inner-joins source and target entities.
  - `agent` → `token_agent_actions::agent_history` → `Vec<serde_json::Value>`; total `agent_rows`.
  - `mappings` → `mappings::proposed` → `Vec<ConceptMapping>`.
- **Source**: `crates/utopia-server/src/api/review_routes.rs:113-177`; `crates/utopia-store/src/pending.rs:219-233`; `resolution.rs:1881-1930`, `2655-2676`; `temporal.rs:1433-1466`; `graph.rs:2073-2143`; `reasoning.rs:795-900`, `2753-2783`; `alignment_queue.rs:116`; `errata.rs:744-757`; `mappings.rs:135-152`; `review.rs:62-114`
- **Confidence**: high for the SQL as written; medium for whether the list joins ever drop a row the count includes (depends on foreign keys and data, not checked)
- **Why it matters here**: These are the functions a token read of each section would have to reach; the counts are computed by a separate query from the lists in every case.

## Finding: Agent History already merges governor rows and ledger rows in one paged sequence

- **Claim**: `token_agent_actions::agent_history` pages a `UNION ALL` of `agent_decisions` and `token_agent_actions` ordered `created_at DESC, id DESC`; governor rows are the unmodified `governance::list` serialization (`AgentDecisionView`, no `source` key) and ledger rows are built by `history_row` with `"source": "token"`, `operation`, `target_kind`, `target_id`, `owner_id`, `owner_name` (joined from `users` at read time), `token_id`, `token_name` (stored snapshot), `outcome`, `left`, `right`, `rationale`, `created_at`. The page total is `counts.agent_rows` (both tables). The ledger table has columns `id, kb_id, operation, target_kind, target_id, owner_id, token_id, token_name, outcome (applied|held|proposed), payload, created_at`, no foreign keys on owner or token, and `resolution_reviews.decided_token` was added alongside.
- **Source**: `crates/utopia-store/src/token_agent_actions.rs:29-47`, `216-321`; `crates/utopia-store/src/review.rs:90-93`; `migrations/0128_a_token_agent_action_is_kept_in_its_own_ledger.sql`; `web/src/api.ts:698-748`
- **Confidence**: high
- **Why it matters here**: The row shape the roadmap asks a token to read exists on the session path; only `review.keep` rows can exist in the ledger at this revision.

## Finding: The Review tab does not fetch proofs or fact evidence; its cards render the list item only

- **Claim**: `Review.tsx` calls `api.review`, `api.reviewHistory`, `api.reviewSummary` and `api.ontology` (Alignment section only) for reads. `api.derivedProof`, `api.blockedProof`, `api.factEvidence` and `api.entityDetail` are called only from `web/src/pages/Graph.tsx` (lines 2510, 2720, 2859, 3576) and `OntologyRegressionsPanel.tsx:156`. A violation card renders `left_text`, `right_text`, `path` (id + text per fact), `path_len`, `predicate`, `detail` and `hint` from the list item; fact cards render the single `quote` and `evidence_count` on `FactReviewItem`.
- **Source**: `grep -rnE "api\.(factEvidence|derivedProof|blockedProof|entityDetail|...)" web/src --include=*.tsx`; `web/src/pages/Review.tsx:292`, `452`, `780`, `1120-1160`, `1215-1246`, `1411-1437`, `1670-1674`; `crates/utopia-core/src/models.rs:1356-1385`
- **Confidence**: high
- **Why it matters here**: "What the Review tab shows" for an item is the list item's own fields. The proof chain of an Axioms item is shown on the Graph page, and `reasoning::blocked_proof` returns steps only for `kind = 'derived_contradiction'` (`None` otherwise, serialized as `{"steps": null}`); `reasoning::proof` takes a derived-fact id, not a violation id, and returns `{"proof": null}` when the derivation is missing (`crates/utopia-store/src/reasoning.rs:2870-2881`, `3137-3154`).

## Finding: The MCP refusal path renders the REST error and adds `not_found` for 404

- **Claim**: `review_tools::refusal` converts an `AppError` by rendering `ApiErr(error).into_response()`, reading `error` and `code` from the body, and substituting `code: "not_found"` when the status is 404; success puts the REST JSON body in `structuredContent` and its serialization in the text content. Argument-type errors are plain-text tool errors with no code. Every Review tool call, refused or not, writes one `mcp.tool_called` audit row in `mcp::handle`.
- **Source**: `crates/utopia-server/src/api/review_tools.rs:158-235`; `crates/utopia-server/src/api/mcp.rs:205-233`
- **Confidence**: high
- **Why it matters here**: New read operations added to `review_ops` inherit code parity with REST without a second table; `unknown_queue` already travels this way.

## Finding: The Decisions history returns audit rows without token fields

- **Claim**: `audit::review_history` selects `id, action, target_kind, target_id, detail, actor_id, actor_name, created_at` for actions matching `review.%`, `fact.%`, `conflict.%` or `merge.%`, and returns `(rows, total)`. It does not select `token_id`/`token_name`; `AuditEventView` marks those fields `#[sqlx(default)]` and skips them when `None`, so they are absent from history rows (the token id and name remain inside `detail` for a token keep). `audit::list_for_kb` (Activity, base Admin, `GET /kbs/{id}/audit`) does select them plus `declared_client`.
- **Source**: `crates/utopia-store/src/audit.rs:129-155`, `170-222`; `crates/utopia-core/src/models.rs:438-461`
- **Confidence**: high
- **Why it matters here**: A token keep appears in Decisions as a `review.keep` row whose actor is the owner; the token is visible there only through `detail`.

## Finding: One list response always computes all counts

- **Claim**: Both the session branch and `review_ops::list` call `review::counts` on every list request; the query contains 17 sub-selects including the unconfirmed-fact predicate over `facts` and an `EXISTS` on `jobs`.
- **Source**: `crates/utopia-server/src/api/review_routes.rs:107`; `crates/utopia-server/src/api/review_ops.rs:94`; `crates/utopia-store/src/review.rs:62-114`
- **Confidence**: high
- **Why it matters here**: Observation only; cost per list page was not measured.

## Finding: Recorded project verification used two databases, one for the store crate and one for the rest

- **Claim**: The last verify-ledger entry ran the workspace tests excluding `utopia-store` against `m001_project` and the `utopia-store` tests against a separately migrated `m001_project_store`, both created with `docker exec utopia-db-1 psql ...` on `127.0.0.1:1517`, with `UTOPIA_TEST_REQUIRE_DB=1`, exit code 0 at `cbffeeb`.
- **Source**: `D:\utopia\.project\archive\001-token-review-skeleton\build\verify-ledger.jsonl` (last line, `command` and `execution.exit_code`); `scripts/prove-db-isolation.sh:1-60`
- **Confidence**: high that the entry records this; the reason for the split was not established from the code read here
- **Why it matters here**: Database-backed verification on this machine has a known working recipe, and it is not a single `cargo test --workspace` against one database.

## Finding: Carried forward from the previous map and still true at this revision

- **Claim**: (a) `utopia-server` embeds `web/src/docs/*.md` at compile time, so `web/src/docs/mcp.md` is both product documentation and a Rust build input. (b) `web/tests/*.test.mjs` are not run by `pnpm test` or CI. (c) `origin` is the fork `evolv3ai/utopia` with default branch `dev`; manifests name `deeplethe/utopia`; `scripts/check-upstream-numbers.sh` reads upstream `deeplethe/utopia` `dev` and open PRs over the network. (d) `AGENTS.md`, `WORKFLOW.md`, `.gsd-path/` and `.claude/` are untracked per-clone files absent from the sidecar. (e) `docs/` is git-ignored except `docs/decisions/`, `docs/design/`, `docs/pipeline.md`.
- **Source**: previous map (`.project/archive/001-token-review-skeleton/research/evidence-codebase.md`); `git diff --stat 3d307f6 HEAD` shows no change to `.gitignore`, `docker/`, `.github/`, `web/vite.config.ts`; `scripts/check-upstream-numbers.sh:1-25`; sidecar root listing
- **Confidence**: medium (re-confirmed by absence of changes, not re-derived line by line)
- **Why it matters here**: A documentation task on `web/src/docs/mcp.md` changes a file the server build reads; numbers for any new ADR or migration are checked against a remote this scan did not contact.

## Apparent intent

- Inference: the program is opening the Review tab to personal-token agents in slices, with reads first and gated writes later — based on the roadmap, ADR 0076 decisions 1–12, the `AgentOperation` enum listing 19 operations of which only `Keep` is reachable (`crates/utopia-store/src/execution_gate.rs:121-195`), and comments such as "这一刀只开 `duplicates` 一档".
- Inference: the intended shape for each new token operation is one `review_ops` function called from both a handler's token branch and `review_tools::call`, leaving the session branch byte-for-byte — based on the module docs of `review_ops.rs` and `review_tools.rs` and on `review_session_unchanged_tests.rs`.
- Inference: "true total per section" is meant to close the count/list gaps recorded above (Alignment rules, Agent rows, Low confidence derived facts) — based on the roadmap wording and the `agent_rows` precedent added in M001 (`review.rs:90-93`).

## Open questions for define

- "The same items a signed-in editor sees" for the Low confidence section: the list includes derived low-confidence facts and the count does not. Which one is the section — should the total follow the list as it is, or is the list expected to change?
- Should the section total be a new field on the list response (and on which sections: all eleven queues, or also `overview`/`decisions`), or a correction of the existing `counts` keys? A corrected `counts.alignment` would also change the session badge.
- Decisions pages by `page`/`per` and returns `{events, total}`; the queues page by `limit`/`offset`. Should the token and MCP read of Decisions keep the route's own paging or be presented with `limit`/`offset` like the other sections?
- Success criterion 3 names "the proof chain of an Axioms item", but the Review tab itself shows only the list item; proofs are fetched on the Graph page and exist only for `derived_contradiction` violations. What must `review_item` return for the other violation kinds (`cycle`, `functional`, `asymmetry`, ...) and for Ontology (`defects`) items?
- `review_item` takes one `id` today and means a duplicate pair. Items in other sections are identified differently (a kind word is a string, not a uuid; an Agent History row may be a governor decision or a ledger row). Is one tool with a section/kind argument intended, and is an item that is no longer waiting in its section "not found" as for duplicates?
- An out-of-scope token gets HTTP 403 from the MCP endpoint and 404 from the token API. Is that difference accepted for M002's criterion 5 ("cannot list or read it over MCP"), as it was for M001?
- `mappings` is a list queue with a count but not one of the thirteen sections. Should a token asking for `queue=mappings` keep receiving `unknown_queue`?
- Should M002's verification reuse the M001 recipe (two per-run databases on the compose Postgres, `UTOPIA_TEST_REQUIRE_DB=1`), and is seeding one item per section expected through store functions in tests rather than through the product's jobs?

## Blocked areas

- Compilation, lint and test state at HEAD — no cargo or pnpm command was run in this scan; the cited ledger entry is for commit `cbffeeb`, whose non-`.project` tree is identical to HEAD by `git diff`. `cargo clippy` status is `unverifiable`.
- Live data questions (whether any base has low-confidence derived facts, proposed implication rules, or list rows dropped by joins) — no database was queried.
- `web/tests/*.test.mjs` browser tests, live-service tests (OCR, MySQL, Trino, ClickHouse, RSS), Docker image build, `scripts/smoke.sh` — not run.
- Upstream `deeplethe/utopia` state (branches, open PRs, claimed ADR and migration numbers) — not contacted; nothing was sent to any remote.
- Files read only in part: `resolution.rs`, `reasoning.rs`, `governance.rs`, `temporal.rs`, `graph.rs`, `errata.rs`, `Review.tsx`, `api.ts`, ADR 0076 (headings and decision titles only), `review_summary.rs` (grep only). Statements about them are limited to the cited lines.
