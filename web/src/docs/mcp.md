# Agents over MCP

Utopia serves every knowledge base as a **Model Context Protocol** server. Any MCP client — Claude Desktop, Cursor, an agent framework, a script — can search a base, read a document, look up an entity and ask what changed, with the same permissions as the person whose token it carries. Reading needs nothing but a token. One tool records: `remember` asks a `write` token held by an editor, and what it records **waits for a person's nod** before it reaches the graph. Six tools reach the Review tab. Two read all thirteen of its sections. Four write, for a `write` token held by an editor: `review_keep` keeps one duplicate pair apart, `review_merge` merges a pair or two entities **behind the server's merge gate**, `review_revert` reverts a merge, and `review_batch` decides up to 50 duplicate pairs in one call. A merge the gate holds changes nothing and waits as a proposal that only a signed-in person can answer; no token and no tool answers a proposal.

## Get a token

Tokens belong to people, not to bases: **Account → Agents & tokens → Personal access tokens**.

| Field | Meaning |
|---|---|
| Name | For your own bookkeeping; shows in the token list and in the audit log |
| Scope | `read` (default) or `write`. Scope is a **ceiling**, not a grant: a token can never do more than its owner can. `remember` needs both — the `write` scope, and editor rights in that base. A viewer's `write` token still cannot record |
| Knowledge bases | Optional. Leave empty and the token reaches every base its owner can open; pick some to narrow it |
| Expires in | 90 days by default |

The token value (`utp_pat_…`) is shown **once**, when it is issued. Revoking it takes effect on the next call.

Anything an agent does with the token is recorded in the base's Activity as the token's owner, with the tool name, so a token is never a way around the audit trail.

## The endpoint

```
POST /api/v1/kbs/{kb_id}/mcp
Authorization: Bearer utp_pat_…
Content-Type: application/json
```

One endpoint per knowledge base, speaking JSON-RPC 2.0 (MCP protocol version `2025-06-18`, stateless HTTP). The `kb_id` is in the base's URL in the browser: `/kb/{kb_id}/…`.

Three methods are served:

- `initialize` — capabilities and protocol version
- `tools/list` — the tools below, with their JSON schemas
- `tools/call` — run one

## The tools

| Tool | What it answers |
|---|---|
| `search_chunks` | Full-text + semantic search over the base's documents. Returns the six best-matching passages, each cut at 800 characters and carrying its `document_id`. Pass `as_of` to search the base as it stood at that moment — earlier versions, documents deleted since; full-text recall stays current, so hits are right but may be incomplete |
| `get_document` | Read one document by `document_id`, all sections in order, up to 24,000 body characters per call. When truncated, call again with the optional `offset` given in the notice to continue |
| `find_entities` | Entities by (partial) name: id, type, and a disambiguator when several share a name. A truncated partial-name result reports its match count; use a more specific name to narrow it |
| `entity_facts` | One entity's facts with validity ranges, paged with `limit` and `offset`. Pass `at` (a date) to see the world as of that day; this is the tool for "who was X in 2024". Pass `as_of` (a date or an RFC3339 moment) to see the facts **as the base held them then**, before later corrections, retractions and merges — "what did we have on record before the memo arrived". The two combine: `at` for the date asked about, `as_of` for when |
| `neighbors` | The entities linked to one entity, one hop, grouped by predicate; narrow with `predicate` or `object_type`. Takes a name as well as an id |
| `timeline` | One entity's dated facts in world-time order; `since` / `until` narrow the window |
| `paths_between` | The chains of facts joining two entities, up to three hops, shortest first. With `at`, every edge must hold at that moment; with `as_of`, the chains as the base held them then |
| `changes` | What the graph learned or revised in a window of **record** time: asserted, corrected, rejected, merged. Needs no entity; use it when the question names a period, not a subject. Returns up to 40 events per call; use `offset` to continue |
| `list_rules` | Business rules with their conditions and conclusions, followed by declared ontology axioms (transitive, symmetric, inverse and sub-property) with the properties and relations they imply; the section says when derivation is off for this base |
| `rule_matches` | The entities a business rule currently marks, with its premises. Takes a business rule ID from `list_rules` |
| `search_docs` | Utopia's own manual, for questions about how the platform works. Never the user's documents |
| `remember` | Record one sentence into the base's memory. **Needs a `write` token held by an editor**; a token without it does not see this tool in `tools/list`, and calling it anyway says why |
| `review_list` | One section of the base's Review tab, any of the thirteen: a page of one of the eleven queue sections with its `total`, a page of the Decisions history, or the Overview summary. See [Review over MCP](#review-over-mcp) |
| `review_item` | One item that `review_list` lists, by its `section` and `id`, exactly as the list returns it; an Axioms item also carries its `proof` |
| `review_keep` | Keep one pending duplicate pair apart. **Needs a `write` token held by an editor**; a token without it does not see this tool in `tools/list`, and calling it anyway returns a tool error with a code that says why |
| `review_merge` | Merge one pending duplicate pair, or two entities named by id. The server's merge gate is asked first: a merge it passes is applied at once, a merge it holds changes nothing and waits as a proposal for a signed-in person. Listed and refused as `review_keep` is |
| `review_revert` | Revert one merge, or every merge one `review_batch` call made. A merge a token agent made is reverted at once; a revert of a person's or the governor's merge waits as a proposal. A revert restores graph state only. Listed and refused as `review_keep` is |
| `review_batch` | Decide 1 to 50 pending duplicate pairs with one action, `merge` or `keep`. Each pair goes through the single-pair path, so the gate is asked for every merge. Listed and refused as `review_keep` is |

The two time axes matter here. `at` reads **world time** (when something was true); `as_of` reads **record time** (what Utopia held at that moment, before it revised it), and `changes` lists what moved on that axis in a window. They are separate parameters on purpose: folded into one they would answer "what happened in March" with "what we learned in March", and both look plausible.

A derived fact returned by `entity_facts` names the rule that produced it. For an ontology
axiom, the text includes its defining property, such as `[rule: inverse of works_at]`.
The structured result's `rule_name` carries the same explanation. That property can
differ from the relation in the derived fact itself, including for historical facts
whose axiom has since been withdrawn.

## The external read contract

**RDF export is the supported machine-readable read contract.**
`GET /api/v1/kbs/{kb_id}/export?format=turtle|jsonld` streams the whole base,
including retracted and corrected assertions, both time axes, quoted evidence,
source documents, and derivations linked to their rules and premise statements.
It uses RDF reification, PROV-O and schema.org. This mapping is the compatibility
boundary: a breaking change needs a decision record explaining why.

Entities, assertions, derivations, rules and documents have UUID-based IRIs:
`urn:utopia:kb:{kb_id}:entity:{id}`, `…:fact:{id}`, `…:derived:{id}`,
`…:rule:{id}` and `…:document:{id}`. The same stored object keeps its identity across exports;
rebuilding a graph is not an identity-preserving operation. `?base=https://example.org/`
instead mints `https://example.org/kb/{kb_id}/{kind}/{id}`. Keep the same base when
joining exports. Imported classes and relations retain their original IRIs.

Ontology export includes explicitly stored `owl:inverseOf` and
`rdfs:subPropertyOf` links, using the target property's imported IRI or existing
key-based IRI. It copies declarations within the base; it does not add reciprocal
links or compute a transitive closure.

A derivation links to `…:rule:{id}` through `prov:wasGeneratedBy`. The rule
resource is typed `prov:Activity` and, by family, `urn:utopia:ns:AxiomRule` or
`urn:utopia:ns:BusinessRule`; it carries an `rdfs:label` (the business rule's
name, or the axiom rule's kind). An axiom rule also states `urn:utopia:ns:axiomKind`
(`transitive`, `symmetric`, `inverse` or `sub_property`) and
`urn:utopia:ns:declaredOn`, the predicate the axiom is declared on, which for
`inverse` and `sub_property` is not the conclusion's predicate. A business rule's
criteria, operands and expressions are not exported: a rule IRI identifies the
stored rule, not a historical version of its definition, and a business rule is
edited in place ([#902](https://github.com/deeplethe/utopia/issues/902)).

MCP remains the agent-facing surface. The structured results below use the same
UUIDs, so an integration can join a selected result to the exported ledger.
Ordinary `/api/v1` UI response shapes are **not** a compatibility promise: they
have no OpenAPI contract or deprecation policy. The export route above is the
explicit exception, not a promise covering every route with that prefix.

The export does not yet include conflict/review state ([#564](https://github.com/deeplethe/utopia/issues/564))
or the chunk identity behind a quote. Per-entity export and a SPARQL endpoint are
also not implemented. Neither MCP nor the export promises historical proof
snapshots: `as_of` selects the derivations held then, but proofs use the current
premise links. Source-document links are the stored provenance, not a separately
versioned snapshot of the evidence set.

## Structured results alongside the text

`find_entities`, `search_chunks`, `entity_facts` and `changes` return a JSON object
in `result.structuredContent`, beside the unchanged human-readable `content`.
No extra argument or protocol upgrade is needed. The six Review tools also return
structured results, with their own shapes and structured errors; they are described
under [Review over MCP](#review-over-mcp), and the rest of this section does not
apply to them. Other tools remain text-only.
Read the JSON field directly; the text is presentation, not a format to parse.

Every structured result includes `kb_id`. IDs are opaque UUID strings, timestamps
are RFC3339 with fractional seconds preserved, unknown optional values are `null`,
and empty collections are `[]`. Clients should tolerate additional fields.
Removing or renaming a documented `structuredContent` field is a breaking change
and requires a decision record explaining why.
Failures have `isError: true` and no structured success payload; an empty result
is a successful read, not an error.

| Tool | Structured fields |
|---|---|
| `find_entities` | `matched_entities`, `truncated`, `entities[]`, in the same ranked order as the text: `id`, `name`, `type_key`, `type_label`, `disambiguator`, `fact_count`. The count is for the partial-name lookup, or `null` when the fallback combined individual words without counting all matches. `truncated` reports the partial-name cap; this is a candidate list, not an exhaustive entity export |
| `search_chunks` | `as_of`, `limit`, `limit_reached`, `chunks[]`. Each chunk has `chunk_id`, `document_id`, zero-based `seq`, `filename`, `text`, `truncated`. The text uses the same 800-character cutoff as the prose |
| `entity_facts` | `entity` (`id`, `name`, `type_key`, `type_label`), effective `at`/`as_of`/`before`, `total_facts`, `matched_facts`, `limit`, `offset`, `next_offset`, `truncated`, `facts[]`, `derived_facts[]` |
| `changes` | Effective `since` (inclusive) and `until` (exclusive), `limit`, `offset`, `next_offset`, `limit_reached`, `changes[]`. Events carry `fact_id`, `at`, `kind`, `subject_id`, `subject_name`, `predicate_label`, `object_name`, `object_value`, `previous_object_name`, `previous_object_value`, `confidence`, `valid_from`, `valid_to`, both validity precisions, `document_id`, `filename`, `quote` |

For a `corrected` event, `object_name` / `object_value` describe the new object;
`previous_object_name` / `previous_object_value` describe the superseded one,
even when that assertion lies outside the requested window. For other event
kinds, both previous fields are `null`. Value fields retain raw JSON, including
units.

An asserted `facts[]` entry contains `id`, `direction`, `predicate_key`,
`predicate_label`, `inferred`, `temporal`, `other_id`, `other_name`, `object_value`,
`confidence`, `valid_from`, `valid_to`, `valid_from_precision`, `valid_to_precision`,
`holds_from`, `holds_to`, `recorded_at`, `invalidated_at`, `supersedes`, and
deduplicated `document_ids`. `direction=out` means the requested entity is the
subject; `in` means it is the object. `object_value` retains the JSON value and
unit rather than formatting them into a string. `inferred` describes an
unaccepted predicate name; it does **not** mean the fact is derived.
`qualifiers[]` preserves attributes attached to the relation itself: each has
`qualifier_type_id`, `key`, `label`, raw `value`, `entity_id`, and `entity_name`.
`statement_qualifiers[]` holds the document's own role words for the statement
(such as `amount` or `lines`), which are not declared attributes: each has `role`,
raw `value`, `entity_id`, `entity_name`, and the `chunk_ids` that say it. Only
chunks live at the read's time speak, so a re-pushed value replaces the old one;
two live chunks that disagree give two entries.

A `derived_facts[]` entry contains `id`, `subject_id`, `subject`, `predicate_id`,
`predicate`, `object_id`, `object`, raw `object_value`, `rule`, `rule_name`,
`rule_id`, `attribute_rule_id`, `confidence`, `valid_from`, `valid_to`, both
validity precisions, `derived_at`, and `invalidated_at`. Rule IDs distinguish
axiom rules from business rules. The derivation ID joins to `…:derived:{id}` in
RDF to follow its proof; this result does not expand the proof tree.

`valid_*` describes stated world time; `holds_*` is the interpreted interval used
for filtering assertions. `recorded_at` (or `derived_at`) and `invalidated_at`
describe the stored record-time lifetime. A later invalidation can be present
even when reading a row held at an earlier `as_of`. `before=T` takes precedence
over `as_of` and reports the effective cutoff `T − 1µs`. A `changes[].at` can be
passed to `before` without losing precision. Events have no separate event UUID;
`fact_id` identifies the affected assertion, and can appear in multiple events.

The structure follows the text selection: assertion filters, `offset`, and the
default 80-fact limit (maximum 300) apply to `facts[]`; derived conclusions only
use `at`/`as_of`/`before` and are listed separately without that limit. Names and
derived conclusions accompany every page. `truncated` says there are more matching
assertions after this page.

For `entity_facts` and `changes`, omit `offset` to start at zero. When `next_offset`
is an integer, call the same tool with that value as `offset` and keep the entity,
filters and time parameters unchanged. A null `next_offset` means there are no
more rows. At or past the end, the page is empty. Facts follow their start time
(undated last), record time and fact ID; changes follow record time newest first,
then fact ID and event kind. These offsets do not hold a snapshot between calls:
new records, corrections or a moving "up to now" window can move page boundaries.

Search returns at most six chunks and changes at most 40 events; `limit_reached`
means the page filled its cap, not that another result is known to exist. Use
`next_offset` to continue changes; narrow the query to inspect more search hits.
Historical full-text search retains the recall limitation described above.

## What an agent records waits for a nod

`remember` stores the sentence immediately — searchable at once, attributed to the token's owner. The facts extracted from it do **not** enter the graph. They queue in Review as proposals, each shown beneath the sentence it came from, and a person confirms or rejects them one at a time.

That gate is why the tool can be served at all. An agent reads documents in the base, and a document can say "remember that X" — so an agent may be talked into recording something by the very material it was asked to read. A proposal that waits for a person costs a click; an assertion that does not costs a wrong edge on the graph, indistinguishable from one someone meant.

The Review card names the agent, not only the person: several clients can share one owner's identity, and "which one said this" is most of what a reviewer has to go on. Never claim to a user that a fact was added — say the sentence was recorded and its facts await confirmation.

## Review over MCP

Six tools serve the Review tab: two reads and four writes. They call the same server functions as the token routes described in the next section, so for one token a tool and its route return the same data and refuse with the same code.

| Tool | Arguments | Listed in `tools/list` for | Successful `structuredContent` |
|---|---|---|---|
| `review_list` | `section` (required string); `limit` and `offset` (optional integers, queue sections only); `page` and `per` (optional integers, `decisions` only) | Every token that reaches the base, `read` or `write`, with an owner who is at least a viewer | The body the token route of that section returns: `{"counts", "queue", "items", "total"}` for a queue section, the summary object for `overview`, `{"events", "total"}` for `decisions` |
| `review_item` | `section` (optional string, default `duplicates`); `id` (required string) | The same | That item exactly as `review_list` returns it. For section `violations`, the same object plus `proof` |
| `review_keep` | `id` (required), `rationale` (optional string) | Only a `write` token whose owner is at least an editor of the base | `{"ok": true, "outcome": "applied", "action_id": "<uuid>"}`, the body of the keep route |
| `review_merge` | Either `id` (the review id of a pending duplicate pair) or both `source` and `target` (two entity ids; `source` is merged into `target`); `rationale` (optional string). No argument is marked required in the schema, because "one form or the other" cannot be expressed there; the server checks it | The same as `review_keep` | The body of the matching merge route: `{"ok": true, "outcome": "applied", "action_id", "merge_id"}`, or `{"ok": true, "outcome": "held" or "proposed", "action_id", "reason", "explanation"}` when the call became a proposal |
| `review_revert` | Exactly one of `merge_id` (one merge) and `batch_id` (every merge of one batch); `rationale` (optional string). No argument is marked required | The same as `review_keep` | With `merge_id`, the body of the revert route: `{"ok": true, "outcome": "applied", "action_id", "merge_id"}` or `{"ok": true, "outcome": "proposed", "action_id", "reason", "explanation"}`. With `batch_id`, the body of the unit revert: `{"batch_id", "reverted", "outcomes"}` |
| `review_batch` | `ids` (required list of review ids), `action` (required, `merge` or `keep`), `rationale` (optional string) | The same as `review_keep` | The body of the token batch: `{"batch_id", "decided", "outcomes"}` |

What each write does, every key of its answer and every reason a call becomes a proposal are described once, under [Review over the token API](#review-over-the-token-api): a tool and its route run the same server function, so the answers are the same object.

**Sections served.** `review_list` reads all thirteen sections of the Review tab. Eleven are queue sections that share one list route; Overview and Decisions each have their own route. Every route below is under `/api/v1`.

| Section in the Review tab | Key | Token route | `review_list` arguments |
|---|---|---|---|
| Overview | `overview` | `GET /kbs/{kb_id}/review/summary` | `section` |
| Awaiting your nod | `pending` | `GET /kbs/{kb_id}/review?queue=pending` | `section`, `limit`, `offset` |
| Duplicates | `duplicates` | `GET /kbs/{kb_id}/review?queue=duplicates` | `section`, `limit`, `offset` |
| Conflicts | `conflicts` | `GET /kbs/{kb_id}/review?queue=conflicts` | `section`, `limit`, `offset` |
| Unconfirmed | `unconfirmed` | `GET /kbs/{kb_id}/review?queue=unconfirmed` | `section`, `limit`, `offset` |
| Low confidence | `lowconf` | `GET /kbs/{kb_id}/review?queue=lowconf` | `section`, `limit`, `offset` |
| Axioms | `violations` | `GET /kbs/{kb_id}/review?queue=violations` | `section`, `limit`, `offset` |
| Ontology | `defects` | `GET /kbs/{kb_id}/review?queue=defects` | `section`, `limit`, `offset` |
| Alignment | `alignment` | `GET /kbs/{kb_id}/review?queue=alignment` | `section`, `limit`, `offset` |
| Errata | `errata` | `GET /kbs/{kb_id}/review?queue=errata` | `section`, `limit`, `offset` |
| Agent History | `agent` | `GET /kbs/{kb_id}/review?queue=agent` | `section`, `limit`, `offset` |
| Decisions | `decisions` | `GET /kbs/{kb_id}/review/history` | `section`, `page`, `per` |
| Merges | `merges` | `GET /kbs/{kb_id}/review?queue=merges` | `section`, `limit`, `offset` |

A paging argument the section does not take is ignored: `overview` reads none, `decisions` does not read `limit` or `offset`, and a queue section does not read `page` or `per`. The route's `types` filter for Duplicates is not a tool argument; `review_list` reads the Duplicates section unfiltered. Any other section name, `mappings` included, is refused with `unknown_queue` and returns no items. Reading is all these sections offer: nothing listed is decided by reading it.

**Totals and paging.** A queue section's result carries `total`: the number of rows that section's list returns over all pages, so the last item is at `offset = total - 1`. `counts` holds the Review tab's badges, with the values they always had, and for three sections the badge named after the section is not the number of listed rows: `counts.alignment` leaves out proposed implication rules, `counts.lowconf` leaves out derived facts that the list includes, and `counts.agent` counts the governor's open proposals only. For Alignment, Low confidence and Agent History, page by `total`, not by `counts`. An empty section is `"items": []` with `"total": 0`, not an error.

**Page size.** For a queue section, `limit` defaults to 10 and is served as 1 to 200: a value below 1 returns one item and a value above 200 returns 200. An out-of-range `limit` is not an error. `offset` defaults to 0, and a negative value is served as 0. An `offset` at or past `total` returns no items and the true `total`. The Alignment list is ordered by each item's `decided_at` and then by the item's own identifier (the `id` of a phrase or rule, the word of a kind word), so paging by `limit` and `offset` over an unchanged queue visits each waiting item exactly once.

**Decisions.** `section: "decisions"` pages with the history route's own `page` and `per`, not with `limit` and `offset`. `page` is zero-based and defaults to 0 (a negative value is served as 0); `per` defaults to 20 and is served as 1 to 100. The result is `{"events", "total"}`: one page of decision events, newest first, each with `id`, `action`, `target_kind`, `target_id`, `detail`, `actor_id`, `actor_name` and `created_at`, and the number of events in the history. A page past the end has no events and the same `total`.

**Agent History.** `section: "agent"` returns rows from two sources in one sequence, newest first, and `limit`, `offset` and `total` apply to the merged sequence. A governor row is the row the Review tab has always shown and has no `source` key. A row for an action made with a token has `"source": "token"` and names both the person and the token: `owner_id` and `owner_name` for the token's owner, `token_id` and `token_name` for the token (the name is the snapshot stored with the action, so a renamed or revoked token still reads as it was), beside `id`, `operation`, `target_kind`, `target_id`, `outcome`, `left`, `right`, `rationale` and `created_at`. `operation` is `review.keep`, `review.merge` (a queue merge), `merge.manual` (a merge of two entities named by id) or `merge.revert`; `outcome` is `applied`, `held` (the merge gate held the call) or `proposed` (the call may only propose). Five more keys are present only when they have a value, so the row of a single keep has none of them: `reason` (why a `held` or `proposed` call was not applied; the reason codes are listed in the next section), `batch_id` (the call was a member of that batch, or a revert made by the unit revert of that batch), `merge_id` (on an applied merge the merge it made, on a `merge.revert` row the merge it targets, on an approved merge proposal the merge the person's approval made), and `state` with `state_at`. A row with outcome `held` or `proposed` and no `state` is an open proposal. `state` is `approved`, `declined` or `superseded` once a person answered the proposal, and `undone` on an applied merge row whose merge has since been reverted, by anyone; `state_at` is the time of the answer or of the revert. On a base where no token has acted, the section holds the governor's rows only.

**One item.** `review_item` takes the `section` an item is listed in and the item's `id`, and returns that item with the fields `review_list` returns for it. `section` may be any of the eleven queue sections or `decisions`; left out, it is `duplicates`.

- `id` is the item's `id` from the list: a uuid. For `decisions` it is the event's `id`; for `agent` it is the row's `id`, whichever source the row has.
- An Alignment item of kind `kind_word` has no `id`. Pass its `kind_word` as `id`; it is compared exactly as stored, with the same letter case and spacing.
- A Duplicates item is the pair: `id`, `score`, `reason`, `stage`, `created_at`, `left` and `right` (each with `id`, `name`, `type_label`, `color`, `disambiguator`, `degree`, `top_facts`), and `proposal` (the governor's open proposal, or `null`). When a token agent's proposal is open on the pair, the item also has `agent_proposal`: `{"id", "operation", "reason", "owner_name", "token_name", "created_at"}`. That is an open queue-merge proposal for this review id or, when there is none, an open manual-merge proposal naming the pair's two entities in either direction. The key is absent when no such proposal is open; it is never `null`.
- A Merges item has `id` (the merge id that `review_revert` takes), `source_name`, `target_name`, `merged_by_name`, `reason`, `created_at` and `reverted_at`. A merge that a token agent made also has `agent`: `{"action_id", "owner_name", "token_name"}`, where `action_id` is the ledger row of the merge; `merged_by_name` on such a row is the token's owner, so read `agent` to tell an agent's merge from a merge the owner made by hand. A merge made by a person approving a proposal is that person's merge and has no `agent`. A merge with an open revert proposal has `agent_proposal`, the same object as on a Duplicates item. Both keys are absent when they have no value.
- An Axioms item (section `violations`) carries one more key, `proof`: the proof steps when the item's `kind` is `derived_contradiction`, and `null` for any other kind, which is not an error. These are the `steps` that `GET /kbs/{kb_id}/violations/{violation_id}/proof` returns. No other section gains a key.
- An id that is not listed in that section of this base gets one answer, `not_found`, with no evidence in it: the same for an unknown id, an id of another base and an item that is no longer waiting there. `section: "overview"` lists no items, so any id with it is `not_found`.

**Who may call.** The two read tools need a token that reaches the base and an owner who is at least a viewer. The four write tools (`review_keep`, `review_merge`, `review_revert`, `review_batch`) need the `write` scope and an owner who is at least an editor; the scope is checked before the owner's role. A caller without that right does not see any of the four in `tools/list`. If it calls one anyway, the answer is a tool error with a code (`token_read_only` or `token_owner_below_editor`), not a JSON-RPC error. No tool answers a proposal, the token's own included: a call to any other name beginning `review_` is JSON-RPC error -32601, and the proposal stays open. A token that is unknown, revoked or expired gets HTTP 401 from the endpoint. A token whose knowledge-base list excludes the base is turned away by the endpoint itself with HTTP 403 before any tool runs, and gets no tool list, item, count or evidence. The token API answers that same token with 404 (next section); the two answers differ, and neither carries anything from the base.

**A keep made by an agent is recorded under the token's owner and the token's name**: the audit row has the owner as its actor and carries the token's id and the name the token had at that moment, and the same keep is one row in the token-agent ledger (`action_id`). The keep is applied at once, changes no entity and no fact, and is not treated as a person's decision; the paragraph on keeps in the next section applies unchanged. Merges, reverts and proposals made through the tools are recorded the same way, as the next section describes for the routes. Each call to any of the six tools, a refused one included, also writes the `mcp.tool_called` row that every MCP tool call writes.

**Errors.** A refused or failed call is a normal `tools/call` result with `isError: true`. Its `structuredContent` is `{"code": "<code>", "message": "<text>"}`, where `code` and `message` are the `code` and `error` of the response the matching token route gives. `content` holds the same message as text.

| Code | Tool | When |
|---|---|---|
| `unknown_queue` | `review_list`, `review_item` | `section` is not one of the thirteen keys above. `mappings`, a list queue the browser uses that is not a Review section, is refused the same way |
| `not_found` | `review_item` and the four write tools | The id is not listed in the named section of this base, or the section is `overview` (`review_item`); no pair with that id is in this base (`review_keep`, `review_merge` with `id`); an entity id is unknown or of another base (`review_merge` with `source` and `target`); the merge id is unknown or of another base, or no token action of this base carries the batch id (`review_revert`). The routes answer these with 404 and no code |
| `token_read_only` | the four write tools | The token has the `read` scope |
| `token_owner_below_editor` | the four write tools | The token's owner is below editor in the base |
| `already_decided` | the four write tools | The pair was decided before this call, or one of the two entities was already merged into another (`review_keep`, `review_merge`); the merge was already reverted, by anyone (`review_revert`) |
| `self_merge` | `review_merge` | `source` and `target` are the same entity |
| `batch_size` | `review_batch` | `ids` is empty or has more than 50 entries. The whole batch is refused and nothing is changed |
| `agent_rate_limited` | the four write tools | The token has finalized 200 writes on this base in the last hour. Nothing is changed |

In the result of `review_batch`, and of `review_revert` with `batch_id`, a refusal of one member is not a tool error: it is that member's `code` and `error` in `outcomes`, and the call itself succeeds. `already_proposed` is not an error either: it is the `code` key of a successful `held` or `proposed` answer that met an open proposal.

An error for which the route has no code has `structuredContent` `{"message": "<text>"}` with no `code` key: the governor is deciding the pair right now (nothing was decided; try again after its proposal), `review_batch` was called with an `action` other than `merge` or `keep` (the whole batch is refused), or the server failed. A refused call writes no ledger row and changes nothing. A call with a missing or malformed argument is not run; it returns `isError: true` with a text message and no `structuredContent`. That covers: no `section` for `review_list`, a `limit` that is not an integer, a `section` that is not a string, an id argument of a write tool that is not a uuid (one entry of `ids` is enough to stop the whole batch), `review_merge` called with both forms, half of the `source` and `target` form or neither form, and `review_revert` called with both `merge_id` and `batch_id` or with neither. For `review_item`, an `id` that is not a uuid is not malformed, because a kind word is not a uuid: outside the Alignment section it is answered `not_found`.

The annotations on these tools (`readOnlyHint` on the two reads; `readOnlyHint`, `destructiveHint` and `idempotentHint`, all `false`, on `review_keep`; `readOnlyHint: false`, `destructiveHint: true` and `idempotentHint: false` on `review_merge`, `review_revert` and `review_batch`) are hints for the client. The server does not read them, and it does not use elicitation: what a token may do is decided by the checks above.

## Review over the token API

A token can also call ten of the HTTP routes behind the Review tab and the entity panel's "Merge into…": five reads and five writes (the decision route for a keep or a merge, the manual merge, the merge revert, the batch, and the unit revert of a batch). Nine are the routes the browser uses — same URLs, same handlers — so an agent reads what a signed-in editor sees; the unit revert is new and has no button in the Review tab. On every write route a signed-in person's request runs the code it ran before and gets the answer it got before: the gate, the proposals, the limit and the breaker described below apply to token callers only. All are under `/api/v1` and take `Authorization: Bearer utp_pat_…`.

| Route | What it does | Who may call it |
|---|---|---|
| `GET /kbs/{kb_id}/review?queue=<key>` | One page of a queue section, as `{"counts", "queue", "items", "total"}`. `queue` is one of the eleven queue keys in the table under [Review over MCP](#review-over-mcp) (`pending`, `duplicates`, `conflicts`, `unconfirmed`, `lowconf`, `violations`, `defects`, `alignment`, `errata`, `agent`, `merges`); left out, it is `duplicates`. Optional `limit` (default 10, served as 1 to 200) and `offset` (default 0; a negative value is served as 0), and for `duplicates` only, `types` (`any`, `same`, `conflict`). `counts`, `total` and the order of `items` are the ones a session gets for the same request; an empty section is `"items": []` with `"total": 0`, not an error | Any token (`read` or `write`) that reaches the base, with an owner who is at least a viewer |
| `GET /kbs/{kb_id}/review/summary` | The Overview summary: what is waiting, what was decided, the base's health, the governor's recent rows | The same |
| `GET /kbs/{kb_id}/review/history?page=&per=` | One page of the Decisions history, as `{"events", "total"}`. `page` is zero-based (default 0); `per` defaults to 20 and is served as 1 to 100 | The same |
| `GET /kbs/{kb_id}/violations/{violation_id}/proof` | The proof behind one Axioms item, as `{"steps"}`: the proof steps for a violation of kind `derived_contradiction`, and `"steps": null` for any other kind or an id that is not a violation of this base | The same |
| `GET /kbs/{kb_id}/derived/{derived_id}/proof` | The proof of one derived fact, as `{"proof"}`: its premises in order, each with its evidence. `"proof": null` when the derivation is withdrawn or does not exist | The same |
| `POST /kbs/{kb_id}/review/{review_id}` with `{"action": "keep", "rationale": "…"}` | Keeps one pending duplicate pair apart. `rationale` is optional (a string or `null`). Answers `{"ok": true, "outcome": "applied", "action_id": "<uuid>"}` | A `write` token that reaches the base, with an owner who is at least an editor |
| `POST /kbs/{kb_id}/review/{review_id}` with `{"action": "merge", "rationale": "…"}` | The queue merge: merges one pending duplicate pair behind the merge gate, in the direction a person's queue merge takes for that pair. See [Merging](#merging) | The same |
| `POST /kbs/{kb_id}/entities/merge` with `{"source": "<entity id>", "target": "<entity id>", "rationale": "…"}` | The manual merge: merges `source` into `target` behind the same gate | The same |
| `POST /kbs/{kb_id}/merges/{merge_id}/revert` with no body or `{"rationale": "…"}` | Reverts one merge, or proposes its revert. See [Reverting a merge](#reverting-a-merge) | The same |
| `POST /kbs/{kb_id}/review/batch` with `{"ids": ["<review id>", …], "action": "merge" or "keep", "rationale": "…"}` | Decides 1 to 50 pending duplicate pairs under one `batch_id`. See [Batches](#batches) | The same |
| `POST /kbs/{kb_id}/review/agent-batches/{batch_id}/revert` with no body or `{"rationale": "…"}` | The unit revert: reverts every merge that batch made | The same, and also a signed-in editor of the base |

`rationale` is optional on every write (a string or `null`).

What `total` counts, why three sections are paged by it and not by `counts`, the page clamp, an offset past the end, the Alignment order and the two kinds of Agent History row are described under [Review over MCP](#review-over-mcp); they are properties of the list itself and are the same on this route. `total` is on every list response, for a signed-in session as well; it is the only key the list response gained. The token API has no route for a single item: an item's fields are the list's, and the fact evidence and entity reads a token already had, with the two proof routes above, give the rest.

A keep made with a token is applied at once and is recorded three ways in one transaction: the pair is marked kept by the token's owner through that token, a row is written to the token-agent ledger (`action_id` is its id), and an audit row is written with the owner as actor plus the token's id and the name it had at that moment. If any of the three cannot be written, the keep fails and the pair stays in the queue. A token's keep is **not** treated as a person's decision: the governor does not read it as precedent, it starts no governor round, and a governor proposal open on the pair stays open for a person to answer.

### Merging

Both merge routes ask the server's merge gate before anything is written; there is no token merge that skips it. The queue merge names a pending pair by its review id and merges in the direction a person's queue merge takes for that pair: the entity with more live facts survives, and on a tie the one created earlier. The manual merge names two entities and merges `source` into `target`, as the caller says.

**The gate passes.** The merge is applied at once and the answer is HTTP 200:

```json
{"ok": true, "outcome": "applied", "action_id": "<uuid>", "merge_id": "<uuid>"}
```

`merge_id` names the merge (it is the id the revert route takes) and `action_id` the ledger row. The merge, the pair's review row (marked merged by the token's owner through that token), the ledger row and the audit row are written in one transaction: if the ledger or audit row cannot be written, the merge is not made. The audit row is `review.merge` for a queue merge and `merge.manual` for a manual merge, with the owner as actor and the token's id, the token's name at that moment, the `action_id` and the `merge_id` in its detail. A manual merge that applies also closes the two entities' own pending duplicate pair, if there is one, as merged. Some steps run after the merge is committed (re-checking signatures, reconciling timelines, refreshing disambiguators). If one of them fails, the merge still exists, so the answer is still 200 and `applied`, with one more key, `"follow_up_error": "<text>"`. The key is absent when nothing failed. Do not read it as a failed merge.

**The gate holds, or the token may only propose.** Nothing in the graph changes: no fact, entity or review row is touched, and retrieval and reasoning see what they saw before. The call is stored as a proposal and the answer is still HTTP 200:

```json
{"ok": true, "outcome": "held", "action_id": "<uuid>", "reason": "kept_by_person", "explanation": "Not applied: a person kept these two apart. Nothing was changed; the call waits as a proposal that a signed-in editor answers in the Review tab."}
```

`outcome` is `held` when the gate held the merge and `proposed` when the call may only propose. `reason` is a stable code; `explanation` is a sentence for the reader and may be reworded. `action_id` is the proposal, a ledger row holding the full request; an `agent.proposal` audit row is written with it.

| `reason` | `outcome` | When |
|---|---|---|
| `identity` | `held` | A source that declares its names to be identifiers stands behind a name of one of the two entities |
| `contradiction` | `held` | The merge would put two values of a one-value relation on one entity, and they cannot be ordered in time |
| `derived` | `held` | Live derived facts rest on one of the two entities |
| `answered` | `held` | One of the two entities was named in a chat answer |
| `kept_by_person` | `held` | The latest decided duplicate pair of these two entities, in either order, was kept apart by a person and not through a token |
| `reverted_merge` | `held` | A merge between these two entities, in either direction, has been reverted |
| `merge_by_person` | `proposed` | Revert only: a person made the merge, including a merge made by approving a proposal |
| `merge_by_governor` | `proposed` | Revert only: the merge has no person behind it; the governor or the automatic adjudicator made it |
| `breaker` | `proposed` | The token's breaker is tripped (see [The limit and the breaker](#the-limit-and-the-breaker)); merges and reverts |

The first six are the gate's holds, checked in the order of the table; the first that applies is the reason. `kept_by_person` and `reverted_merge` are looked up by the two entity ids, for the queue merge and the manual merge alike. When the breaker is tripped the reason is `breaker` whatever the gate would have said.

A token's merge, like its keep, is **not** treated as a person's decision: the governor does not read a live token merge as precedent, it starts no governor round, and a governor proposal open on the pair is not answered by it.

### Proposals

There is one open proposal per base, operation and target. The target of a queue merge is its review id; the target of a manual merge is the unordered pair of entity ids, so `A` into `B` and `B` into `A` are the same proposal and it keeps the direction of the first call; the target of a revert is the merge id. A repeated call, or a second call arriving at the same moment, writes nothing and returns the open proposal: the same 200 answer with the existing `action_id` and one more key, `"code": "already_proposed"`.

An open proposal is read as a row of Agent History (outcome `held` or `proposed`, no `state`) and as `agent_proposal` on the Duplicates item or Merges item it targets; both are described under [Review over MCP](#review-over-mcp).

**Only a signed-in person answers a proposal.** The two answer routes take a browser session and nothing else. Every personal token gets 401 from them, the token that made the proposal included, and no MCP tool answers one.

| Route | Body | Answer |
|---|---|---|
| `POST /kbs/{kb_id}/review/agent-proposals/{proposal_id}` | `{"action": "approve"}` or `{"action": "decline"}` | `{"ok": true, "state": "approved"}`, with `state` one of `approved`, `declined`, `superseded`, plus `"merge_id"` when a merge was made or reverted |
| `POST /kbs/{kb_id}/review/agent-proposals` | `{"ids": ["<proposal id>", …], "action": "approve" or "decline"}`, 1 to 500 ids | `{"answered": n, "outcomes": [{"id", "state", "merge_id", "code", "error"}]}`, one outcome per id in request order, all five keys always present |

- The caller must be an editor of the base; a signed-in person below editor gets 403.
- **Approve** runs the person's own merge or revert, with that person as actor and no gate, exactly as if they had done it in the Review tab; the audit row of that merge or revert carries `agent_proposal`, the proposal's id. It counts as that person's decision everywhere, so the governor reads it as human precedent and the governance fuse counts an approved revert as that person's revert.
- Approving a proposal whose target can no longer be decided closes it as `superseded` and applies nothing: the pair was decided meanwhile, one of the entities was merged into another, the pair now names other entities, or the merge was already reverted.
- **Decline** closes the proposal and changes nothing else.
- A proposal that was already answered gets 409 `already_decided`; an id that is unknown, of another base or not a proposal gets 404. In the bulk answer these are that item's `code` (`already_decided`, `not_found`) with `state` null, and the other items are still answered.
- An `action` other than `approve` or `decline` is 422 `unknown_action`; an empty list or more than 500 ids is 422 `batch_size`. Both refuse the whole request and answer nothing.
- While a running governor round holds the pair, approving the queue-merge proposal on it answers 409 with no code (in the bulk answer: `code` null with the message in `error`). Nothing was decided and the proposal stays open; answer it again later.
- Approving a manual-merge proposal leaves the two entities' own pending duplicate pair open, as a person's manual merge does. (A token's own applied manual merge closes it.)

Known limit: an approval is not one transaction with the merge or revert it runs. If the server stops between the two, the merge exists as the person's merge and the proposal is still open; answering it later closes it `superseded` with no `merge_id`.

### Reverting a merge

`POST /kbs/{kb_id}/merges/{merge_id}/revert` takes no body, or `{"rationale": "…"}`.

- **A merge a token agent made** on this base, through this token or another one, and that has not been reverted, is reverted at once: `{"ok": true, "outcome": "applied", "action_id": "<uuid>", "merge_id": "<uuid>"}`, where `merge_id` is the merge that was reverted. A `merge.revert` ledger row and audit row are written under the token's owner and the token's name. `follow_up_error` can appear here as it can on a merge.
- **A merge a person or the governor made** is not reverted by a token. Nothing changes and the call waits as a proposal: `{"ok": true, "outcome": "proposed", "action_id": "<uuid>", "reason": "merge_by_person", "explanation": "…"}`, with `reason` one of `merge_by_person`, `merge_by_governor` and `breaker`. A repeat carries `"code": "already_proposed"`. A token therefore cannot move the governance fuse. When a person approves the proposal, the revert runs as that person's and the fuse counts it as it counts any revert by a person.
- A merge that was already reverted, by anyone, is 409 `already_decided`. A merge id that is unknown or of another base is 404.
- A body that is neither empty nor a JSON object (with `rationale`, if present, a string or `null`) is 422 `invalid_body`, after the scope and role checks.

**A revert is not a full undo.** It restores graph state only: the facts, the entities and the evidence go back to where they were before the merge. It does not recall derivations, violations, exports or answers drawn while the merge was live: a fact derived from the merged entity, a violation recorded on it, a triple already exported and an answer already given stay as they are. Two differences from the state before the merge also remain in the graph:

- **The untyped target.** A target that had no type before the merge (an untyped entity) keeps the type it took from the source.
- **Copied statement links.** When both entities held the same typed fact, the merge kept one of them and copied the duplicate's statement links onto the surviving fact. A revert does not remove those statement links; the surviving fact keeps them.

Review rows are not restored either: a pair the merge closed stays closed. After a revert the merge's own ledger row reads `state: "undone"`, whoever reverted it. A signed-in person reverts an agent's merge from the Merges section as before, on this same route.

Known limits: a token revert is not one transaction with its ledger and audit rows. The server writes them right after the revert and retries; if they still cannot be written the call fails although the merge is reverted, the merge's ledger row reads `undone`, and a second revert answers `already_decided`. A person's revert and a token's revert of one merge sent at the same moment can both run the revert; nothing on the token path prevents it, as nothing prevents two people's reverts from doing so.

### Batches

`POST /kbs/{kb_id}/review/batch` from a token takes 1 to 50 review ids and one action, `merge` or `keep`. (A signed-in person's batch on the same route is unchanged: up to 500 ids, no gate, and its old answer `{"decided", "outcomes": [{"id", "error"}]}`.) The server issues one `batch_id` and passes each id, in request order, through the single-pair path above. A batch member gets nothing a single call would not: the gate is asked for every merge, and the limit and the breaker apply per member. The answer is HTTP 200:

```json
{"batch_id": "<uuid>", "decided": 1, "outcomes": [
  {"id": "<review id>", "outcome": "applied", "action_id": "<uuid>", "merge_id": "<uuid>", "reason": null, "code": null, "error": null},
  {"id": "<review id>", "outcome": "held", "action_id": "<uuid>", "merge_id": null, "reason": "derived", "code": null, "error": null},
  {"id": "<review id>", "outcome": null, "action_id": null, "merge_id": null, "reason": null, "code": "not_found", "error": "…"}
]}
```

- One outcome per id, in request order. All seven keys are always present and `null` when they do not apply. `outcome` is `applied`, `held` or `proposed`; it is `null` when that pair was refused, and then `code` and `error` say why: `not_found` for an unknown id, `already_decided`, `agent_rate_limited` for each pair past the limit, or `code` null with a message when the governor is deciding that pair. `code` is `already_proposed` on a `held` or `proposed` member that met an open proposal. An applied member can also carry `follow_up_error`.
- `decided` counts the `applied` members. One refused pair does not stop the others, and the batch is not atomic.
- An id given twice gets, the second time, the answer its state then gives: `already_decided` after an applied first time, the same proposal with `already_proposed` after a held one.
- Refused as a whole, with nothing changed and no ledger row: an empty list or more than 50 ids (422 `batch_size`), and an action other than `merge` or `keep` (422 with no code). The scope and role checks come first, so a `read` token sending 51 ids gets `token_read_only`.
- Every ledger row and audit row of the batch carries the `batch_id`, in Agent History and in Activity.
- Revoking the token does not stop a batch that is already running; the cap of 50 bounds what one call can do.

**The unit revert.** `POST /kbs/{kb_id}/review/agent-batches/{batch_id}/revert` reverts every merge the batch made, best effort. A `write` token of an editor and a signed-in editor may both call it; there is no button for it in the Review tab. Answer, HTTP 200:

```json
{"batch_id": "<uuid>", "reverted": 1, "outcomes": [
  {"merge_id": "<uuid>", "outcome": "applied", "code": null, "error": null},
  {"merge_id": "<uuid>", "outcome": null, "code": "already_decided", "error": "…"}
]}
```

- One outcome per merge the batch applied, in the order the merges were made; `reverted` counts the `applied` ones. A merge that was already reverted is that item's `already_decided`, so a second unit revert changes nothing.
- Pairs the batch kept apart and members that became proposals are left as they are. A batch that made no merge (all keeps, or all held) answers 200 with `"reverted": 0` and `"outcomes": []`. 404 is only for a batch id that no token action of this base carries.
- Called with a token, each merge goes through the token revert above, with its limit and breaker: an item can come back `proposed` (the breaker is tripped) or with `code` `agent_rate_limited`, and the revert rows carry the `batch_id`. Called by a signed-in editor, each merge goes through the person's revert, recorded as that person's.
- A token's body follows the rule of the revert route (empty or `{"rationale": "…"}`, otherwise 422 `invalid_body`).

### The limit and the breaker

Both are checked for every token write that would finalize, in the one place all entries share, so they hold alike for the decision route, the manual merge, the revert, each batch member and the MCP tools. Neither is a per-base setting.

**The limit.** A token finalizes at most 200 writes per base in a rolling hour. Keeps, queue merges, manual merges and reverts count, each when its ledger row is `applied`; proposals and refused calls do not. The 200th write applies; the next call answers **HTTP 429 with code `agent_rate_limited`**, changes nothing and writes no ledger row, until earlier writes are more than an hour old. Two things to know: the limit is checked before the gate, so a call past the limit answers 429 even if it would only have become a proposal; and the count is read before the write, not in its transaction, so calls of one token that are in flight at the same moment can overshoot the limit by the number in flight.

**The breaker.** When people have reverted two merges made by one token on one base within seven days, that token's merges and reverts on that base stop being applied: each lands as a proposal with reason `breaker`, batch merge members and unit revert items included. Its keeps still finalize. The breaker closes by itself when fewer than two such reverts lie within the last seven days, counted from the time of each revert.

- Counted: a revert by a person of a merge this token made — from the Merges section, by approving a revert proposal, or by a signed-in unit revert.
- Not counted: a revert made by a token (this one or another), and a revert of a merge another token made. One revert by a person does not trip it.
- A person's revert is recognised from the ledger (the merge is reverted and no applied token revert names it) and also from the person's own `merge.revert` audit row, so it still counts when a token reverted the same merge at the same moment.

The breaker uses the numbers of the governance fuse (two reverts, seven days) but is separate from it: it is per token and base, and a token's own reverts never move the fuse.

### Refusal codes

Refusals carry a stable `code` beside the `error` message where one is listed:

| Status | Code | When |
|---|---|---|
| 401 | none | The token is unknown, revoked or expired; or the route is one of the Review routes that take a signed-in session only (see below), the two proposal answer routes included |
| 404 | none | On the five reads: the token's knowledge-base list excludes the base, or its owner cannot open it. The answer is the same as for a base that does not exist, and carries no item, count, total, summary or proof. (The MCP endpoint answers a token whose list excludes the base with HTTP 403 instead.) On a write: no pair with that id in this base (keep, queue merge); an entity id that is unknown or of another base (manual merge); a merge id that is unknown or of another base (revert); a batch id that no token action of this base carries (unit revert) |
| 422 | `unknown_queue` | A token asks the list for `mappings` or for any name that is not one of the eleven queue keys. No items are returned |
| 422 | `unknown_type_filter` | `types` on the Duplicates section is not `any`, `same` or `conflict` |
| 422 | `self_merge` | The manual merge names one entity as both `source` and `target` |
| 422 | `batch_size` | A token batch has no ids or more than 50 (a signed-in person's batch: more than 500). The whole batch is refused |
| 422 | none | A batch names an action other than `merge` or `keep`. The whole batch is refused |
| 422 | `invalid_body` | A token sends the revert or the unit revert a body that is neither empty nor a JSON object such as `{"rationale": "why"}` |
| 403 | `token_read_only` | A `read` token calls any of the five write routes |
| 403 | `token_kb_out_of_scope` | A `write` token whose knowledge-base list excludes the base calls any of the five write routes |
| 403 | `token_owner_below_editor` | The token's owner is below editor in the base |
| 403 | `token_operation_not_open` | A `write` token of an editor sends the decision route an action other than `keep` or `merge`. Nothing is written |
| 409 | `already_decided` | The item was decided before this call: a repeated call, a lost race, or a person decided it first. For a keep or a queue merge, the pair is no longer pending; for either merge, one of the entities was already merged into another; for a revert, the merge was already reverted |
| 409 | none | The governor is deciding this pair right now (keep, queue merge, or a manual merge of two entities whose own pending pair it holds); nothing was decided, so try again after its proposal |
| 429 | `agent_rate_limited` | The token has finalized 200 writes on this base in the last hour. Nothing is changed and no ledger row is written |

`already_proposed` is not a refusal: it is the `code` key of a 200 answer whose call met an open proposal. `unknown_action` and the 500-id `batch_size` of the proposal answer routes are listed under [Proposals](#proposals); a token never sees them, because those routes answer it with 401.

The checks run in a fixed order: the token's scope, then its knowledge-base list, then the owner's role, and only then the request itself (batch size, batch action, the revert body, an action that is not open) and the limit. So a `read` token whose owner is a viewer gets `token_read_only`, whatever it sent. A refusal changes nothing: the item stays as it was and no ledger or audit row is written.

**What stays closed to tokens.** A token reads every Review section, and on duplicate pairs and merges it has the writes described above: keep, merge behind the gate, revert and batch. Nothing else in Review is decided by a token. Answering a proposal — approving or declining it at the `agent-proposals` routes — is session only, for every token and for the proposal's own token. Every other route that decides a Review item — confirming, rejecting or closing a fact, resolving a conflict, answering the governor's own decisions, deciding an errata action, an ontology proposal, a mapping, a violation or a defect, an Alignment phrase or rule, running the consistency check, the nod on what `remember` recorded — is also session only and answers a token with 401. On the decision route itself, an action other than `keep` or `merge` answers `token_operation_not_open`. Reading "Awaiting your nod" gives a token no way to nod: the list shows what waits, and the nod route answers a token with 401. One Review write was open to `write` tokens before the Review tools existed and is unchanged: deciding an Alignment kind word (`POST /kbs/{kb_id}/review/alignment/kind-words/{kind_word}`), for a backend that sets up a base. A signed-in person's requests and responses on all of these routes are unchanged, apart from the added `total` on list responses.

## Try it from a shell

List the tools:

```bash
curl -s -X POST https://your-utopia/api/v1/kbs/$KB/mcp \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
```

Search, then read the whole document the best hit came from:

```bash
curl -s -X POST https://your-utopia/api/v1/kbs/$KB/mcp \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call",
       "params":{"name":"search_chunks","arguments":{"query":"Series C target"}}}'

curl -s -X POST https://your-utopia/api/v1/kbs/$KB/mcp \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call",
       "params":{"name":"get_document","arguments":{"document_id":"<id from the hit>"}}}'
```

If the document is truncated, repeat `get_document` with the same `document_id` and the `offset` from the notice (for example, `"offset":24000` after the first full page). The offset is a zero-based Unicode character position across trimmed section bodies in order; headers and separators do not count. Omitting it starts at 0. At or past the end, the tool reports no remaining text.

A document id from another base, or a made-up one, comes back as "No document with that id in this knowledge base" — the tool cannot tell you what it is not allowed to see.

## Point a client at it

Most MCP clients accept a remote server as a URL plus headers. For a client that reads a JSON config:

```json
{
  "mcpServers": {
    "utopia-general": {
      "url": "https://your-utopia/api/v1/kbs/<kb_id>/mcp",
      "headers": { "Authorization": "Bearer utp_pat_…" }
    }
  }
}
```

One entry per knowledge base you want the agent to reach. The agent sees the same base the token's owner sees, and nothing else.

## What is not here yet

- **SQL.** `query_data` (a read-only query against a mounted database) is a chat tool and is not served over MCP. It reaches production databases outside this deployment, and what a borrowed agent should be able to ask them is a question this version does not answer.
- **Streaming.** Responses are single JSON-RPC replies; there is no server-sent event channel.
