# Evidence — capability-map

<!-- Written by one researcher role. Consumed by the decider. -->

Dimension: capability-map
Questions assigned: How is "the agent's name" known to the server: the token's name, MCP `clientInfo`, or a declared field? What do Activity and Agent History show today for the governor and errata agents?; Map each of the thirteen Review sections to its existing routes and store functions; name the sections with no dedicated read route.; What do `review/batch` and `review/agent/{decision_id}` do today, and what does a batch of agent actions do to the confused-deputy risk?

Method: static reading of the checkout at `3d307f6` (no project command was run) plus the live MCP specification on modelcontextprotocol.io. All repository paths are relative to `D:\utopia`. All routes are nested under `/api/v1` (`crates/utopia-server/src/api/mod.rs:647`). "Session-only" below means the `AuthUser` extractor: it accepts the `utopia_token` cookie or a `Bearer` value, but decodes either as a session JWT, so a `utp_pat_` personal token gets 401 (`crates/utopia-server/src/auth.rs:156-173`, `:175-187`).

## Finding: Only two of the thirteen sections have their own read route; eleven share `GET /kbs/{id}/review`

- **Claim**: The Review page makes exactly three list requests: `GET /kbs/{id}/review?queue=&limit=&offset=&types=` for eleven sections (pending, duplicates, conflicts, unconfirmed, lowconf, violations, defects, alignment, errata, merges, agent), `GET /kbs/{id}/review/summary` for Overview, and `GET /kbs/{id}/review/history?page=&per=` for Decisions. All three handlers take `AuthUser` and require base role Viewer. No Review list route accepts a personal token.
- **Source**: `web/src/pages/Review.tsx:1326-1338` (`QUEUE_FETCHED`), `:1409-1435` (the three queries); `web/src/api.ts:2572-2586`, `:2786-2787`, `:2800-2802`; `crates/utopia-server/src/api/mod.rs:532-534`; `crates/utopia-server/src/api/review_routes.rs:76-156` (`list`), `:717-729` (`history`), `:732-741` (`summary`)
- **Confidence**: high
- **Why it matters here**: Charter success criterion 1 (each section listable by token, same items as the tab) needs token access to three handlers, not thirteen; the per-section dispatch already exists inside `list` as a `match queue`.

Section-to-capability map. Rail label is from `web/src/i18n/en.ts:2098-2111`; queue key is the `queue` parameter. Every write below requires base role Editor through `require_kb(..., Role::Editor)` and, unless stated, `AuthUser`. "Audit" is the `audit_events.action` written.

| # | Section (rail label) | Queue key | Read route → store function | Item evidence carried in the list item | Write routes (method path → handler → store) | Audit event |
|---|---|---|---|---|---|---|
| 1 | Overview | none | `GET /kbs/{id}/review/summary` → `summary` → `utopia_store::review_summary::summary` (`review_summary.rs:33`) | aggregate counts only | none | none |
| 2 | Awaiting your nod | `pending` | shared `GET /review` → `utopia_store::pending::list` (`pending.rs:219`); also `GET /kbs/{id}/review/pending?chunk_id=` → `pending_for_chunk` → `pending::for_chunk` (`pending.rs:236`), used by the chat card | `quote` (the remembered sentence), `proposed_by_name`, `proposed_token_name` (`web/src/api.ts:932-959`) | `POST /kbs/{id}/review/pending/{pending_id}` body `{action: confirm\|reject}` → `decide_pending` → `pending::confirm` (`pending.rs:295`) / `pending::reject` (`:566`) | `fact.nod_confirmed` / `fact.nod_rejected` (`review_routes.rs:1079-1140`) |
| 3 | Duplicates | `duplicates` (+ `types=any\|same\|conflict`) | shared `GET /review` → `utopia_store::resolution::list_reviews` (`resolution.rs:1881`) | both sides' `top_facts`, `degree`, type, `score`, open agent `proposal` (`web/src/api.ts:662-690`) | `POST /kbs/{id}/review/{review_id}` `{action: merge\|keep, rationale?}` → `decide` → `resolution::decide_review` (`resolution.rs:1991`); `POST /kbs/{id}/review/batch` → `batch` → `resolution::decide_reviews` (`:1909`); `POST /kbs/{id}/entities/merge` `{source,target,rationale?}` → `manual_merge` → `resolution::merge_entities` (`:2144`) | `review.merge` / `review.keep`; `merge.manual` |
| 4 | Conflicts | `conflicts` | shared `GET /review` → `utopia_store::temporal::list_conflicts` (`temporal.rs:1433`) | old and new fact ids, subjects, objects, dates (`web/src/api.ts:962-978`); no sentences | `POST /kbs/{id}/conflicts/{conflict_id}` `{action: close\|retime_new\|keep\|reject_new, close_at?, new_from?, …}` → `resolve_conflict` → `temporal::resolve_review_conflict` (`temporal/conflicts.rs:14`) | `conflict.close_old` / `conflict.retime_new` / `conflict.keep_both` / `conflict.reject_new`, written inside the store transaction (`temporal/conflicts.rs:56-61`, `:184-193`) |
| 5 | Unconfirmed | `unconfirmed` | shared `GET /review` → `utopia_store::graph::stale_facts` (`graph.rs:2111`) | `quote`, `evidence_count` (`web/src/api.ts:918-928`) | `POST /kbs/{id}/facts/{fact_id}/reject` → `reject_fact` → `graph::reject_fact` (`graph.rs:2171`); `POST /kbs/{id}/facts/{fact_id}/close` `{valid_to, valid_to_precision?}` → `close_fact` → `temporal::close_superseded` (`temporal.rs:882`). The tab offers no confirm here (`Review.tsx:2031-2052`) | `fact.reject`; `fact.close` |
| 6 | Low confidence | `lowconf` | shared `GET /review` → `utopia_store::graph::low_confidence_facts` with threshold `LOW_CONFIDENCE_BELOW = 0.75` (`graph.rs:2073`, `review.rs:20`) | `quote`, `evidence_count` | `POST /kbs/{id}/facts/{fact_id}/confirm` → `confirm_fact` → `graph::confirm_fact` (`graph.rs:2148`); `POST …/reject` as above | `fact.confirm`; `fact.reject` |
| 7 | Axioms | `violations` | shared `GET /review` → `utopia_store::reasoning::open_violations` (`reasoning.rs:795`) | the violating facts are in the item; the proof chain is a separate route, `GET /kbs/{id}/violations/{violation_id}/proof` (`AuthUser`, Viewer, `graph_routes.rs:548`) | `POST /kbs/{id}/review/violations/{violation_id}` `{resolution: fact_retracted\|fact_closed\|axiom_relaxed\|accepted, close_at?, fact_id?}` → `decide_violation` → `reasoning::retract_from_violation` (`reasoning.rs:948`) / `temporal::close_superseded` / `reasoning::decide` (`:977`); `POST /kbs/{id}/consistency/check` → `run_consistency_check` → `reasoning::run` (`:556`) + `reasoning::check_ontology` (`:1016`) | `violation.decided` plus `fact.reject` or `fact.close` for the fact touched; `consistency.checked` |
| 8 | Ontology | `defects` | shared `GET /review` → `utopia_store::reasoning::open_defects` (`reasoning.rs:2753`) | defect detail with rule pairs and examples (`web/src/api.ts:900-917`) | `POST /kbs/{id}/review/defects/{defect_id}` `{resolution: fixed\|accepted}` → `decide_defect` → `reasoning::decide_defect` (`:2790`) | `defect.decided` |
| 9 | Alignment | `alignment` | shared `GET /review` → `utopia_store::alignment_queue::list` (`alignment_queue.rs:116`), three item kinds: `phrase`, `kind_word`, `rule`; the page also calls `GET /kbs/{id}/ontology` for the choices (`Review.tsx:1668-1672`), which already takes `SessionOrToken` (`ontology_routes.rs:28-30`) | example statements and the aligner's two votes | `POST /kbs/{id}/review/alignment/phrases/{binding_id}` → `decide_alignment_phrase` → `phrase_bindings::decide_with_delivery` (202 + `job_id`); `POST …/alignment/rules/{rule_id}` `{approve}` → `decide_alignment_rule` → `implication_rules::decide_with_delivery` (202 + `job_id`); `POST …/alignment/kind-words/{kind_word}` `{class?}` → `decide_alignment_kind_word` → `type_bindings::decide_and_apply_human` — **the only Review handler that takes `SessionOrToken`** and calls `require_write` (`review_routes.rs:1370-1426`) | `alignment.phrase_decided`; `alignment.rule_decided`; `alignment.kind_word_decided` (with `detail.token_id` when a token called) |
| 10 | Errata | `errata` | shared `GET /review` → `utopia_store::errata::held` (`errata.rs:744`), status `held` only | document name, `quote`, `reason`, gate `detail`, proposed triple | `POST /kbs/{id}/review/errata/{action_id}` `{approve}` → `decide_errata` → `errata::decide_held` (`errata.rs:780`) | `errata.decided` |
| 11 | Agent History (rail label in this checkout: "Agent") | `agent` | shared `GET /review` → `utopia_store::governance::list` (`governance.rs:875`) over `agent_decisions` | `reason`, `precedents`, `trace`, `question`, `confidence` (`web/src/api.ts:693-716`) | `POST /kbs/{id}/review/agent/{decision_id}` `{action: merge\|keep\|revert, rationale?}` → `agent_answer` (see the dedicated finding) | `review.merge` / `review.keep` / `merge.revert` / `merge.manual`, each with `detail.agent_decision` |
| 12 | Decisions | none | `GET /kbs/{id}/review/history?page=&per=` → `history` → `utopia_store::audit::review_history` (`audit.rs:129`), `per` clamped 1–100 | the audit row's self-contained `detail` snapshot | none | none |
| 13 | Merges | `merges` | shared `GET /review` → `utopia_store::resolution::list_merges` (`resolution.rs:2632`) | source and target names, `merged_by_name`, `reason`, `reverted_at` | `POST /kbs/{id}/merges/{merge_id}/revert` → `revert_merge` → `resolution::revert_merge` (`resolution.rs:2500`) then `governance::after_revert` | `merge.revert` |

Notes on the table:
- `GET /review` also answers `queue=mappings` (`review_routes.rs:118-120`); "Data mapping" is a link out of the page, not one of the thirteen (`Review.tsx:1825-1842`).
- `limit` on `GET /review` defaults to 10 and is clamped to 1–200; an unknown queue returns the coded error `unknown_queue` (`review_routes.rs:24`, `:86`, `:140-148`).
- Every `GET /review` response also carries all counts from `utopia_store::review::counts` (`review.rs:62-111`), so one request lists one section and counts all of them.

## Finding: The Review tab reads item evidence from the list item itself; it calls no per-item evidence route

- **Claim**: `Review.tsx` contains no request other than the list calls and the write mutations; the sentences, facts and entities shown on a card are fields of the list item (`quote`, `evidence_count`, `top_facts`, `precedents`, `trace`, examples). Separate per-item evidence routes exist outside the page: `GET /kbs/{id}/facts/{fact_id}/evidence` and `GET /kbs/{id}/entities/{entity_id}` take `SessionOrToken`; `GET /kbs/{id}/violations/{violation_id}/proof` and `GET /kbs/{id}/derived/{derived_id}/proof` take `AuthUser`.
- **Source**: `web/src/pages/Review.tsx:1409-1672` (all `api.*` calls in the file); `crates/utopia-server/src/api/graph_routes.rs:177-179`, `:524-532`, `:536-556`; `crates/utopia-server/src/api/mod.rs:446-464`
- **Confidence**: high
- **Why it matters here**: Success criterion 2 ("the sentences, facts, or entities the Review tab shows") is met for most sections by the list payload alone; the gaps are fuller evidence (all sentences behind `evidence_count`, the conflict facts' sentences, the violation proof chain), where two of the four existing routes are still session-only.

## Finding: The Alignment badge count and the Alignment list disagree on implication rules

- **Claim**: `review::counts` computes `alignment` from phrase bindings plus undecided type bindings only, while `alignment_queue::list` also returns `implication_rules` with status `proposed`, and `alignment_queue::waiting` (Overview) counts all three.
- **Source**: `crates/utopia-store/src/review.rs:98-99`; `crates/utopia-store/src/alignment_queue.rs:136-148`, `:228-241`
- **Confidence**: high
- **Why it matters here**: Success criterion 1 compares token results with "the same items a signed-in editor sees"; a total taken from `counts.alignment` under-counts the section, so a token client paging by that total can stop before the last rule items.
- **Limits**: Established by code reading; not reproduced against data.

## Finding: The server's only durable name for a calling agent is the personal token's `name`; MCP `clientInfo` is never read

- **Claim**: `mcp::handle` answers `initialize` without reading `params`, keeps no session, and on `tools/call` passes `actor = user.id` and `via_token = token_id` to the tool. The repository contains no occurrence of `clientInfo`, `client_info` or `Mcp-Session-Id` under `crates/`, `docs/` or `web/src/`. The token name is a user-typed 1–64 character string stored in `personal_tokens.name`, with no uniqueness constraint and no rename route (`/me/tokens` has list, issue and revoke only).
- **Source**: `crates/utopia-server/src/api/mcp.rs:171-203` (`initialize`), `:231-242` (`ToolCtx`); `crates/utopia-store/src/tokens.rs:69-83`; `migrations/0017_personal_tokens.sql:12-46`; `crates/utopia-server/src/api/mod.rs:239-246`; repository search for `clientInfo|client_info|Mcp-Session-Id` returned no matches
- **Confidence**: high
- **Why it matters here**: Success criterion 6 needs "the token owner and the agent" on every action; today the token id is the only agent-distinguishing value the server holds, and the product's existing answer to "which agent" is the token name.

## Finding: The product already shows "owner via token name" for `remember`, and nowhere else

- **Claim**: `remember` carries `proposed_by` and `proposed_token` into `pending_facts`; the pending list joins `personal_tokens` and returns `proposed_token_name`; the card renders `pendingSaidVia(owner, tokenName)`. The stated reason in code is that one person can run three agents and a card naming only the person cannot tell them apart (ADR 0026 reference). Audit rows for token calls carry only the token id: `mcp.tool_called` sets `target_kind = personal_token`, `target_id = token id`, `detail = {tool}`; five REST call sites add `detail.token_id` through `audit::via_token`. No audit reader joins `personal_tokens`, and `AuditEventView` has no token field.
- **Source**: `crates/utopia-server/src/api/tools.rs:38-43`, `:1037-1046`; `crates/utopia-store/src/pending.rs:208-217`; `web/src/pages/PendingFacts.tsx:125-133`; `crates/utopia-server/src/api/mcp.rs:247-256`; `crates/utopia-store/src/audit.rs:30-40`, `:129-154`, `:170-214`; `crates/utopia-core/src/models.rs:439-450`
- **Confidence**: high
- **Why it matters here**: The "agent's name" precedent exists for the proposal path (success criterion 4) but Activity and Decisions cannot display it without a new join or a snapshot column; `audit_events` is append-only (`migrations/0007_audit.sql:48-62`), so a name must be written at insert time or resolved at read time.

## Finding: MCP `clientInfo` is self-reported, and a stateless server on the implemented revision sees it only on `initialize`

- **Claim**: In revision 2025-06-18 (the `PROTOCOL_VERSION` this server answers with) `clientInfo` is `{name, title, version}` inside the `initialize` request only; later requests carry `MCP-Protocol-Version` and, only if the server issued one, `Mcp-Session-Id`. This server issues no session id, so it cannot associate a `tools/call` with an earlier `initialize`. In the current revision 2026-07-28 the client identity moves to per-request `_meta["io.modelcontextprotocol/clientInfo"]`, marked not required ("Clients SHOULD include … on every request unless specifically configured not to do so"), and the specification says these values "are self-reported by the sender and are not verified by the protocol … Implementations SHOULD NOT … rely on them for security decisions."
- **Source**: https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle ; https://modelcontextprotocol.io/specification/2025-06-18/basic/transports (Session Management, Protocol Version Header) ; https://modelcontextprotocol.io/specification/versioning (current = 2026-07-28) ; https://modelcontextprotocol.io/specification/2026-07-28/basic/index (`_meta` reserved keys, per-request protocol fields) ; `crates/utopia-server/src/api/mcp.rs:9-18`, `:33`
- **Confidence**: high
- **Why it matters here**: `clientInfo` can label an action for display but cannot be the attribution of record: it is absent on `tools/call` for the revision this server speaks, optional in the current one, and unverified in both; the REST token API has no equivalent field at all.

## Finding: The HTTP `User-Agent` of every token call is already stored on the audit row but is not returned to any reader

- **Claim**: A middleware captures client IP and `User-Agent` (truncated to 256 bytes) for every request and `audit::record_on` writes them to `audit_events.client_ip` and `user_agent`; neither audit list query selects those columns.
- **Source**: `crates/utopia-server/src/client_ctx.rs:40-66`; `crates/utopia-store/src/audit.rs:95`, `:107-123`, `:137-141`, `:188-193`
- **Confidence**: high
- **Why it matters here**: It is a second self-reported client label that exists today for both MCP and REST token calls, with the same trust limits as `clientInfo`.

## Finding: Activity labels governor and adjudicator rows identically, and mislabels the errata agent's run row

- **Claim**: Activity is `GET /kbs/{id}/audit` (`AuthUser`, base role **Admin**), rendering `actor_name`, else "a removed user" when `actor_id` is set, else "AI adjudicator" when the action starts with `review.`, else "the engine". Governor rows are written with `actor_id = NULL` and `detail.via = "governor"`; adjudicator rows with `actor_id = NULL` and a different `via`; both display as "AI adjudicator". The errata agent writes one `errata.reviewed` row per job with `actor_id = Uuid::nil()`, which matches no user, so by the rendering rule it displays as "a removed user"; `web/src` has no label for `errata.reviewed`, `errata.decided` or `mcp.tool_called`, so the raw action string is shown. Individual errata actions the agent applied write no audit row; they exist only in `errata_actions`.
- **Source**: `crates/utopia-server/src/api/kbs.rs:383-410`; `web/src/pages/KbSettings.tsx:547-564`, `:655-665`; `web/src/i18n/en.ts:2548-2551`; `crates/utopia-server/src/governance.rs:891-906`; `crates/utopia-server/src/adjudication.rs:434-450`, `:474-487`; `crates/utopia-server/src/errata.rs:97-107`; `migrations/0007_audit.sql:3-14` (no foreign key on `actor_id`); search of `web/src` for `mcp.tool_called|errata.(reviewed|decided)` returned no matches
- **Confidence**: medium
- **Why it matters here**: Success criterion 6 puts agent actions in Activity; Activity is readable only by a base Admin in a browser session, has no notion of "agent name", and its existing machine-actor conventions (`NULL`, nil UUID) are the opposite of the charter's "token owner and agent" rule.
- **Limits**: The single level is the lower of two. The code paths (route, role, actor values written, absence of labels) are high confidence; the rendered strings are medium, because they are derived from the rendering code and were not observed in a running UI.

## Finding: Agent History is the governor's own table and cannot hold a token agent's action without a schema change

- **Claim**: The `agent` queue lists `agent_decisions`, whose `target_kind` is constrained to `'review'`, whose `action` is `merge|keep|unsure`, whose `status` is `proposed|applied|accepted|overridden|reverted|superseded`, and which has no actor or token column — only `decided_by`, the person who answered. Only `utopia_store::governance::record`, called from the governor job, inserts into it. The errata agent and the older batch adjudicator write nothing there. The row shows the pair, confidence, reason, precedents, trace, and "answered by {person}" (the answering person's display name) when a person has answered.
- **Source**: `migrations/0035_governance_reads_the_ledger.sql:14-40`; `crates/utopia-store/src/governance.rs:727-751`, `:863-890`; `crates/utopia-server/src/governance.rs:567`, `:582-602`; `web/src/pages/Review.tsx:554-661`
- **Confidence**: high
- **Why it matters here**: "Every agent action appears in … Agent History under the token owner and the agent's name" (charter scope and criterion 6) has no column to land in today for any operation other than a duplicate-pair merge or keep, and none for the owner or agent name.
- **Limits**: A search of `migrations/` for `agent_decisions` finds only `0035`, which creates it, and `0036_the_agent_asks_before_it_guesses.sql:9-12`, which adds `question`, `trace`, `calls`; no migration changes the CHECK constraints or adds an actor column.

## Finding: Decisions shows only four action families, so most Review writes never appear there

- **Claim**: `audit::review_history` and the Overview's decided counts filter on `review.%`, `fact.%`, `conflict.%`, `merge.%`. Writes audited as `violation.decided`, `defect.decided`, `alignment.*`, `errata.decided` and `mapping.decided` are excluded. A row with no actor name renders as "AI adjudicator".
- **Source**: `crates/utopia-store/src/audit.rs:128-154`; `crates/utopia-store/src/review_summary.rs:30-31`; `web/src/pages/Review.tsx:679-725` (`e.actor_name ?? S.review.aiActor`)
- **Confidence**: high
- **Why it matters here**: A token agent resolving Axioms, Ontology, Alignment or Errata items would be visible in Activity (Admin-only) but not in the Decisions section an editor reads.

## Finding: `review/batch` decides duplicate pairs only, one action per call, up to 500 ids, with per-item results

- **Claim**: `POST /kbs/{id}/review/batch` (`AuthUser`, Editor) takes `{ids: uuid[], action: "merge"|"keep", rationale?: string}`. `ids` must number 1–500 or the whole call fails with coded error `batch_size`; any other action fails the whole call with a validation error. Each id is a `resolution_reviews` row and runs through the same `decide_review` as the single route, sequentially and without a shared transaction. The response is HTTP 200 `{decided: n, outcomes: [{id, error: string|null}]}`; failed items (not found, already decided, locked by the governor, merge error) carry an error string and the rest proceed. Each success writes one `review.merge` or `review.keep` audit row with `detail = {left, right, score, batch, why}`, where `batch` is the number of ids in the request (`ids.len()`). It then calls `governance::after_human_decision` for the decided ids and emits one review event. No other queue has a batch route.
- **Source**: `crates/utopia-server/src/api/review_routes.rs:471-558`; `crates/utopia-store/src/resolution.rs:1906-1929`, `:1991-2064`; `crates/utopia-core/src/models.rs:1613-1616`; `web/src/api.ts:2591-2599`; `web/src/pages/Review.tsx:1471-1492`
- **Confidence**: high
- **Why it matters here**: The charter's "batch" write exists for exactly one of the thirteen sections; the per-item outcome shape and the 500 cap are the existing contract a token or MCP batch would inherit or have to change.

## Finding: `review/agent/{decision_id}` is a person's answer to one governor decision, with three legal transitions

- **Claim**: `POST /kbs/{id}/review/agent/{decision_id}` (`AuthUser`, Editor) takes `{action: "merge"|"keep"|"revert", rationale?}` and returns `{ok: true}`. Legal combinations: (a) decision `proposed` + `merge|keep` → `decide_review` as the person, decision becomes `accepted` if the action equals the proposal else `overridden`, audit `review.merge|review.keep`; (b) decision `applied` + `revert` → requires a `merge_id` (else coded error `not_a_merge`), `revert_merge`, decision `reverted`, audit `merge.revert`, then the fuse check; (c) decision `applied` with agent action `keep` + `merge` → `merge_entities` with reason "overrode agent keep", decision `overridden`, audit `merge.manual`. Anything else is coded error `agent_answer`. An already-answered decision returns a conflict from `governance::settle`. All audit rows carry `detail.agent_decision`.
- **Source**: `crates/utopia-server/src/api/review_routes.rs:341-469`; `crates/utopia-store/src/governance.rs:981-1007`
- **Confidence**: high
- **Why it matters here**: This route is the existing "revert an agent's merge" and "accept or override a proposal" surface; it is written as the human side of the governor loop, so opening it to a token makes one agent the reviewer of another.

## Finding: A human merge path has no execution gate; the two built-in agents both stop at it

- **Claim**: `execution_gate::hold_for_merge` is called in exactly two non-test places — the governor and the batch adjudicator — and holds a merge for a person when it would touch violations, derivations, answers or an identity source. `decide_review`, `decide_reviews`, `manual_merge` and `agent_answer` call `merge_entities` directly with no gate.
- **Source**: `crates/utopia-server/src/governance.rs:572-592`; `crates/utopia-server/src/adjudication.rs:393-411`; search for `hold_for_merge` across `crates/` (two server call sites, one definition at `crates/utopia-store/src/execution_gate.rs:123`, one test); `crates/utopia-store/src/resolution.rs:2017-2037`; `crates/utopia-server/src/api/review_routes.rs:656-702`
- **Confidence**: high
- **Why it matters here**: Opening the existing merge and batch handlers to a token would give a token agent a wider merge power than the product's own governor has (ADR 0027 gate), in up to 500 merges per request.

## Finding: A token agent's decisions would be stored as human precedent and would steer the governor

- **Claim**: Every Review write records `actor_id = user.id` and `decided_by = user.id`. The governor treats any audit row with `actor_id IS NOT NULL` and action `review.merge|review.keep|merge.manual` as a person's precedent: precedents lower its auto-apply confidence bar from 0.85 to 0.75, a contrary precedent blocks auto-apply, and `settled_by_people` returns a verdict from prior person decisions. After each human decision `after_human_decision` marks the pair's open proposal `accepted` or `overridden`, supersedes sibling proposals, and enqueues a `govern` job when governance is on. The fuse that switches governance off counts `agent_decisions` rows with status `reverted`, which are produced by a person reverting a governor merge.
- **Source**: `crates/utopia-store/src/governance.rs:8-9`, `:20-23`, `:89-126`, `:522-571`, `:595-616`, `:1059-1074`; `crates/utopia-server/src/governance.rs:908-936`, `:978-1009`; `crates/utopia-server/src/api/review_routes.rs:303-336`, `:395-417`
- **Confidence**: medium
- **Why it matters here**: With today's row shapes, an agent acting through a token is indistinguishable from its owner in the ledger: its batch becomes precedent the governor cites and extends to the rest of the cluster, two token-made reverts trip the governance fuse, and no fuse covers the token agent's own merges because they never enter `agent_decisions`.
- **Limits**: The single level is the lower of two. The queries and call sites are high confidence; the end-to-end effect is medium, because it was not exercised against a database.

## Finding: Existing MCP surface is eleven read tools and `remember`; a refused tool call leaves no audit row and no stable code

- **Claim**: `POST /kbs/{id}/mcp` re-authenticates the bearer token on every request. Read tools are exposed to any token that reaches the base and whose owner is at least Viewer; `remember` is listed and callable only when the token scope is `write` and the owner is Editor or above. A call to a non-exposed tool returns JSON-RPC error `-32601` with an English message and returns before the audit insert, so only executed calls produce `mcp.tool_called`. A token whose `kb_ids` exclude the base gets `AppError::Forbidden` from `authorize`. On the REST side, `SessionOrToken::require_write` returns coded 403 errors `token_read_only` and `token_kb_out_of_scope`.
- **Source**: `crates/utopia-server/src/api/mcp.rs:43-69`, `:84-112`, `:197-258`; `crates/utopia-server/src/api/tools.rs:227-257`; `crates/utopia-server/src/api/chat.rs:170`, `:206`, `:345-560` (tool definitions); `crates/utopia-server/src/auth.rs:231-252`
- **Confidence**: high
- **Why it matters here**: Charter scope adds Review tools "beside the existing eleven read tools and `remember`"; success criterion 5 asks for a stable error code on refused writes, which REST has for scope but MCP does not, and criterion 6 has no record of refused attempts.

| MCP tool | Kind | Exposure check | Writes |
|---|---|---|---|
| `search_chunks` | read | any token reaching the base, owner ≥ Viewer | `mcp.tool_called` only |
| `get_document` | read | same | same |
| `search_docs` | read | same | same |
| `find_entities` | read | same | same |
| `entity_facts` | read | same | same |
| `neighbors` | read | same | same |
| `timeline` | read | same | same |
| `paths_between` | read | same | same |
| `changes` | read | same | same |
| `list_rules` | read | same | same |
| `rule_matches` | read | same | same |
| `remember` | propose (document + `memory_ingest` job → `pending_facts`) | scope `write` and owner ≥ Editor | `mcp.tool_called`; pending rows carry owner and token |
| `query_data` | read of external sources | defined for chat, not exposed over MCP | — |

No MCP tool lists a Review section, reads a Review item, or performs a Review write.

## Assigned questions — answers

- How is "the agent's name" known to the server: the token's name, MCP `clientInfo`, or a declared field? What do Activity and Agent History show today for the governor and errata agents? → Today only through the token: the server resolves a bearer token to `user_id` + `token_id` and never reads `clientInfo` or any declared field (`crates/utopia-server/src/api/mcp.rs:171-242`; no match for `clientInfo` in the repository). The one place an agent is named to a person is the Awaiting-your-nod card, which shows "owner via token name" from `personal_tokens.name` (`crates/utopia-store/src/pending.rs:208-217`, `web/src/pages/PendingFacts.tsx:125-133`). Audit rows hold the token id only (`mcp.rs:247-256`, `crates/utopia-store/src/audit.rs:30-40`) plus an unsurfaced `user_agent`. Per the specification, `clientInfo` reaches a server on `initialize` only in revision 2025-06-18 (which this server speaks, statelessly, so it is not available at `tools/call`), is an optional per-request `_meta` field in the current revision 2026-07-28, and is self-reported and not for security decisions (https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle, https://modelcontextprotocol.io/specification/2026-07-28/basic/index). Activity (Admin-only, session-only) shows governor rows as "AI adjudicator" — the same label as the older adjudicator, distinguished only by an undisplayed `detail.via = "governor"` — and shows the errata agent's per-job `errata.reviewed` row with a nil-UUID actor, which the rendering rule turns into "a removed user" with an untranslated action (`web/src/pages/KbSettings.tsx:655-665`, `crates/utopia-server/src/governance.rs:891-906`, `crates/utopia-server/src/errata.rs:97-107`). Agent History lists only the governor's `agent_decisions` rows (pair, action, confidence, reason, precedents, trace, status, and the person who answered); it names no agent and contains nothing from the errata agent (`migrations/0035_governance_reads_the_ledger.sql:14-40`, `web/src/pages/Review.tsx:554-661`).
- Map each of the thirteen Review sections to its existing routes and store functions; name the sections with no dedicated read route. → See the table under the first finding. Sections with their own read route: Overview (`GET /kbs/{id}/review/summary`) and Decisions (`GET /kbs/{id}/review/history`). Sections with no dedicated read route, all served by the shared `GET /kbs/{id}/review?queue=`: Awaiting your nod, Duplicates, Conflicts, Unconfirmed, Low confidence, Axioms, Ontology, Alignment, Errata, Agent History, Merges (Awaiting your nod additionally has the per-chunk `GET /kbs/{id}/review/pending`). All three list handlers are session-only at Viewer; of the write handlers only `decide_alignment_kind_word` accepts a token (`crates/utopia-server/src/api/review_routes.rs:76-156`, `:717-741`, `:1370-1379`; `crates/utopia-server/src/api/mod.rs:532-616`).
- What do `review/batch` and `review/agent/{decision_id}` do today, and what does a batch of agent actions do to the confused-deputy risk? → `review/batch` applies one `merge` or `keep` to 1–500 duplicate-pair ids, item by item with no shared transaction, returning `{decided, outcomes:[{id,error}]}` and one audit row per success; `review/agent/{decision_id}` records a person's accept, override or revert of one governor decision (`crates/utopia-server/src/api/review_routes.rs:341-558`). On the risk, the code shows three amplifiers and this dimension draws no policy conclusion from them: one request can make up to 500 irreversible-until-reverted merges; the human merge path skips the execution gate that both built-in agents obey (`crates/utopia-server/src/governance.rs:572-592`, `crates/utopia-server/src/adjudication.rs:393-411`); and the resulting rows are recorded as the owner's own decisions, which the governor reads as human precedent and extends to sibling pairs, while the governance fuse does not count them (`crates/utopia-store/src/governance.rs:89-126`, `:522-571`; `crates/utopia-server/src/governance.rs:908-936`, `:978-1009`). Each merge writes an `entity_merges` row that `revert_merge` can undo (`crates/utopia-store/src/resolution.rs:2500-2629`). The outside-practice half of this question belongs to the `pitfalls` dimension.

## Dead ends

- Searching the repository for `clientInfo`, `client_info`, `Mcp-Session-Id` under `crates/`, `docs/`, `web/src/` — no matches; there is no partial implementation to build on.
- Searching `docs/decisions/` for a written rule on naming an agent (token name, agent name) — the search terms returned nothing; the rationale exists only as code comments citing ADR 0014 and ADR 0026 (`crates/utopia-server/src/api/tools.rs:38-43`). The ADR texts themselves were not read in this dimension.
- Looking for a Review-tab request that fetches evidence per item (`factEvidence`, proof routes) — `Review.tsx` makes none; those routes are used by other pages.
- Looking for audit rows per errata action applied by the errata agent — none are written; only the per-job `errata.reviewed` row and the person's `errata.decided`.
- The MCP specification "draft" lifecycle URL now serves the versioning page for the stateless revision rather than an `initialize` lifecycle; the handshake facts were taken from the dated 2025-06-18 pages instead.
- Rendered UI strings and the precedent/fuse effects were derived from code and not observed: no server, database, build or test was run in this dimension, per its read-only constraint.
