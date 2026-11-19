# Intent — utopia M002 token-review-reads

<!-- Written by /gsd-path-define in milestone + brownfield mode. Derived from
     the approved ROADMAP.md entry M002 and CHARTER.md; no re-interview.
     Constraints and vetoes here override everything downstream. -->

Lane: milestone
<!-- Derived from the approved roadmap entry M002 (owner approval of the roadmap, driver D025). The entry lists no open questions, so research and decide do not run for this milestone. -->

Review panel: off
<!-- Copied from CHARTER.md (Delegated, driver D008). -->

Finding skeptics: off

Surfaces: token-authenticated HTTP API, MCP endpoint

## Summary

After M001 an agent holding a personal token can list only the Duplicates section and the Overview summary of the Review tab; the other eleven sections and their evidence still answer a token with a refusal or 401. This milestone opens the whole read side: a personal token lists every one of the thirteen Review sections and reads any listed item's evidence over the token API and MCP, seeing the same items a signed-in editor sees. Every list response gains the true total of its section, so an agent can page to the last item. No Review write is added. Source: ROADMAP.md entry M002; program spec `C:\Users\Owner\.gsd-path-driver\projects\utopia-ef4aa95f\SPEC.md`.

Lines marked `Delegated (driver D0NN)` were answered for the owner by the gsd-path driver under the owner's consent of 2026-11-19 ("you have my consent to be the human") and can be overruled; the log is `C:\Users\Owner\.gsd-path-driver\projects\utopia-ef4aa95f\DECISIONS.md`.

## Problem

The spec's goal is that an agent "can list each queue, read the evidence" in every section: "Overview, Awaiting your nod, Duplicates, Conflicts, Unconfirmed, Low confidence, Axioms, Ontology, Alignment, Errata, Agent History, Decisions and Merges" (spec, Goal). At `904b2a2` the token list path serves only `duplicates` and refuses every other section with `unknown_queue`; the Decisions history and both proof reads answer a token with 401; list responses carry no total, and for Alignment, Agent History and Low confidence the count named after the section is not the number of listed rows. An agent therefore cannot find the problems it is meant to act on in M003 and M004, and cannot page reliably through the sections it can see.

## Users

- An agent (MCP client or script) holding a personal token of a base member, read or write scope, owner at least Viewer: lists all thirteen sections and reads item evidence.
- A signed-in person in the Review tab: sees every section as before; the only change in what the web app receives is an added total on list responses.
- Technical users; today they read eleven of the thirteen sections in the browser only.

## Success criteria

1. Over the token API, a read-scope token of a base Viewer lists each of the thirteen sections and receives the same items a signed-in editor sees in that section of the Review tab, on a base seeded with at least one item per section.
2. Over MCP, `review_list` returns the same items as the token API for each of the thirteen sections with the same token.
3. For one listed item of each section, the token API and MCP `review_item` return the sentences, facts or entities the Review tab shows for it, including the proof chain of an Axioms item.
4. A token client that pages the Alignment section by the returned total reaches the last implication-rule item.
5. A token whose `kb_ids` exclude the base receives 404 on every Review read over the token API and cannot list or read it over MCP.
6. Agent History read by a token shows the M001 keep as a ledger row naming the owner and the token, beside governor rows.
7. A signed-in person's Review tab lists every section as before.

## Edge coverage

<!-- Rulings marked Delegated were given for the owner by the driver. -->

| Edge | Criterion | Category | Disposition | Detail |
|------|-----------|----------|-------------|--------|
| E1 | SC1 | empty | held-out | Delegated (driver D079): for a section with no items a token gets an empty list and total 0, the same as a signed-in editor, not an error |
| E2 | SC1 | ordering | held-out | Delegated (driver D079): a token receives a section's items in the order the session list returns for the same request |
| E3 | SC1 | boundary | held-out | Delegated (driver D079): the list queues keep the existing clamp — a page size below 1 is served as 1 and above 200 as 200; an offset at or past the total returns no items and the true total |
| E4 | SC1 | boundary | held-out | Delegated (driver D075, D079): a section name outside the thirteen, including `mappings`, is refused with the existing `unknown_queue` coded error and returns no items |
| E5 | SC1 | precision | held-out | Delegated (driver D069, D070): `total` is the number of rows the section's list returns over all pages. For Low confidence it counts the listed rows, derived facts included; for Agent History it counts rows of both sources. The existing `counts` keys keep today's values |
| E6 | SC2 | adjacency | held-out | Delegated (driver D071): over MCP, `review_list` for section `decisions` takes the Decisions route's own `page` and `per` and returns its `{events, total}`; the other list sections take `limit` and `offset`; `overview` takes neither |
| E7 | SC3 | empty | held-out | Delegated (driver D073): `review_item` for an id that is not listed in the named section of this base — unknown, belonging to another base, or no longer waiting — answers not found with no evidence, the same for all three |
| E8 | SC3 | adjacency | held-out | Delegated (driver D072): `review_item` returns the list item's own fields for every section; an Axioms item of kind `derived_contradiction` also carries its proof steps; an Axioms item of another kind carries a null proof and is not an error; an Ontology item returns its list fields |
| E9 | SC3 | encoding | held-out | Delegated (driver D073): `review_item` takes a `section` and an `id`; `section` omitted means `duplicates` as in M001; an Alignment kind-word item is identified by its word, compared exactly as stored; an Agent History row is identified by its row id whichever source it has |
| E10 | SC4 | boundary | criterion SC4 | The total includes implication rules, so the last rule is reached at offset total − 1 |
| E11 | SC4 | ordering | held-out | Delegated (driver D080): the Alignment list gains an id tie-break in its order, so a client paging by `limit` and `offset` over an unchanged queue visits each waiting item exactly once |
| E12 | SC5 | boundary | held-out | Delegated (driver D074, D081): a dead or revoked token gets 401 on the token API and on the MCP endpoint; a token whose `kb_ids` exclude the base gets 404 on the token API and the MCP endpoint's existing HTTP 403 before any tool runs, with no item, count or evidence in the answer |
| E13 | SC6 | empty | held-out | Delegated (driver D082): on a base with no token action a token reads the governor's rows only, exactly as the session list; the ledger row's token name is the snapshot stored with the action |
| E14 | SC7 | idempotency | held-out | Delegated (driver D083): the existing session tests for the Review list, summary and history routes pass without modification, and a session list response differs from today's only by the added `total` key |

## Prohibitions

| Prohibition | Criterion | Must not | Disposition | Detail |
|-------------|-----------|----------|-------------|--------|
| N1 | SC1 | return Review items, totals or evidence over the token API to a token whose `kb_ids` exclude the base | criterion SC5 | Delegated (driver D084): 404, the same answer as for a base that does not exist — charter veto 3 |
| N2 | SC2 | return Review items, totals or evidence over MCP to a token whose `kb_ids` exclude the base | criterion SC5 | Delegated (driver D084) |
| N3 | SC3 | return through `review_item` or an opened proof read anything a signed-in Viewer of that base could not read through the existing routes | judgment | Delegated (driver D084): the reviewer checks that every item and proof read passes the same reader check as the list (owner at least Viewer, base inside `kb_ids`) and returns only fields the session routes already return for that item |
| N4 | SC1 | make any Review write other than the M001 keep reachable by a token | judgment | Delegated (driver D084): the reviewer checks that this milestone adds no token branch to a write handler and no MCP write tool, and that reading "Awaiting your nod" gives a token no way to nod — spec, Hard constraint |
| N5 | SC4 | | dismissed | Delegated (driver D084): a total is the count of rows the same caller may already list; giving it to a caller who may not list is covered by N1 and N2 |
| N6 | SC5 | tell a token whose `kb_ids` exclude the base, over the token API, whether that base exists | criterion SC5 | Delegated (driver D084): 404, the same answer as for a base that does not exist |
| N7 | SC6 | show a token-agent row as a person's own decision or as a governor row | criterion SC6 | Delegated (driver D084): the ledger row names the owner and the token, beside governor rows |
| N8 | SC7 | change what a signed-in person sees or receives in the Review tab other than the added total | criterion SC7 | Delegated (driver D084): ROADMAP M002 Scope out |

## Scope: in

- Token access, at owner role Viewer and through the handlers the Review tab already uses, to the eleven queue sections (Awaiting your nod `pending`, Duplicates `duplicates`, Conflicts `conflicts`, Unconfirmed `unconfirmed`, Low confidence `lowconf`, Axioms `violations`, Ontology `defects`, Alignment `alignment`, Errata `errata`, Agent History `agent`, Merges `merges`), the Overview summary and the Decisions history. No new list route per section.
- Item evidence for every section: the list item's own fields plus the per-item reads, with the violation proof and derived-fact proof reads opened to tokens at Viewer beside the already open fact evidence and entity reads.
- Every list response carries the true total of the listed section as a new `total` field, for session and token callers alike, so paging reaches the last item (the Alignment badge count omits implication rules). Existing `counts` keys keep their values (Delegated, driver D070).
- An id tie-break in the Alignment list order (Delegated, driver D080).
- Agent History read by a token returns the governor's rows and the token-agent ledger rows with their source field.
- MCP `review_list` covers all thirteen sections and `review_item` returns evidence for every item kind, both through the shared operation layer (`review_ops`).
- Refusals: 404 when the token's `kb_ids` exclude the base, 401 for a dead token, the existing coded error for an unknown section.
- Documentation of the full read surface in the API and MCP documentation (`web/src/docs/mcp.md`); ADR 0076 amended where a read shape differs from what it records, and corrected per the doc rulings below (queue 35, 36, 37) when the plan absorbs them.

## Scope: out (vetoes)

- Every Review write beyond the keep shipped in M001: M003 and M004 — charter veto on straight-through agent writes: "Without it the factory is likely to build straight-through agent writes and remove the safeguard the product was designed around." (spec, Hard constraint)
- Carrying an open token-agent proposal on a queue item: arrives with the proposal store in M003 and M004.
- A fix for an empty Low confidence section on a base where every fact is at full confidence: the charter does not scope it. The Low confidence list itself does not change (Delegated, driver D069).
- Widening the Decisions section beyond its four action families, or opening the Activity view to tokens or to roles below Admin.
- Any change to a signed-in person's Review tab responses other than the added section total (do not break the existing session Review tab, its badges or its paging).
- The `mappings` list queue: not a Review section; it stays refused for a token (Delegated, driver D075).
- A change to how the MCP endpoint answers a token whose `kb_ids` exclude the base (HTTP 403 for every tool): unchanged (Delegated, driver D074).
- The nod gate on `remember` and every `pending_facts` decision: unchanged, never finalized by a token (charter veto).
- Widening a token beyond its owner or its scope (charter veto 3).
- Anything sent to the upstream repository or the owner's deployed instance.

## Constraints

- Stack is the existing one: Rust workspace (axum 0.8, sqlx, Postgres 16 with pgvector), React 19 / Vite web app; no new framework or service (CHARTER.md Constraints).
- Program synthesis decisions are settled (`.project/SYNTHESIS.md`): handlers change extractor in place, one shared operation function per Review operation for REST and MCP, the session branch of every handler keeps today's code, attribution is owner plus token.
- The owner's authority ruling binds: "Option 1: the recommended per-operation table" (owner, 2026-11-18, SYNTHESIS.md User rulings). This milestone makes no row of it reachable beyond M001's keep.
- Delegated (driver D005): fork first, upstream later. Work lands on `evolv3ai/utopia` `dev` in `direct` mode. Upstream conventions bind: an ADR before a public-API or data-model change (ADR 0076 is amended, no new record unless a new number is needed, in which case it is checked with `scripts/check-upstream-numbers.sh`), DCO `Signed-off-by`, Chinese explanatory comments, sentence-named tests and migrations, one behaviour-sentence commit subject per change. Nothing is sent to `deeplethe/utopia`.
- Delegated (driver D006, D076): store and server behaviour is verified against a migrated Postgres 16 + pgvector with `UTOPIA_TEST_REQUIRE_DB=1` — the compose `db` service on `127.0.0.1:1517`, one database per task Verify, the `utopia-store` tests on their own freshly migrated database. Tests seed one item per section through store functions or direct inserts, not through model jobs. A plain `cargo test` without a database is not evidence.
- Delegated (driver D007): this machine's toolchain (Windows, Node 26, pnpm 10.14) is acceptable Verify evidence. Host rules from `.project/LESSONS.md` apply: a Verify builds into its own empty `CARGO_TARGET_DIR` outside the worktree, and PostgreSQL 16 client tools are on PATH.
- Do not run the full-repository suite as a task Verify for a narrow change; each task Verify names its own tests and its own database (SYNTHESIS.md, Verification stack).
- If this milestone adds a migration, the same task bumps `CURRENT_SCHEMA_VERSION` in `crates/utopia-cli/src/main.rs` (`.project/LESSONS.md`).
- No budget, deadline, paid service or deployment is in scope. The owner's vibecoding.bot instance is not touched.

## Current state (brownfield only)

- **What exists**: The M001 token path at `904b2a2`: `crates/utopia-server/src/api/review_ops.rs` (shared operations `list`, `item`, `summary`, `keep`), `review_tools.rs` (MCP `review_list`, `review_item`, `review_keep`) and token branches in three handlers of `review_routes.rs`. `review_ops::list` serves only `duplicates`; the eleven-queue `match` lives in the session branch of `review_routes::list`. Thirteen sections map to three routes: eleven `queue` keys on the list route, `overview` on `summary`, `decisions` on `history` (which pages by `page`/`per` and already returns a total). `history`, `derived_proof` and `blocked_proof` take `AuthUser` and answer a token with 401; fact evidence and entity reads already accept tokens. List responses are `{counts, queue, items}` with no total; `counts.alignment` omits proposed implication rules, the pager uses `counts.agent_rows` for Agent History, and the Low confidence count excludes derived facts that the list includes. Agent History already merges governor rows and ledger rows (`source: "token"`) in one paged sequence. The Review tab renders list-item fields only; proofs are fetched on the Graph page, and `blocked_proof` returns steps only for `derived_contradiction` violations.
- **Must not break**: the signed-in Review tab, its badges (`counts`) and its paging; the M001 token reads, keep, refusal codes and attribution; the MCP endpoint's existing tools and its 403 for an out-of-scope token; the nod gate on `remember`.
- **Doc-vs-code rulings**: recorded in `research/DOCS-AUDIT.md` `## User rulings`. New in this milestone — queue 36 (ADR 0076 decision 6, 404 versus the MCP endpoint's 403): fix-doc (Delegated, driver D074); queue 37 (ADR 0076 decision 12, `token_operation_not_open` versus 401 on manual merge, revert and batch): fix-doc (Delegated, driver D077); queue 38 (upstream claim on 0127): accept-drift (Delegated, driver D078). Queue 35 (ADR 0076 status line, kind-word decision already open) is classed fix-doc by the audit and needs no ruling. Earlier rulings (queue 1, 4, 6, 11, 16, 17, 20, 28: fix-doc; 13, 14: accept-drift) stand, none planned yet.
- **Ground truth**: `.project/research/evidence-codebase.md`, `.project/research/DOCS-AUDIT.md`

## Risks

- "Same items a signed-in editor sees" must be shown per section; some sections need seeded data that is expensive to build (errata, alignment rules, derived contradictions).
- The Low confidence section may list nothing on the owner's base; the criterion is met on seeded data only.
- Published images predate token reads, so the read surface can be demonstrated only on a build of this checkout.
- Adding a total to list responses touches the shape the web app reads; a mistake breaks the session tab.
- Two M001 tests and the MCP tool descriptions assert today's `unknown_queue` refusal for the other sections and must change with the behaviour, without weakening the refusal for a truly unknown section.
- Totals are computed by a separate query from each list; a list join that drops a row its count includes would make a total wrong (not checked against data).

## Open questions

- None

## Corrections

<!-- Verbatim user corrections from playback and later phases. Append-only,
     numbered from C1. -->
- C1 2026-11-19: Delegated (driver D087): "E14 is narrowed: the one assertion in `crates/utopia-server/src/api/review_session_unchanged_tests.rs` that lists the keys of a session list body gains `total`; nothing else in that file changes and every other existing session test file passes without modification."
