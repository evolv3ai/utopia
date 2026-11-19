# Intent — utopia M001 token-review-skeleton

<!-- Written by /gsd-path-define in milestone + brownfield mode. Derived from
     the approved ROADMAP.md entry M001 and CHARTER.md; no re-interview.
     Constraints and vetoes here override everything downstream. -->

Lane: milestone
<!-- Derived from the approved roadmap entry M001 (owner approval, driver D025). The entry lists no open questions, so research and decide do not run for this milestone. -->

Review panel: off
<!-- Copied from CHARTER.md (Delegated, driver D008). -->

Finding skeptics: off

Surfaces: token-authenticated HTTP API, MCP endpoint, Review tab in the web app (Agent History section), Activity view in the base settings of the web app

## Summary

The Review tab is the only way to maintain a Utopia knowledge graph and it accepts only a browser session, so an agent that can read the graph cannot act on what it finds. This milestone is the thinnest end-to-end slice of the program: an agent holding a personal token lists Duplicates and Overview and finalizes one keep through the server-side authority policy, over both the token API and MCP, with the action attributed to "owner via token name" in Agent History and Activity. It also proves the verification ground (compose Postgres 16 + pgvector) and writes ADR 0076 before the first route or schema change. Source: ROADMAP.md entry M001; program spec `C:\Users\Owner\.gsd-path-driver\projects\utopia-ef4aa95f\SPEC.md`.

Lines marked `Delegated (driver D0NN)` were answered for the owner by the gsd-path driver and can be overruled; the log is `C:\Users\Owner\.gsd-path-driver\projects\utopia-ef4aa95f\DECISIONS.md`.

## Problem

Review routes take a session cookie only; a personal access token gets 401 on them, and the MCP server exposes eleven read tools plus `remember` and nothing that touches review. On a large machine-extracted base (the owner's has "Cursor" as eight entities and every vendor's MCP server merged into one node) the clean-up cannot be done one click at a time. Before any broad agent write is opened, the program needs one proven path through the server-side finalize-versus-propose policy, with attribution, so later milestones extend a safe pattern rather than a straight-through write.

## Users

- An agent (MCP client or script) holding a personal token of a base member: reads Duplicates and Overview with any scope at owner role Viewer or above; keeps a duplicate pair with scope `write` and an owner at least Editor.
- A signed-in editor of the base, who sees the agent's keep in the Review tab's Agent History section.
- A signed-in base Admin, who sees it in the Activity view.
- Technical users; today they use the browser Review tab only.

## Success criteria

1. Against the compose Postgres 16 + pgvector instance with `UTOPIA_TEST_REQUIRE_DB=1`, the existing token read and token setup tests pass, and two builds with different migration sets each migrate and test their own database on that instance.
2. Over the token API, a read-scope token of a base Viewer lists Duplicates and Overview and receives the same items and counts a signed-in editor sees in those sections of the Review tab; a token whose `kb_ids` exclude the base receives 404.
3. Over MCP, `review_list` returns the same Duplicates items and Overview summary as the token API for the same token, and `review_item` returns the evidence the Review tab shows on that pair's card.
4. Over the token API and over MCP, a write-scope token owned by an editor keeps one duplicate pair: the pair leaves the Duplicates queue, no fact or entity changes, and one ledger row with outcome `applied` and one audit row with `token_id` and `token_name` exist.
5. The same keep is refused with `token_read_only` for a read-scope token and with `token_owner_below_editor` for a write token whose owner is below Editor, over the token API and as an MCP tool error carrying the same code; a repeat of a decided keep answers `already_decided`; the MCP write tool is not listed for either refused caller.
6. A token call to merge, revert or batch changes nothing in the graph and is refused.
7. In the Review tab, a signed-in editor sees the token keep in the Agent History section as "owner via token name", beside unchanged governor rows.
8. In the Activity view, a signed-in base Admin sees the token keep as "owner via token name" with a readable action label.
9. A database-backed test shows a token keep does not lower the governor's bar, does not settle the pair as decided by people, and starts no govern round.
10. A signed-in person's Review tab requests and responses for Duplicates, Overview, keep, merge and batch are unchanged, and `remember` still lands a pending fact that waits for a nod.
11. ADR 0076 exists in the decision records with the ruled authority table, and its number and migration 0128 do not collide with upstream at landing.

## Edge coverage

<!-- Rulings marked Delegated were given for the owner by the driver; rows citing
     D025 restate text of the roadmap entry the owner approved. -->

| Edge | Criterion | Category | Disposition | Detail |
|------|-----------|----------|-------------|--------|
| E1 | SC1 | idempotency | held-out | Delegated (driver D026): running the migration runner a second time against the same per-run database succeeds and changes no schema — test-only rule about the existing runner |
| E2 | SC1 | concurrency | criterion SC1 | Two builds with different migration sets each use their own database on the one instance |
| E3 | SC2 | empty | held-out | Delegated (driver D027): for a section with no items a token gets the same empty list and zero counts a signed-in editor gets, not an error — special case of SC2 through the same handler |
| E4 | SC2 | ordering | held-out | Delegated (driver D028): a token receives a section's items in the same order the session list returns for the same request — same handler and query, no new ordering |
| E5 | SC2 | boundary | criterion SC2 | An owner exactly at Viewer reads; a token whose `kb_ids` exclude the base gets 404 |
| E6 | SC3 | boundary | held-out | `review_list` applies the existing 1 to 200 page clamp: a page size below 1 is served as 1 and above 200 as 200 (ROADMAP M001 Scope in, owner-approved D025; `review_routes.rs:86`) |
| E7 | SC3 | boundary | held-out | Delegated (driver D029): in this milestone `review_list` for a section other than Duplicates or Overview is refused with the existing `unknown_queue` coded error and returns no items — transitional until M002 serves all thirteen sections |
| E8 | SC3 | empty | held-out | Delegated (driver D030): `review_item` for an id that is not a duplicate pair of this base answers a not-found tool error with no evidence, identical for an unknown id and an id that belongs to another base — existing 404 convention of base-scoped reads |
| E9 | SC4 | idempotency | criterion SC5 | A repeat of a decided keep answers `already_decided` |
| E10 | SC4 | concurrency | held-out | Two token keeps of the same pair at the same time (REST and MCP, or two calls): exactly one is applied, the other answers `already_decided`, and exactly one ledger row with outcome `applied` exists (ROADMAP M001 Scope in: "`already_decided` for a lost race or repeated call", owner-approved D025) |
| E11 | SC4 | concurrency | held-out | A failed `token_name` snapshot or audit write fails the keep: the pair stays in the Duplicates queue and no ledger row with outcome `applied` exists (ROADMAP M001 Scope in: "A failed snapshot or audit write fails the action", owner-approved D025) |
| E12 | SC5 | boundary | criterion SC5 | An owner exactly at Editor keeps (SC4); an owner one role below is refused with `token_owner_below_editor` |
| E13 | SC5 | adjacency | held-out | Delegated (driver D031): a token that is both read-scope and owned by someone below Editor gets `token_read_only`, because scope is checked before role — SYNTHESIS, Token access to Review routes |
| E14 | SC6 | adjacency | criterion SC6 | A token batch is refused whatever its action, including a batch of keeps; merge covers both the queue decision and the manual merge |
| E15 | SC6 | idempotency | held-out | Delegated (driver D032): "changes nothing in the graph" is checked by reading the base's entities, facts, review rows and merges before and after the refused call and finding them equal — test approach only |
| E16 | SC7 | encoding | held-out | Delegated (driver D033): Agent History shows the name the token had when the keep was made (the stored `token_name` snapshot), also after the token is renamed or revoked — display of the snapshot the roadmap already stores |
| E17 | SC7 | ordering | held-out | Delegated (driver D034): ledger rows are merged into the section's existing sort order by their action time; governor rows keep the relative order they have today — a sort key in one list read |
| E18 | SC8 | empty | held-out | Delegated (driver D035): a token row whose call sent no User-Agent still reads "owner via token name" with its action label and shows no client label — rendering of one optional field |
| E19 | SC9 | adjacency | held-out | Delegated (driver D036): on a base holding both a person's keep and a token's keep, the person's keep still counts as human precedent exactly as today; only the token's keep is excluded — test-only pin of the roadmap's precedent isolation |
| E20 | SC10 | idempotency | held-out | Delegated (driver D037): "unchanged" is checked by the existing session tests for the Duplicates list, Overview, keep, merge and batch routes and for `remember` passing without modification — test approach only |
| E21 | SC11 | boundary | criterion SC11 | Delegated (driver D038): if upstream has claimed ADR 0076 or migration 0128 by landing, the next lowest free number above every merged and claimed number is used and SC11 is judged on the number actually landed — SYNTHESIS, Migration and ADR numbering |

## Prohibitions

| Prohibition | Criterion | Must not | Disposition | Detail |
|-------------|-----------|----------|-------------|--------|
| N1 | SC1 | | dismissed | Delegated (driver D042): the verification ground touches only throwaway test databases on this machine and no person or product data |
| N2 | SC2 | return Review items or evidence to a token whose `kb_ids` exclude the base | criterion SC2 | Delegated (driver D040): 404, the same answer as for a base that does not exist — spec, Goal and Facts to check (token scopes); charter veto 3 |
| N3 | SC3 | return Review items, counts or evidence over MCP to a token whose `kb_ids` exclude the base | judgment | Delegated (driver D040): the reviewer checks that `review_list` and `review_item` pass the same reader check as the token API and give such a token no item, count or evidence |
| N4 | SC4 | show or count a token keep as a person's own decision | criterion SC9 | Delegated (driver D039): also shown as "owner via token name" in SC7 and SC8 — spec, Goal: "Every action appears in Activity and Agent History under the token owner and the agent's name" |
| N5 | SC5 | list the keep tool to a caller who is refused the keep | criterion SC5 | "the MCP write tool is not listed for either refused caller" (ROADMAP M001 success criterion 5, owner-approved D025) |
| N6 | SC6 | open any agent write other than keep in this milestone | criterion SC6 | Delegated (driver D041): spec, Hard constraint: "Without it the factory is likely to build straight-through agent writes and remove the safeguard the product was designed around." |
| N7 | SC7 | show a token keep as the governor's row or as a person's own, or alter how governor rows are shown | criterion SC7 | Delegated (driver D039): "owner via token name", beside unchanged governor rows |
| N8 | SC8 | present the declared client (the caller's User-Agent) as a verified identity, or read it for authorization | judgment | The reviewer checks that Activity labels the client as declared wherever it is shown and that no authorization or policy code reads it (ROADMAP M001 Scope in: "the stored User-Agent, marked as declared", owner-approved D025) |
| N9 | SC9 | let a token decision start a govern round or settle a governor proposal | criterion SC9 | ROADMAP M001 Scope in, precedent isolation (owner-approved D025) |
| N10 | SC10 | weaken the nod on `remember` | criterion SC10 | Delegated (driver D043): spec, Goal: "The Charter makes remember wait for a person's nod because a document can talk an agent into recording something; the same risk applies to an agent confirming or merging facts on the strength of text it just read." |
| N11 | SC11 | record an authority table in ADR 0076 that differs from the owner's ruling | criterion SC11 | "with the ruled authority table" (ROADMAP M001 success criterion 11, owner-approved D025; SYNTHESIS.md User rulings) |

## Scope: in

- Decision record ADR 0076, written before the first route or schema change: the opened Review routes and MCP tools, the owner's ruled per-operation authority table with its reasons, the `token_agent_actions` ledger, attribution as owner plus token, the named constants (batch cap 50, 200 finalized writes per rolling hour), and the statement that a merge revert restores graph state only. The low-confidence "ontology adoption has no undo" reason is confirmed or corrected against the store before the record is final. ADR and migration numbers start at 0076 and 0128, re-checked against upstream before each landing; migration SQL is idempotent.
- Verification ground: the existing token read and token setup tests pass against the compose Postgres 16 + pgvector instance with `UTOPIA_TEST_REQUIRE_DB=1` (the program's proof that ADR 0014 decision 7 holds on this build), and per-run database isolation on that one instance is shown to work from this machine's toolchain.
- Read slice: a personal token (read or write scope, owner at least Viewer) lists the Duplicates section and the Overview summary through the same handlers the Review tab uses, and reads one duplicate item's evidence; a token whose `kb_ids` exclude the base gets 404.
- MCP read tools `review_list` (section parameter, existing 1 to 200 page clamp) and `review_item`, listed for any token that reaches the base with an owner at least Viewer; in this milestone they serve Duplicates and Overview.
- The shared operation layer: each Review operation is one server function taking the resolved caller (owner, token, base), called by both the REST token branch and the MCP tool. The session branch of every handler runs today's code unchanged.
- The server-side policy function, living with the execution gate module, holding the compiled-in authority table and returning `applied`, `held` or `proposed`. In this milestone only the keep row is reachable by a token.
- Write slice: keep on one duplicate pair with a write-scope token owned by an editor, over REST and over an MCP write tool that is listed and callable only with scope `write` and an owner at least Editor. Tool annotations are hints only; elicitation is not used.
- Stable refusal codes on that write: `token_read_only`, `token_kb_out_of_scope`, the new `token_owner_below_editor`, and `already_decided` for a lost race or repeated call; a refused MCP tool call carries the same code in its structured error.
- The ledger table `token_agent_actions` and one row per token-agent action; every token Review write also writes an audit row with the owner as actor plus `token_id` and a `token_name` snapshot. A failed snapshot or audit write fails the action.
- Precedent isolation: token decisions are never human precedent. The governor's precedent queries and its settled-by-people check exclude token rows, review rows carry the deciding token, and a token decision starts no govern round and settles no governor proposal.
- Attribution display: Agent History returns governor rows unchanged plus ledger rows told apart by a source field, shown as "owner via token name"; the Activity view gains optional token id, token name and declared client (the stored User-Agent, marked as declared) and renders token rows as "owner via token name" with a readable action label.
- Documentation of the routes and tools opened by this milestone in the product's API and MCP documentation.

## Scope: out (vetoes)

- Token merge, manual merge, revert and batch: closed to tokens until M003, because a merge may not ship before the gate and the proposal store exist — charter veto on straight-through agent writes: "Without it the factory is likely to build straight-through agent writes and remove the safeguard the product was designed around." (spec, Hard constraint)
- Every propose-only operation and the proposal store: M003 and M004.
- Token listing of the other eleven sections and evidence for their items, proof routes, true section totals: M002.
- Per-token rate limit and breaker: M003.
- Any change to what a signed-in person sees or can do in the Review tab beyond the added token rows in Agent History and Activity; the person's ungated merge and 500-id batch stay as they are (do not break the existing session Review tab).
- The nod gate on `remember` and every `pending_facts` decision: unchanged, never finalized by a token — charter veto: "The Charter makes remember wait for a person's nod because a document can talk an agent into recording something; the same risk applies to an agent confirming or merging facts on the strength of text it just read." (spec, Goal)
- Widening a token beyond its owner or its scope: a read-scope token, or a token whose owner is below editor, gains no Review write (charter veto 3).
- A per-base authority setting, a base-level machine token or service account, MCP `clientInfo` as agent name: ruled out by the synthesis.
- Labels for existing machine actors in Activity (governor, adjudicator, errata agent): unchanged.
- Anything sent to the upstream repository or the owner's deployed instance.

## Constraints

- Stack is the existing one: Rust workspace (axum 0.8, sqlx, Postgres 16 with pgvector), React 19 / Vite web app; no new framework or service (CHARTER.md Constraints).
- The owner's authority ruling binds: "Option 1: the recommended per-operation table" (owner, 2026-11-18, SYNTHESIS.md User rulings). In this milestone only its keep row is reachable by a token.
- Program synthesis decisions are settled (`.project/SYNTHESIS.md`): handlers change extractor in place, one shared operation function per Review operation for REST and MCP, the policy function lives with the execution gate module, attribution is owner plus token, the ledger is `token_agent_actions`, token rows are never human precedent, numbers start at ADR 0076 and migration 0128.
- Delegated (driver D005): fork first, upstream later. Work lands on `evolv3ai/utopia` `dev` in `direct` mode. Upstream conventions bind: an ADR before a public-API or data-model change, DCO `Signed-off-by`, Chinese explanatory comments, sentence-named tests and migrations, one behaviour-sentence commit subject per change. Nothing is sent to `deeplethe/utopia`.
- Delegated (driver D006): store and server behaviour is verified against a migrated Postgres 16 + pgvector with `UTOPIA_TEST_REQUIRE_DB=1` — the compose `db` service on `127.0.0.1:1517`, started on the owner's instruction, one database per task Verify. A plain `cargo test` without a database is not evidence.
- Delegated (driver D007): this machine's toolchain (Windows, Node 26, pnpm 10.14) is acceptable Verify evidence.
- Do not run the full-repository suite as a task Verify for a narrow change; each task Verify names its own tests and its own database (SYNTHESIS.md, Verification stack).
- No budget, deadline, paid service or deployment is in scope. The owner's vibecoding.bot instance is not touched.

## Current state (brownfield only)

- **What exists**: A working self-hosted knowledge-base server: a nine-crate Rust workspace (`utopia-server` axum API with an MCP endpoint, `utopia-store` for all Postgres access) and a React 19 web app, with about 1,870 Rust tests and 355 web tests passing locally. Every handler in `crates/utopia-server/src/api/review_routes.rs` takes `AuthUser` (session only) except `decide_alignment_kind_word`, which takes `SessionOrToken`; personal tokens already read entities, ontology, export, graph and search routes (ADR 0014 decision 7). The server crate has no `lib.rs`, so its tests live in `src/` as sibling `*_tests.rs` files, and the backend build embeds `web/src/docs/*.md`.
- **Must not break**: the signed-in Review tab and its routes, including the person's ungated merge and 500-id batch; the nod gate on `remember`; the existing token read routes and their refusal codes; the governor's own rows and its decision route.
- **Doc-vs-code rulings**: recorded at program define in `research/DOCS-AUDIT.md` `## User rulings`, all queued and none planned in this milestone — queue 1, 11: fix-doc (Delegated, driver D009, D012); queue 4: fix-doc (D010); queue 6: fix-doc, no toolchain pin (D011); queue 13, 14: accept-drift (D013, D014); queue 16: fix-doc, no stdout feature (D015); queue 17: fix-doc (D016); queue 20: fix-doc (D017); queue 28: fix-doc (D018).
- **Ground truth**: `.project/research/evidence-codebase.md`, `.project/research/DOCS-AUDIT.md`

## Risks

- Windows-side builds may not reach or migrate the compose database, or per-run databases may not isolate; every later milestone's evidence depends on this.
- The precedent exclusion is medium confidence and was never exercised end to end; if a token row still steers the governor, no agent finalize is safe and M003 changes shape.
- One shared operation function for REST and MCP is medium confidence; if the MCP tool path cannot share it, both surfaces drift and every later milestone doubles.
- Finalized keeps are not rate limited until M003; exposure is bounded because a keep changes only the review row and is excluded from precedent.
- Number collision with upstream (migration 0127 is already claimed by an open upstream pull request); renumbering is expected at any upstream submission.
- Activity is readable only by a base Admin in a session, so criterion 8 cannot be observed by an editor.
- Local toolchain differs from CI (accepted, driver D007).

## Open questions

- None

## Corrections

<!-- Verbatim user corrections from playback and later phases. Append-only,
     numbered from C1. -->
