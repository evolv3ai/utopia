# Evidence — codebase

Repo root: D:\utopia
Scanned: 2026-11-19
Checks run: all in the verify sidecar at `6c52ce8f8091b18d8713ce8d509fec756644a4f1` (read-only commands only).
`git rev-parse HEAD` → `6c52ce8f8091b18d8713ce8d509fec756644a4f1`; `git status --short` → clean;
`ls migrations | wc -l` → 114; `ls migrations | tail` → last file `0128_a_token_agent_action_is_kept_in_its_own_ledger.sql`;
`git log --oneline -25` → listed below; `git log -12 | grep "Signed-off-by\|Co-Authored-By"` → no match in 12 commits;
`rustc --version` → 1.99.1; `cargo --version` → 1.99.1; `node --version` → v26.13.1; `pnpm --version` → 10.14.0;
`docker ps` → `utopia-db-1`, image `pgvector/pgvector:pg16`, `127.0.0.1:1517->5432`, up and healthy.
Not run: `cargo test`, `cargo build`, `cargo clippy`, `pnpm test`, `pnpm build`. The sidecar has no `target/` and no
`web/node_modules`, and build output may not stay in it. Every claim below about test behaviour is a static reading
of the test source, not an execution; pass/fail counts at this revision are `unverifiable` in this inspection.
Two further `git` commands were refused by the repository's pre-tool guard and were not retried.

## Map

- **Stack**: Rust workspace, edition 2021, version 0.1.0 (`Cargo.toml:16-17`), nine crates under `crates/`
  (`utopia-cli`, `-core`, `-extract`, `-ingest`, `-llm`, `-reason`, `-search`, `-server`, `-store`). axum 0.8,
  axum-extra 0.10, tower-http 0.6, sqlx 0.8, tokio 1, jsonwebtoken 10, pgvector 0.4, reqwest 0.13
  (`Cargo.toml:25-75`). Postgres 16 with pgvector (CI image `pgvector/pgvector:pg16`,
  `.github/workflows/checks.yml:55`). Web: React 19.3, Vite 6.4, TypeScript 5.7, vitest 5.0.0, TanStack Query 5 and
  Router 1, Tailwind 4, `packageManager` pnpm 10.2.1 (`web/package.json`). No `rust-toolchain` file; CI uses
  `dtolnay/rust-toolchain@stable` and Node 20 (`checks.yml:31,178`). Host toolchain differs: rustc 1.99.1, Node 26.13.1, pnpm 10.14.0.
- **Entry points**: `crates/utopia-server/src/main.rs` (HTTP server on :1516, job worker, hourly timers; the govern
  sweep is at `main.rs:269-295`, the `"govern"` job arm at `main.rs:644`). Router in
  `crates/utopia-server/src/api/mod.rs` (Review routes `mod.rs:534-617`, MCP `mod.rs:240`, audit `mod.rs:182`).
  `crates/utopia-cli/src/main.rs` (backup, restore; `CURRENT_SCHEMA_VERSION` at line 85).
  `cargo run -p utopia-store --example migrate` migrates a database. Web entry `web/src/main.tsx`, router `web/src/router.tsx`.
- **Architecture**: `utopia-store` holds all SQL (one module per domain; `resolution.rs`, `governance.rs`,
  `execution_gate.rs`, `token_agent_actions.rs`, `audit.rs`, `tokens.rs`, `access.rs`). `utopia-server` holds axum
  handlers in `src/api/*_routes.rs`, background deciders (`adjudication.rs`, `governance.rs`) and the MCP endpoint
  (`api/mcp.rs`). A session request goes extractor (`AuthUser` or `SessionOrToken`, `auth.rs:157,191`) → `require_kb`
  role check → store function → best-effort `audit::record` → `state.emit_review`. A token Review request branches
  inside the same handler to `api/review_ops.rs`, which authorizes, consults `execution_gate::agent_policy`, and
  calls the store; MCP Review tools (`api/review_tools.rs`) call the same `review_ops` functions. The server crate
  has no `lib.rs`; tests are sibling `*_tests.rs` files declared from `api/mod.rs:695-710` or by `#[path]`.
- **Conventions**: Chinese explanatory comments throughout Rust and TSX; English user-facing strings and tool
  descriptions. Sentence-named migrations (`0128_a_token_agent_action_is_kept_in_its_own_ledger.sql`), tests
  (`a_token_keep_is_never_human_precedent.rs`) and ADRs (`docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md`).
  Errors are `utopia_core::AppError` (`crates/utopia-core/src/error.rs`), rendered by `ApiErr`
  (`crates/utopia-server/src/error.rs:18-69`) as `{error, code?, detail?}`. Database tests start with
  `utopia_store::test_db::url()` and skip when `UTOPIA_DATABASE_URL` is unset, or panic when
  `UTOPIA_TEST_REQUIRE_DB` is set (`crates/utopia-store/src/test_db.rs:17-33`). Server tests build their own
  organization and bases and call `cleanup()`. Web tests are node-environment vitest files `src/**/*.test.ts`
  (`web/vite.config.ts:17-21`). Commit subjects on this fork: `T00N: <behaviour sentence>` for task commits and
  `build:`/`plan:`/`ship:`/`integrate:` for pipeline commits. CONTRIBUTING requires `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
  `cd web && pnpm install --frozen-lockfile && pnpm build` (`CONTRIBUTING.md:60-63`), an ADR before a data-model or
  public-API change (`:39`), unique migration numbers (`:114`), and DCO sign-off (`:130-143`).
- **Maturity**: 114 migrations, 71 ADR files in `docs/decisions/`. `crates/utopia-server/src/api/` holds about
  43,000 lines, of which the six `review_*_tests.rs` files and `review_session_unchanged_tests.rs` are about 6,200.
  The token Review surface at this revision is: reads of all thirteen sections, one listed item, two proof reads,
  and one write (keep). Token merge, revert, batch, the proposal store, a rate limiter and a breaker do not exist
  in code. The policy table for all nineteen operations is compiled in, with one caller (Keep).
- **Recent activity**: the last 25 commits are M001 and M002 of this program: `T001`…`T004` of M002 (section totals,
  eleven queue sections plus Decisions and proofs over HTTP, MCP `review_list`/`review_item` for thirteen sections,
  documentation and ADR 0076 amendments), `ship: M002`, `integrate: M002 — merge gsd-path/M002 into dev`, and before
  them `T007` (backup manifest schema version) and `ship: M001`. Work clusters in
  `crates/utopia-server/src/api/review_*`, `crates/utopia-store/src/token_agent_actions.rs`,
  `crates/utopia-store/src/execution_gate.rs`, `web/src/docs/mcp.md`, `docs/decisions/0076-…md`.

## Finding: The token Review path is one operation layer, and its only write is keep

- **Claim**: `crates/utopia-server/src/api/review_ops.rs` (316 lines) is the shared layer. `ReviewCaller {user, token, kb_id}`
  at lines 25-29; `not_open()` at 32-38 returns `CodedForbidden` code `token_operation_not_open`; `authorize_read`
  43-55 (owner at least Viewer and `token.covers(kb)`, otherwise `NotFound`); `authorize_write` 60-75
  (`require_token_write` first, then Editor, mapping plain `Forbidden` to code `token_owner_below_editor`);
  `TOKEN_SECTIONS` 80-92 (eleven names); `unknown_queue` 95-97; `list` 103-116; `history` 121-133;
  `violation_proof` 138-147; `derived_proof` 151-159; `item` 192-277 (scans the list pages, no by-id query);
  `summary` 280-286; `keep` 293-316. `keep` returns `{ok, outcome, action_id}` and does not call
  `governance::after_human_decision`.
- **Source**: `crates/utopia-server/src/api/review_ops.rs:25-316`
- **Confidence**: high
- **Why it matters here**: every M003 token operation has an existing place and shape to follow; nothing for merge,
  revert, batch or proposals is stubbed there.

## Finding: Which Review handlers take a token today

- **Claim**: In `crates/utopia-server/src/api/review_routes.rs`, five handlers take `SessionOrToken`: `list` (85-112),
  `decide` (351-420), `history` (801-820), `summary` (825-839) and `decide_alignment_kind_word` (1468-1524). All
  others take `AuthUser`: `close_fact` 252, `resolve_conflict` 313, `agent_answer` 434, `batch` 567,
  `confirm_fact` 641, `reject_fact` 665, `revert_merge` 689, `manual_merge` 737, `decide_mapping` 852,
  `decide_violation` 905, `run_consistency_check` 1056, `decide_defect` 1111, `pending_for_chunk` 1155,
  `decide_pending` 1177, `run_inference` 1240, `decide_alignment_rule` 1312, `decide_alignment_phrase` 1350,
  `decide_errata` 1532. `AuthUser` decodes every Bearer value as a JWT (`auth.rs:159-172`), so a personal token
  on those routes gets 401. The two proof routes take a token elsewhere (through `review_ops::violation_proof` /
  `derived_proof`).
- **Source**: `crates/utopia-server/src/api/review_routes.rs` (lines above); `crates/utopia-server/src/auth.rs:157-217`
- **Confidence**: high
- **Why it matters here**: the three routes M003 opens (`/review/batch`, `/merges/{id}/revert`, `/entities/merge`)
  are session-only extractors today; opening them changes the extractor and adds a token branch, as `decide` did.

## Finding: The token branch of the decision route refuses every action except keep, after the write checks

- **Claim**: In `decide`, a token caller with `body.action != "keep"` runs `review_ops::authorize_write` and then
  returns `review_ops::not_open()` (403, `token_operation_not_open`). The test is on the string, so `"merge"` and
  any unknown action get the same answer; a read-scope token asking for merge gets `token_read_only`.
- **Source**: `crates/utopia-server/src/api/review_routes.rs:357-369`
- **Confidence**: high
- **Why it matters here**: this is the exact branch a token queue-merge would replace; an unknown action for a token
  is currently "not open", not a validation error.

## Finding: The policy function and ruled table live in the store's execution gate module

- **Claim**: `crates/utopia-store/src/execution_gate.rs` defines `AgentOperation` (nineteen variants, 124-146),
  `AgentRuling {Finalize, FinalizeBehindGate, ProposeOnly}` (150-157), `AgentOutcome {Applied, Held, Proposed}` with
  `as_str` (161-175), `agent_ruling` (181-196, no wildcard arm) and `agent_policy(op, hold: Option<&Hold>)` (200-209).
  `Merge | RevertMerge | Batch` are `FinalizeBehindGate`; `agent_policy` answers `Held` when a hold is passed and
  `Applied` when it is `None`. The only callers outside the module are `review_ops::keep`
  (`review_ops.rs:301`) and `token_agent_actions::keep_duplicate_pair` (`token_agent_actions.rs:67`), plus the
  table test in `crates/utopia-store/tests/a_token_keep_is_never_human_precedent.rs:983-1043`.
- **Source**: `crates/utopia-store/src/execution_gate.rs:121-209`; grep for `agent_policy|AgentOperation` across `crates/`
- **Confidence**: high
- **Why it matters here**: the table already carries M003's three rows; `agent_policy` takes a `Hold` only, so a
  "revert of a person's merge" or a breaker-forced proposal has no input to it today, and `AgentOutcome::Proposed`
  is reachable only through a `ProposeOnly` row.

## Finding: The ADR 0027 merge gate has four hold reasons and consults neither kept pairs nor reverted merges

- **Claim**: `hold_for_merge(pool, kb_id, a, b) -> AppResult<Option<Hold>>` (`execution_gate.rs:213-241`) first
  checks an identity source (a live built-in `known_as` fact on either side whose evidence comes from a source with
  `kind = 'statements'` and `config->>'names' = 'identity'`) → `Hold::Identity`; then `hold(&impact_of(..))`.
  `impact_of` (247-283) runs three reads: `temporal::merge_would_overlap` (functional contradictions, time-aware),
  a count of live `derived_facts` with either side as subject or object, and a count of assistant
  `conversation_messages` whose `resolved` names either side. `hold` (108-119) returns, in order,
  `Contradiction(label)`, `Derived(n)`, `Answered(n)`, or `None`. `Hold::review_reason()` gives `escalate_identity`
  or `escalate_impact|<kind> <value>` (54-59); `Hold::explain()` gives an English sentence (62-71). No query in the
  module reads `resolution_reviews` or `entity_merges`.
- **Source**: `crates/utopia-store/src/execution_gate.rs:35-283`
- **Confidence**: high
- **Why it matters here**: four of the six hold reasons in M003's second success criterion exist; "a pair a person
  kept" and "a pair with a reverted merge" are not implemented anywhere in the gate, and `Hold` has no variant for them.

## Finding: The gate has exactly two callers, both machine deciders; no person's path calls it

- **Claim**: `hold_for_merge` is called at `crates/utopia-server/src/adjudication.rs:396` (batch adjudicator, in
  `apply_verdict`, escalating the review with `hold.review_reason()`) and `crates/utopia-server/src/governance.rs:575`
  (governor, which escalates and records a `proposed` `agent_decisions` row whose reason begins
  `held for a person: …`). `impact_of` is also called at `governance.rs:842` for the governor's `consequences`
  tool. `decide`, `batch`, `agent_answer`, `manual_merge` and `resolution::decide_review` call no gate.
- **Source**: grep `hold_for_merge|impact_of\(` over `crates/`; `adjudication.rs:392-411`; `governance.rs:572-592`; `review_routes.rs:384-392,594-602,759-767`
- **Confidence**: high
- **Why it matters here**: "the same store function the person's path calls" is ungated by construction; the gate
  call has to be added on the token branch, and the session path stays as it is.

## Finding: A person's queue merge is read-then-write with no status guard, and its audit row is best-effort

- **Claim**: `resolution::decide_review` (`resolution.rs:2028-2101`) selects the row `WHERE status = 'pending'`
  (else `NotFound`), refuses with a plain `Conflict` string when `governance::locked_by_agent` is true, follows
  `merged_into` through `survivor` on both sides, calls `merge_entities` when the survivors differ, then runs
  `UPDATE resolution_reviews SET status = 'merged' … WHERE id = $1` with no status predicate and outside any
  transaction spanning the merge. It does not set `decided_token`. The handler takes a name snapshot with
  `.ok().flatten()`, writes the audit row with `let _ = audit::record(..)`, and writes none when the snapshot is
  `None` (`review_routes.rs:371-409`).
- **Source**: `crates/utopia-store/src/resolution.rs:2028-2101`; `crates/utopia-server/src/api/review_routes.rs:370-419`
- **Confidence**: high
- **Why it matters here**: a lost race on this path surfaces as `NotFound` or as `Conflict("Entity already merged")`
  from `merge_entities` (`resolution.rs:2197-2199`), not as `already_decided`; and the token keep path was written
  separately (one transaction, row lock, guarded UPDATE, `audit::record_on`) for exactly this reason
  (`token_agent_actions.rs:87-178`).

## Finding: What `merge_entities` changes

- **Claim**: `merge_entities(pool, kb_id, source_id, target_id, merged_by: Option<Uuid>, reason) -> AppResult<Uuid>`
  (`resolution.rs:2181-2487`) refuses a self-merge (`Invalid` code `self_merge`) and a side already merged
  (`Conflict`). In one transaction it locks the affected timelines; invalidates facts between the two entities;
  repoints `facts.subject_id` and `facts.object_id` from source to target; invalidates later duplicates grouped by
  subject, predicate, object, object value and `valid_from`, after copying their `typed_fact_sources` and
  `fact_evidence` rows onto the surviving fact; drops name vectors where identity evidence was copied; sets the
  target's `profile_embedding`, `profile_n` and `type_id` (`target.type_id.or(source.type_id)`, line 2345); sets
  `source.merged_into`; closes as `kept` with reason `superseded by merge` every other pending review that would
  become a self-pair or a duplicate of an existing pending pair (2374-2396); repoints the remaining pending reviews
  of the source to the target, appending `|redirected` to their reason (2400-2413); and inserts the `entity_merges`
  row with the moved, invalidated and copied id arrays and the target's profile and type before the merge
  (2415-2438). After commit, outside the transaction, it records signature violations into `axiom_violations`,
  runs `temporal::reconcile_moved_facts`, stores `temporal_corrections` on the merge row, and refreshes
  disambiguators (2448-2486).
- **Source**: `crates/utopia-store/src/resolution.rs:2181-2487`
- **Confidence**: high
- **Why it matters here**: a merge writes to `facts`, `fact_evidence`, `typed_fact_sources`, `entities`,
  `resolution_reviews` (other pairs), `entity_merges`, `axiom_violations` and name vectors; the post-commit steps
  mean a merge can return an error after the entities are already merged.

## Finding: `revert_merge` does not restore everything a merge changed

- **Claim**: `revert_merge(pool, kb_id, merge_id) -> AppResult<()>` (`resolution.rs:2537-2666`) answers `NotFound`
  for an unknown merge and a plain `Conflict("Merge already reverted")` when `reverted_at` is set (2549-2551). It
  moves the ledgered facts back, re-homes live rows rewritten after the merge as new rows on the source
  (`temporal::rehome_tx`, 2593-2615), deletes the copied evidence, clears `invalidated_at` on the ledgered
  invalidated facts, re-tidies the timelines, restores the target's profile, clears `source.merged_into` and sets
  `reverted_at`. It does not touch `resolution_reviews`: the decided pair stays `merged`, reviews closed as
  `superseded by merge` stay `kept`, and redirected reviews keep the target's id and the `|redirected` suffix. The
  target's type is restored with `type_id = coalesce($4, type_id)` (2643-2646), so a target that had no type before
  the merge keeps the type it took from the source. Signature violations recorded after the merge are not removed.
  `entities.updated_at` changes on both sides.
- **Source**: `crates/utopia-store/src/resolution.rs:2537-2666`, with the merge side at 2345, 2374-2413, 2459-2469
- **Confidence**: high for the code reading; medium for how often the type case occurs in practice
- **Why it matters here**: M003's criteria 5 and 7 compare "facts, entities and evidence" before and after. A
  byte-equal comparison of `entities` (as `review_token_api_tests.rs:36-50` does with `to_jsonb(t)`) cannot hold
  across a merge and its revert because of `updated_at`, and an untyped target is a real state difference. Existing
  store tests (`a_revert_gives_both_timelines_back_as_they_were`, `merging_moves_names_and_reverting_brings_them_back`,
  `undoing_a_merge_leaves_the_survivors_own_evidence`) show what the project itself asserts about revert.

## Finding: A merge row records a user id and a free-text reason, nothing about a token

- **Claim**: `entity_merges` has `merged_by UUID REFERENCES users(id)` (NULL means automatic) and `reason TEXT`
  (`migrations/0005_resolution.sql:41-60`, plus two columns from 0125). `MergeLogView` exposes `id, source_name,
  target_name, merged_by_name, reason, created_at, reverted_at` (`crates/utopia-core/src/models.rs:1080-1089`);
  `list_merges` joins `users` on `merged_by` (`resolution.rs:2669-2691`). The web `MergeRow` prints
  `mergedBy(name)` or `mergedByAi` and a Revert button whenever `reverted_at` is null (`web/src/pages/Review.tsx:484-522`).
  `token_agent_actions` has no column referring to a merge (`migrations/0128_…sql:18-37`).
- **Source**: files and lines above
- **Confidence**: high
- **Why it matters here**: "the merge was made by a token agent" cannot be read from any stored row today; a token
  merge written with `merged_by = owner` would show in the Merges section as that person's merge.

## Finding: The session batch route is sequential, capped at 500, and can drop audit rows silently

- **Claim**: `batch` (`review_routes.rs:567-639`) takes `{ids, action, rationale}`, requires Editor, and refuses an
  empty list or more than `BATCH_MAX = 500` (line 563) with `Invalid` code `batch_size`. It reads name snapshots
  with `.unwrap_or_default()` (593), calls `resolution::decide_reviews`, which loops `decide_review` per id and
  returns `Vec<ReviewBatchOutcome {id, error: Option<String>}>` with the error as its display text
  (`resolution.rs:1946-1966`, `models.rs:1628-1631`). For each successful id it writes one audit row
  `review.merge` or `review.keep` with `detail.batch = ids.len()` — only when a snapshot was found, and with the
  result discarded (608-622). It then calls `after_human_decision` once for the decided ids and answers
  `{decided, outcomes}`. An unknown action is refused for the whole batch as `Validation`. There is no batch id and
  no batch undo.
- **Source**: `crates/utopia-server/src/api/review_routes.rs:552-639`; `crates/utopia-store/src/resolution.rs:1943-1966`
- **Confidence**: high
- **Why it matters here**: the existing per-item outcome carries no code and no outcome kind (applied, proposed),
  and `detail.batch` is a size, not an identity; the session shape is pinned by
  `review_session_unchanged_tests.rs:424-440`.

## Finding: The session revert route calls the fuse path for every reverted merge

- **Claim**: `revert_merge` handler (`review_routes.rs:689-725`) requires Editor, snapshots names, calls
  `resolution::revert_merge`, writes a best-effort `merge.revert` audit row `{source, target}`, then calls
  `governance::after_revert(state, kb, merge_id, user.id)`. `after_revert` (`governance.rs:970-976`) calls
  `gov::settle_by_merge`, which updates an `agent_decisions` row `WHERE merge_id = $2 AND status = 'applied'` to
  `reverted` (`crates/utopia-store/src/governance.rs:1104-1121`); only when a row was updated does it call `fuse`.
  The other revert entry is `agent_answer` with `("applied", "revert")` (`review_routes.rs:476-498`), which calls
  `fuse` directly.
- **Source**: lines above
- **Confidence**: high
- **Why it matters here**: a person's revert of a merge that has no `agent_decisions` row (a person's merge, or a
  future token merge) does not reach the fuse. The fuse counts governor rows only.

## Finding: The governance fuse

- **Claim**: `fuse(state, kb_id)` (`crates/utopia-server/src/governance.rs:911-967`) returns unless the base has
  `governance` on; counts `agent_decisions` rows with `status = 'reverted'` and `decided_at >= since`, where
  `since` is the later of `governance_since` and now minus `FUSE_WINDOW_DAYS = 7`
  (`store/governance.rs:1073-1087`); at `FUSE_REVERTS = 2` or more it calls `gov::trip` (sets
  `knowledge_bases.governance = FALSE`), raises alert kind `GOVERNANCE_TRIPPED` for Editors, and writes a
  `kb.updated` audit row with a null actor and `via: "fuse"`.
- **Source**: `crates/utopia-server/src/governance.rs:908-967`; `crates/utopia-store/src/governance.rs:1072-1100`
- **Confidence**: high
- **Why it matters here**: the constants M003's per-token breaker reuses are `gov::FUSE_WINDOW_DAYS` and
  `gov::FUSE_REVERTS`; the fuse's unit is the base, and its effect is a switch on the base, not on an actor.

## Finding: The governor, its proposals and its decision route

- **Claim**: `govern` (`server/governance.rs:103-151`) runs per base under a Postgres advisory lock, up to
  `MAX_ROUNDS = 20` rounds of up to `CLUSTERS_PER_ROUND = 3` clusters; it marks the taken reviews
  `stage = 'adjudicating'` (`gov::lock`), which `locked_by_agent` reads together with "governance on" and "a
  running `govern` job" (`store/governance.rs:967-982`). `apply` records each look in `agent_decisions` as
  `applied` (with `merge_id` for a merge) or `proposed` (`server/governance.rs:553-642`). `gov::gate`
  (`store/governance.rs:554-582`) returns `Propose` when there is any revert precedent touching either name, when
  a person decided the same name pair the other way, on a type-family conflict, or on a version or phrase name
  shape; otherwise it compares confidence with `SUPPORTED_CONF = 0.75` when precedents support the verdict and
  `AUTO_CONF = 0.85` when they do not. `agent_decisions` allows `target_kind = 'review'` only, actions
  `merge|keep|unsure`, six statuses, and one open proposal per target
  (`migrations/0035_governance_reads_the_ledger.sql:14-40`). `agent_answer` (`review_routes.rs:434-550`, `AuthUser`)
  accepts or overrides a proposal through `decide_review`, reverts an applied merge, or merges an applied keep, and
  settles the row. Govern rounds are queued from `after_human_decision` (`server/governance.rs:982-1013`), from
  extraction (`extraction_open.rs:1281`), from the base settings route (`api/kbs.rs:231`) and hourly
  (`main.rs:269-295`).
- **Source**: lines above
- **Confidence**: high
- **Why it matters here**: the Duplicates card already shows one kind of open proposal
  (`ReviewItem.proposal`, filled from `agent_decisions` in `assemble_reviews`, `resolution.rs:1803-1838`); a
  token-agent proposal on the same card is a second, differently shaped thing.

## Finding: Token decisions are excluded from precedent by two markers, and a reverted merge is counted without either

- **Claim**: Precedent queries exclude audit rows that have a `token_id` key (`NOT_BY_TOKEN`,
  `store/governance.rs:85`, used at 101, 114, 128, 827) and review rows with `decided_token IS NOT NULL`
  (`type_pair_stats` 171, `decided_before` 595). `type_pair_stats.reverted` counts
  `entity_merges … WHERE reverted_at IS NOT NULL` for the type pair with no reference to who merged or who reverted
  (179-190), and `Precedents::supports(true)` requires `reverted == 0` (536-548). The `reverts` precedent family
  reads `merge.revert` audit rows touching either name, excluding only rows with a `token_id` (126-136); any
  non-empty `reverts` makes `gate` return `Propose` and `settled_by_people` return `None` (564, 617).
- **Source**: `crates/utopia-store/src/governance.rs:74-196, 536-629`
- **Confidence**: high for the code; medium for the size of the effect on a real base
- **Why it matters here**: a token merge that is later reverted raises the governor's bar for that type pair, and a
  person's revert of a token merge becomes a revert precedent for both names. Neither lowers the bar, but both are
  token-originated rows steering the governor, which ADR 0076's open question "the precedent exclusion has not
  been exercised end to end" (`docs/decisions/0076-…md:351-353`) leaves open.

## Finding: A kept pair is not remembered by the review queue

- **Claim**: `resolution_reviews` is unique per pair only `WHERE status = 'pending'`
  (`migrations/0005_resolution.sql:23-25`), and `create_review` inserts with `ON CONFLICT` on that partial index
  (`resolution.rs:1605-1629`). A pair decided `kept` or `merged` can therefore get a new pending row later. The
  governor reads the earlier decision through `decided_before` (latest decided row for the same two entity ids by
  a person); nothing else reads it.
- **Source**: `migrations/0005_resolution.sql:10-27`; `crates/utopia-store/src/resolution.rs:1604-1629`; `crates/utopia-store/src/governance.rs:587-606`
- **Confidence**: high
- **Why it matters here**: "a pair a person kept" has to be looked up by entity ids across earlier rows; a manual
  merge names two entities and has no review id at all.

## Finding: Manual merge does not close the pair's own pending review

- **Claim**: `manual_merge` (`review_routes.rs:737-783`, body `{source, target, rationale}`, answer `{merge_id}`)
  calls `merge_entities` with the caller's direction and writes a best-effort `merge.manual` audit row. Inside
  `merge_entities` both review updates exclude the `(source, target)` pair itself (`resolution.rs:2379-2380,
  2406-2407`), "marked merged by the caller"; `manual_merge` does not mark it. A pending review for that pair stays
  pending with one side merged. It does not call `after_human_decision`.
- **Source**: lines above
- **Confidence**: medium (static reading; not executed)
- **Why it matters here**: the two merge entries of M003 (queue decision and manual merge) differ in target
  (review id against two entity ids), in who picks the direction (`merge_direction` against the caller), and in
  what happens to the review row.

## Finding: The ledger of token agent actions

- **Claim**: Migration `0128_a_token_agent_action_is_kept_in_its_own_ledger.sql` creates `token_agent_actions`
  with `id, kb_id (FK, cascade), operation, target_kind, target_id, owner_id, token_id, token_name, outcome
  CHECK IN ('applied','held','proposed'), payload JSONB, created_at`, index `(kb_id, created_at DESC, id DESC)`,
  and adds `resolution_reviews.decided_token UUID`. `owner_id` and `token_id` are bare UUIDs without foreign keys.
  There is no later-state, gate-reason, batch-id, undo-reference or answering-person column and no unique index on
  open proposals. `keep_duplicate_pair` (`token_agent_actions.rs:56-180`) checks the policy, checks
  `locked_by_agent` before opening the transaction, locks the review row `FOR UPDATE`, returns `NotFound` or
  `CodedConflict already_decided`, reads the token name from `personal_tokens`, updates the review with a status
  guard, inserts the ledger row (`operation 'review.keep'`, `target_kind 'review'`, payload
  `{left, right, score, why}`), and writes the audit row with `audit::record_on` on the same transaction
  (detail adds `token_id`, `token_name`). `list`, `count`, `agent_history_total` and `agent_history` (190-330)
  read it; `agent_history` pages the union of `agent_decisions` and the ledger by `created_at DESC, id DESC` and
  serializes ledger rows through `history_row` (219-236) with `source: "token"`.
- **Source**: `migrations/0128_a_token_agent_action_is_kept_in_its_own_ledger.sql`; `crates/utopia-store/src/token_agent_actions.rs:1-330`
- **Confidence**: high
- **Why it matters here**: the migration's header says each further column "arrives with the cut that first writes
  it" (lines 12-13); M003 writes all of them. `history_row` reads `left`, `right` and `why` from the payload, which
  is keep-shaped.

## Finding: How Agent History and Activity read token rows

- **Claim**: Agent History is `queue=agent` in `queue_page` (`review_routes.rs:199-205`), calling
  `token_agent_actions::agent_history` and `agent_history_total`; `counts.agent` is the number of open governor
  proposals and `counts.agent_rows` adds the ledger count (`crates/utopia-store/src/review.rs:87-100`). Activity is
  `GET /kbs/{id}/audit` → `kbs::audit_log` (`api/kbs.rs:383-410`), `AuthUser` with base role Admin, calling
  `audit::list_for_kb`, which returns `token_id`, `token_name` and `declared_client` (the stored `user_agent`, only
  on rows with a token id) from `audit_events.detail` (`store/audit.rs:191-201`); `AuditEventView` carries them as
  optional fields (`models.rs:439-465`). The Decisions read `audit::review_history` does not select them
  (`audit.rs:129-154`). Web: `TokenAgentRow` (`web/src/pages/TokenAgentRow.tsx`, 54 lines) renders "owner via
  token name", the operation label from `S.review.tokenOperations` (one key, `review.keep`, `web/src/i18n/en.ts:2140-2142`,
  `zh.ts:1874-1876`) and the outcome chip from `S.review.tokenOutcome` (`applied`, `held`, `proposed`), with no
  control; `Review.tsx:2272-2285` picks it by `d.source === "token"`. `KbSettings.tsx:658-684` uses the same three
  helpers for Activity rows that have `token_name`.
- **Source**: lines above
- **Confidence**: high
- **Why it matters here**: an operation without an entry in `tokenOperations` is shown raw; outcome and status
  labels for `approved`, `declined`, `superseded`, `undone` do not exist; Activity is Admin-only and session-only.

## Finding: There is no rate limiter of any kind on the server

- **Claim**: No per-token, per-base, per-IP or global request limiter exists. A search of `crates/` for
  `rate.limit`, `429`, `TOO_MANY`, `tower_governor` finds only `utopia-llm`'s handling of an upstream model
  provider's 429. `AppError` has no variant that renders 429: the variants are `NotFound`, `Unauthorized`,
  `Forbidden`, `CodedForbidden`, `Conflict`, `CodedConflict`, `Validation`, `Invalid`, `Db`, `Other`
  (`crates/utopia-core/src/error.rs`), mapped to 404, 401, 403, 403, 409, 409, 422, 422, 500, 500
  (`crates/utopia-server/src/error.rs:22-58`).
- **Source**: grep over `crates/` (command in this session); the two error files
- **Confidence**: high
- **Why it matters here**: `agent_rate_limited` needs a new error variant or status mapping, and the MCP refusal
  path (`review_tools::refusal`, `review_tools.rs:254-281`) derives its code from the REST response body, so a new
  coded status reaches MCP only through `ApiErr`.

## Finding: Token authentication and revocation are checked once per request

- **Claim**: `tokens::authenticate` (`crates/utopia-store/src/tokens.rs:155-178`) is one
  `UPDATE personal_tokens SET last_used_at = now() WHERE token_hash = $1 AND revoked_at IS NULL AND (expires_at
  IS NULL OR expires_at > now()) RETURNING …`; a revoked or expired token is `Unauthorized`. It runs in the
  `SessionOrToken` extractor (`auth.rs:197-217`) and at the top of every MCP POST (`api/mcp.rs:84-112`). Nothing
  re-checks the token during a handler. `tokens::revoke` stamps `revoked_at` and keeps the row (132-145).
  `Authenticated` is `{user_id, token_id, scope, kb_ids}` with `covers` and `can_write` (27-51).
  `keep_duplicate_pair` reads the token's name by id inside its transaction without checking `revoked_at`
  (`token_agent_actions.rs:110-119`).
- **Source**: lines above
- **Confidence**: high
- **Why it matters here**: a request already past the extractor finishes after revocation; a 50-id batch would run
  to its end. The roadmap entry records this as an accepted risk.

## Finding: MCP Review tools are defined and dispatched in one file, and a refused call is an audited tool error

- **Claim**: `crates/utopia-server/src/api/review_tools.rs` (374 lines): names `review_list`, `review_item`,
  `review_keep` (28-30), `serves` (33-35), `definitions` (46-167), `annotations` (170-176), `listed(can_write)`
  (181-202, hides `review_keep` unless `can_write`), `refusal` (254-281, renders the `AppError` through `ApiErr`
  and copies `code` and `error` into `structuredContent`, adding `not_found` for a 404), `call` (288-374).
  `api/mcp.rs` computes `can_write` as write scope and base role at least Editor (107-110), appends
  `review_tools::listed` in `tools/list` (197-204), and in `tools/call` routes names for which `serves` is true to
  `review_tools::call` without consulting `is_exposed`, then writes an `mcp.tool_called` audit row
  `{tool: name}` for every call, refused or not (205-233). An MCP call from a token whose `kb_ids` exclude the base
  is refused with HTTP 403 before any tool runs (`mcp.rs:96-98`); the token API answers 404. Review tools are not
  in `chat::tools_schema` or `tools::dispatch`.
- **Source**: lines above
- **Confidence**: high
- **Why it matters here**: a new write tool needs an entry in `serves`, `definitions`, `annotations`, the
  `can_write` filter in `listed` (which names `KEEP` literally, line 189) and `call`; the audit row for the tool
  call carries no `token_name`.

## Finding: Existing tests that assert a refusal or an absence M003 is meant to lift

- **Claim**: The following assertions fail when M003's operations open. All are static readings.
  1. `crates/utopia-server/src/api/review_token_api_tests.rs`, test
     `a_refused_token_merge_revert_or_batch_leaves_the_graph_equal` (line 295): token `merge` on
     `/review/{pair}` → 403 at 351 and `token_operation_not_open` at 352; `/review/batch` with `merge` (356-357)
     and with `keep` (360-361), `/merges/{merge_id}/revert` (363) and `/entities/merge` (365-366) → 401 at 372;
     graph rows equal before and after (377-387); ledger empty (388-391). The file header states the same at line 11.
  2. `crates/utopia-server/src/api/review_token_reads_tests.rs`, test
     `a_token_cannot_nod_and_no_review_write_but_keep_answers_a_token` (line 1049): `/review/batch` with `keep`
     (1097-1100) and `/merges/{uuid}/revert` (1138-1141) → 401 at 1146. The same loop also asserts 401 for routes
     M003 does not open (`/review/agent/{id}`, violations, defects, errata, alignment rules, mappings, conflicts,
     fact confirm and reject, 1101-1137) and for the nod (1081-1094), which must keep holding.
  3. `crates/utopia-server/src/api/review_mcp_reads_tests.rs`, test
     `the_mcp_endpoint_offers_no_review_tool_beyond_list_item_and_keep` (line 1039): the review tools listed are
     exactly `[review_item, review_list]` for read tokens and `[review_item, review_keep, review_list]` for a write
     token of an editor (1048-1072); every review tool other than `review_keep` has annotations
     `{readOnlyHint: true}` (1073-1082); `tools/call` of `review_decide`, `review_confirm`, `review_merge`,
     `review_pending` answers JSON-RPC `-32601` (1149-1168).
  4. `crates/utopia-server/src/api/review_token_api_tests.rs`, test `a_keep_over_the_api_is_refused_with_its_code`
     (line 464): a read-scope token sending `merge` gets `token_read_only` (500-512). This one keeps holding if the
     scope check stays first.
  5. `web/src/pages/TokenAgentRow.test.ts`, case "has no Merge, Keep or Revert button, and no button at all"
     (lines 46-53): the rendered token row contains no `button|input|select|textarea` element.
  6. Documentation that states the refusals as current behaviour and is compiled into the server through
     `include_str!` (`crates/utopia-server/src/docs_corpus.rs:16`): `web/src/docs/mcp.md:3, 54, 244, 290, 296`;
     and `docs/decisions/0076-…md:3` (status line), `:126` (code table) and `:177-199` (decision 12 and its revisions).
  No MCP test was found that calls a merge, revert or batch tool expecting a tool-error code; no test asserts 422
  for a token on these routes. `crates/utopia-server/src/api/mcp_tests.rs` asserts on individual tools in
  `tools/list` (lines 830, 1298, 2178), not on the full list.
- **Source**: the files and lines named; grep for `merge|revert|batch|not_open|UNAUTHORIZED` over the server test files
- **Confidence**: high for the listed assertions; medium that the list is complete (the 3,848-line `mcp_tests.rs`
  and non-review test files were searched by pattern, not read in full)
- **Why it matters here**: M002's lesson; a plan that opens these routes without owning these tests will meet red
  tests outside its declared files.

## Finding: Tests and fixtures the next plan inherits

- **Claim**: The fixture `Fx` (`review_session_unchanged_tests.rs:17-221`) builds an organization, two bases
  (`kb`, `other_kb`), an editor and a viewer with sessions, two entity types, and helpers `entity`, `pair_of`,
  `pair`, `conflicting_pair`, `token(user, name, scope, kb_ids)`, `call(bearer, method, path, body)`, `review_row`
  (returns `(status, decided_by, decided_token)`), `audits`, `cleanup`; `Fx::new(governance: bool)`.
  `seed_every_section` (`review_totals_tests.rs:56-87`) runs `scripts/seed-review-sections.sql` (362 lines; reads
  the base from the session setting `seed.kb`; inserts only, `ON CONFLICT DO NOTHING`) and then makes one token
  keep through `keep_duplicate_pair`. The script's header states what it seeds, including "merges 2 · agent 2
  governor rows (one open proposal)" (lines 20-28). Session behaviour is pinned by
  `a_session_editor_gets_todays_duplicates_overview_keep_merge_and_batch_responses`
  (`review_session_unchanged_tests.rs:233`): merge answers `{ok: true}` with one `entity_merges` row and one
  `review.merge` audit row (405-421), batch answers `{decided, outcomes:[{id, error}]}` (424-440). Store-level
  merge and revert behaviour is covered by tests under `crates/utopia-store/tests/store/` (for example
  `an_automatic_merge_is_gated_by_what_it_can_undo.rs`, `a_timeline_is_recomputed_from_the_rows_it_has.rs`,
  `a_governor_reads_the_ledger.rs`) and `crates/utopia-store/tests/a_token_keep_is_never_human_precedent.rs`
  (table test at 983-1043, schema check at 360-361).
- **Source**: lines above
- **Confidence**: high
- **Why it matters here**: several review test files share `Fx` and `seed_every_section`; tests that count rows of
  a seeded section (`the_seed_puts_items_in_every_one_of_the_thirteen_sections`, `review_totals_tests.rs:664`)
  constrain any change to the seed.

## Finding: The web Review tab is one 2,347-line component with no component-level test of a section

- **Claim**: `web/src/pages/Review.tsx` defines the row components in-file — `DuplicateCard` 143-249 (status,
  type-conflict chip, the governor's `proposal` chip at 189-196, checkbox, rationale input, Merge and Keep),
  `MergeRow` 484-522, `AgentRow` 556-663 (answers a governor row: Keep/Merge on `proposed`, Revert on an applied
  merge, Merge on an applied keep) — and `Review()` from 1379, holding all queries and mutations: `review`
  1411-1424 (`api.review(kb, queue, limit, offset, types)`), `decide` 1446-1457, `agentAnswer` 1459-1471, `batch`
  1473-1494, `revert` 1591-1594, shared `invalidate` 1439-1444. Page sizes: duplicates 6, merges 10, agent 20
  (57-60, 1356-1369). The batch bar is at 1935-1978, the Duplicates list at 1985-2007, Agent History at
  2270-2287, Merges at 2289-2305. `decide` and `revert` have no `onError` handler. API client functions:
  `review` `api.ts:2604-2621`, `reviewBatch` 2625-2632, `decideReview` 2664-2668, `mergeEntities` 2746,
  `revertMerge` 2816, `agentAnswer` 2824; types `ReviewItem` 687-696, `AgentDecision` 699-724, `TokenAgentAction`
  728-745, `AgentQueueRow` 748, `MergeLog` 1079. Vitest runs `src/**/*.test.ts` in the `node` environment with no
  DOM (`web/vite.config.ts:17-21`); a component is tested by `renderToStaticMarkup(createElement(…))` and string
  assertions (`TokenAgentRow.test.ts`, `reviewDuplicatesPagination.test.ts`, which imports the exported
  `DuplicateReviewPager` from `Review.tsx`). Review-related test files: `TokenAgentRow.test.ts`,
  `reviewDuplicatesPagination.test.ts`, `reviewErrors.test.ts`, `KbActivity.test.ts`, `alignmentPhrase.test.ts`.
  `DuplicateCard`, `MergeRow`, `AgentRow` and `Review` are not exported and have no test. `web/tests/*.test.mjs`
  are node-test files that need Playwright Chromium and are not run by `pnpm test` or CI. `pnpm build` runs
  `scripts/style-guard.mjs`, then `tsc --noEmit`, then `vite build` (`web/package.json`).
- **Source**: lines above
- **Confidence**: high
- **Why it matters here**: nothing in the web test setup clicks a button or observes a mutation; a criterion
  "a signed-in editor approves it in the Review tab" has no existing automated form in this repository, and
  `TokenAgentRow` was split into its own file to be testable by static render.

## Finding: Schema version constant equals the migration count

- **Claim**: `CURRENT_SCHEMA_VERSION: u32 = 114` (`crates/utopia-cli/src/main.rs:85`), with a comment saying it is
  the number of migration files, not the highest number, guarded by a test named
  `schema_version_policy_compares_against_current`. `migrations/` holds 114 files; the highest number is 0128.
  Numbers 0081-0089, 0107, 0108, 0112, 0124 and 0127 are absent. CI fails on a duplicated number prefix
  (`.github/workflows/checks.yml:138-145`). `scripts/check-upstream-numbers.sh` exists for the upstream check
  ADR 0076 describes (`docs/decisions/0076-…md:289-295`).
- **Source**: `crates/utopia-cli/src/main.rs:80-85`; `ls migrations | wc -l` → 114
- **Confidence**: high
- **Why it matters here**: M001's patch wave (commit `aab411c`, T007) existed because a new migration was added
  without bumping this constant; each new migration in M003 moves it by one.

## Finding: How the project is built and verified, and what a green local run means

- **Claim**: CI (`.github/workflows/checks.yml`) runs `cargo fmt --all --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` (40-43); `cargo run -p utopia-store --example migrate`
  then `cargo test --workspace` with `UTOPIA_DATABASE_URL=postgres://utopia:utopia@localhost:5432/utopia`,
  `UTOPIA_TEST_REQUIRE_DB=1` and `UTOPIA_TEST_REQUIRE_PDFTOTEXT=1` (79-91); `cargo build --workspace` (102-103);
  `sqlx migrate run` twice on a fresh database (153-157); and for web `pnpm install --frozen-lockfile`,
  `pnpm test`, `node --test ../scripts/bench/lib.test.mjs`, `pnpm build` on Node 20 (173-189). Without
  `UTOPIA_DATABASE_URL` every database-backed test returns early as passed (`test_db.rs:17-33`,
  `CONTRIBUTING.md:68-76`). The server build embeds `web/src/docs/mcp.md` (`docs_corpus.rs:16`). On this host the
  compose database container `utopia-db-1` is up on `127.0.0.1:1517`.
- **Source**: lines above; `docker ps` in this session
- **Confidence**: high for the commands; `unverifiable` here for their results at this revision (none was run)
- **Why it matters here**: the program's settled Verify rule (a per-task database on the compose instance,
  `UTOPIA_TEST_REQUIRE_DB=1`) matches CI's shape; this inspection adds no execution evidence to it.

## Finding: Commits in recent history carry no DCO sign-off

- **Claim**: None of the last twelve commits on this branch contains a `Signed-off-by` line.
- **Source**: `git log -12 | grep -n "^commit\|Signed-off-by\|Co-Authored-By"` → twelve `commit` lines, no trailer line
- **Confidence**: high for those twelve commits
- **Why it matters here**: `CONTRIBUTING.md:130-143` and the charter's constraint (driver D005) name DCO sign-off as
  binding for a later upstream proposal.

## Apparent intent

- The code is being opened to token agents one ruled row at a time, each row through the single policy function,
  with the session path left byte-for-byte alone and pinned by a test written before the change — based on
  `review_ops.rs:1-13`, `execution_gate.rs:121-123, 177-196` (the full table present, one caller),
  `review_session_unchanged_tests.rs:1-9`.
- The ledger schema is meant to grow by migration per milestone rather than be laid out ahead — based on
  `migrations/0128_…sql:12-13` and ADR 0076 decision 7 (`docs/decisions/0076-…md:140-147`).
- Token writes are meant to be atomic with their attribution, unlike the session writes they sit beside — based on
  `token_agent_actions.rs:10-13, 163-176` against `review_routes.rs:393-409`.

## Open questions for define

- Criteria 5 and 7 say the graph "returns to its state before the merge". `revert_merge` leaves the decided review
  as `merged`, leaves reviews it closed or redirected as they are, keeps a type the target took from the source,
  and changes `updated_at`. Which of these count as "state" for the comparison, and is the remainder accepted as
  it stands or in scope to change?
- A token merge whose post-commit steps fail (`reconcile_moved_facts`, `refresh_disambiguators`) returns an error
  after the entities are merged. Is that item `applied` with a ledger row, or a failed item?
- A token merge needs a recorded link from merge to token for "the merge was made by a token agent". Is the ledger
  the only record, or should the Merges section also say "owner via token name" for such a merge (it would
  otherwise read as that person's merge)?
- A person's revert of a token merge becomes a revert precedent for both names and raises the governor's bar for
  that type pair. Is that the intended reading of "a token merge does not lower the governor's bar", or should
  token-made merges be excluded from those two reads as well?
- For a manual merge by a token there is no review row: what is the proposal's target, and what does "a pair a
  person kept" mean for two entity ids that may have several earlier review rows?
- Should the pair's own pending review be closed when a token manual merge applies? The session manual merge
  leaves it pending today.
- The existing refusal tests (finding above) include routes M003 does not open. Is editing those shared test
  functions in place acceptable, or must the still-valid refusals move to a new test first?
- Activity is Admin-only and session-only. Criterion 10 says every merge, revert, batch member and proposal
  "appears in Activity": is an Admin session the accepted reader for that check?
- Recent commits carry no `Signed-off-by`. Is sign-off required on M003's commits, or deferred to an upstream
  submission?

## Blocked areas

- Test execution — `cargo test`, `cargo clippy`, `pnpm test` and `pnpm build` were not run: the sidecar has no build
  cache or `node_modules` and may not retain build output. All statements about tests are static.
- `crates/utopia-server/src/api/mcp_tests.rs` (3,848 lines), `crates/utopia-store/src/temporal.rs` and
  `crates/utopia-store/src/review_summary.rs` were searched by pattern and read in part, not in full.
- Two `git` invocations (a custom `--format` and a single-commit `git log`) were refused by the repository's
  pre-tool guard; commit bodies were read only through plain `git log -N`.
