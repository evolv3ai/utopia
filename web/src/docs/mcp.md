# Agents over MCP

Utopia serves every knowledge base as a **Model Context Protocol** server. Any MCP client — Claude Desktop, Cursor, an agent framework, a script — can search a base, read a document, look up an entity and ask what changed, with the same permissions as the person whose token it carries. Reading needs nothing but a token. One tool records: `remember` asks a `write` token held by an editor, and what it records **waits for a person's nod** before it reaches the graph. Three tools reach the Review tab: two read all thirteen of its sections, and `review_keep` keeps one duplicate pair apart for a `write` token held by an editor.

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
No extra argument or protocol upgrade is needed. The three Review tools also return
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

Three tools serve the Review tab. They call the same server functions as the token routes described in the next section, so for one token a tool and its route return the same data and refuse with the same code.

| Tool | Arguments | Listed in `tools/list` for | Successful `structuredContent` |
|---|---|---|---|
| `review_list` | `section` (required string); `limit` and `offset` (optional integers, queue sections only); `page` and `per` (optional integers, `decisions` only) | Every token that reaches the base, `read` or `write`, with an owner who is at least a viewer | The body the token route of that section returns: `{"counts", "queue", "items", "total"}` for a queue section, the summary object for `overview`, `{"events", "total"}` for `decisions` |
| `review_item` | `section` (optional string, default `duplicates`); `id` (required string) | The same | That item exactly as `review_list` returns it. For section `violations`, the same object plus `proof` |
| `review_keep` | `id` (required), `rationale` (optional string) | Only a `write` token whose owner is at least an editor of the base | `{"ok": true, "outcome": "applied", "action_id": "<uuid>"}`, the body of the keep route |

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

**Agent History.** `section: "agent"` returns rows from two sources in one sequence, newest first, and `limit`, `offset` and `total` apply to the merged sequence. A governor row is the row the Review tab has always shown and has no `source` key. A row for an action made with a token has `"source": "token"` and names both the person and the token: `owner_id` and `owner_name` for the token's owner, `token_id` and `token_name` for the token (the name is the snapshot stored with the action, so a renamed or revoked token still reads as it was), beside `id`, `operation`, `target_kind`, `target_id`, `outcome`, `left`, `right`, `rationale` and `created_at`. On a base where no token has acted, the section holds the governor's rows only.

**One item.** `review_item` takes the `section` an item is listed in and the item's `id`, and returns that item with the fields `review_list` returns for it. `section` may be any of the eleven queue sections or `decisions`; left out, it is `duplicates`.

- `id` is the item's `id` from the list: a uuid. For `decisions` it is the event's `id`; for `agent` it is the row's `id`, whichever source the row has.
- An Alignment item of kind `kind_word` has no `id`. Pass its `kind_word` as `id`; it is compared exactly as stored, with the same letter case and spacing.
- A Duplicates item is the pair: `id`, `score`, `reason`, `stage`, `created_at`, `left` and `right` (each with `id`, `name`, `type_label`, `color`, `disambiguator`, `degree`, `top_facts`), and `proposal` (the governor's open proposal, or `null`).
- An Axioms item (section `violations`) carries one more key, `proof`: the proof steps when the item's `kind` is `derived_contradiction`, and `null` for any other kind, which is not an error. These are the `steps` that `GET /kbs/{kb_id}/violations/{violation_id}/proof` returns. No other section gains a key.
- An id that is not listed in that section of this base gets one answer, `not_found`, with no evidence in it: the same for an unknown id, an id of another base and an item that is no longer waiting there. `section: "overview"` lists no items, so any id with it is `not_found`.

**Who may call.** The two read tools need a token that reaches the base and an owner who is at least a viewer. `review_keep` needs the `write` scope and an owner who is at least an editor; the scope is checked before the owner's role. A caller without that right does not see `review_keep` in `tools/list`. If it calls the tool anyway, the answer is a tool error with a code, not a JSON-RPC error. A token that is unknown, revoked or expired gets HTTP 401 from the endpoint. A token whose knowledge-base list excludes the base is turned away by the endpoint itself with HTTP 403 before any tool runs, and gets no tool list, item, count or evidence. The token API answers that same token with 404 (next section); the two answers differ, and neither carries anything from the base.

**A keep made by an agent is recorded under the token's owner and the token's name**: the audit row has the owner as its actor and carries the token's id and the name the token had at that moment, and the same keep is one row in the token-agent ledger (`action_id`). The keep is applied at once, changes no entity and no fact, and is not treated as a person's decision; the paragraph on keeps in the next section applies unchanged. Each call to any of the three tools also writes the `mcp.tool_called` row that every MCP tool call writes.

**Errors.** A refused or failed call is a normal `tools/call` result with `isError: true`. Its `structuredContent` is `{"code": "<code>", "message": "<text>"}`, where `code` and `message` are the `code` and `error` of the response the matching token route gives. `content` holds the same message as text.

| Code | Tool | When |
|---|---|---|
| `unknown_queue` | `review_list`, `review_item` | `section` is not one of the thirteen keys above. `mappings`, a list queue the browser uses that is not a Review section, is refused the same way |
| `not_found` | `review_item`, `review_keep` | The id is not listed in the named section of this base, or the section is `overview` (`review_item`); no pair with that id is in this base (`review_keep`). The routes answer a missing item with 404 and no code |
| `token_read_only` | `review_keep` | The token has the `read` scope |
| `token_owner_below_editor` | `review_keep` | The token's owner is below editor in the base |
| `already_decided` | `review_keep` | The pair was decided before this call: a repeated call, a lost race, or a person decided it first |

An error for which the route has no code has no `code` field: the governor is deciding the pair right now (try again after its proposal), or the server failed. A failed keep writes nothing, and the pair stays in the Duplicates section. A call with a missing or malformed argument (no `section` for `review_list`, a `limit` that is not an integer, a `section` that is not a string, an `id` for `review_keep` that is not a uuid) is not run; it returns `isError: true` with a text message and no `structuredContent`. For `review_item`, an `id` that is not a uuid is not malformed, because a kind word is not a uuid: outside the Alignment section it is answered `not_found`.

The annotations on these tools (`readOnlyHint` on the two reads; `readOnlyHint`, `destructiveHint` and `idempotentHint`, all `false`, on `review_keep`) are hints for the client. The server does not read them, and it does not use elicitation: what a token may do is decided by the checks above.

## Review over the token API

A token can also call six of the Review tab's own HTTP routes: five reads and the keep. They are the routes the browser uses — same URLs, same handlers — so an agent reads what a signed-in editor sees. All are under `/api/v1` and take `Authorization: Bearer utp_pat_…`.

| Route | What it does | Who may call it |
|---|---|---|
| `GET /kbs/{kb_id}/review?queue=<key>` | One page of a queue section, as `{"counts", "queue", "items", "total"}`. `queue` is one of the eleven queue keys in the table under [Review over MCP](#review-over-mcp) (`pending`, `duplicates`, `conflicts`, `unconfirmed`, `lowconf`, `violations`, `defects`, `alignment`, `errata`, `agent`, `merges`); left out, it is `duplicates`. Optional `limit` (default 10, served as 1 to 200) and `offset` (default 0; a negative value is served as 0), and for `duplicates` only, `types` (`any`, `same`, `conflict`). `counts`, `total` and the order of `items` are the ones a session gets for the same request; an empty section is `"items": []` with `"total": 0`, not an error | Any token (`read` or `write`) that reaches the base, with an owner who is at least a viewer |
| `GET /kbs/{kb_id}/review/summary` | The Overview summary: what is waiting, what was decided, the base's health, the governor's recent rows | The same |
| `GET /kbs/{kb_id}/review/history?page=&per=` | One page of the Decisions history, as `{"events", "total"}`. `page` is zero-based (default 0); `per` defaults to 20 and is served as 1 to 100 | The same |
| `GET /kbs/{kb_id}/violations/{violation_id}/proof` | The proof behind one Axioms item, as `{"steps"}`: the proof steps for a violation of kind `derived_contradiction`, and `"steps": null` for any other kind or an id that is not a violation of this base | The same |
| `GET /kbs/{kb_id}/derived/{derived_id}/proof` | The proof of one derived fact, as `{"proof"}`: its premises in order, each with its evidence. `"proof": null` when the derivation is withdrawn or does not exist | The same |
| `POST /kbs/{kb_id}/review/{review_id}` with `{"action": "keep", "rationale": "…"}` | Keeps one pending duplicate pair apart. `rationale` is optional (a string or `null`). Answers `{"ok": true, "outcome": "applied", "action_id": "<uuid>"}` | A `write` token that reaches the base, with an owner who is at least an editor |

What `total` counts, why three sections are paged by it and not by `counts`, the page clamp, an offset past the end, the Alignment order and the two kinds of Agent History row are described under [Review over MCP](#review-over-mcp); they are properties of the list itself and are the same on this route. `total` is on every list response, for a signed-in session as well; it is the only key the list response gained. The token API has no route for a single item: an item's fields are the list's, and the fact evidence and entity reads a token already had, with the two proof routes above, give the rest.

A keep made with a token is applied at once and is recorded three ways in one transaction: the pair is marked kept by the token's owner through that token, a row is written to the token-agent ledger (`action_id` is its id), and an audit row is written with the owner as actor plus the token's id and the name it had at that moment. If any of the three cannot be written, the keep fails and the pair stays in the queue. A token's keep is **not** treated as a person's decision: the governor does not read it as precedent, it starts no governor round, and a governor proposal open on the pair stays open for a person to answer.

Refusals carry a stable `code` beside the `error` message where one is listed:

| Status | Code | When |
|---|---|---|
| 401 | none | The token is unknown, revoked or expired; or the route is one of the Review routes that take a signed-in session only (see below) |
| 404 | none | On the five reads: the token's knowledge-base list excludes the base, or its owner cannot open it. The answer is the same as for a base that does not exist, and carries no item, count, total, summary or proof. (The MCP endpoint answers a token whose list excludes the base with HTTP 403 instead.) On the keep: no pair with that id in this base |
| 422 | `unknown_queue` | A token asks the list for `mappings` or for any name that is not one of the eleven queue keys. No items are returned |
| 422 | `unknown_type_filter` | `types` on the Duplicates section is not `any`, `same` or `conflict` |
| 403 | `token_read_only` | A `read` token calls the keep route |
| 403 | `token_kb_out_of_scope` | A `write` token whose knowledge-base list excludes the base calls the keep route |
| 403 | `token_owner_below_editor` | The token's owner is below editor in the base |
| 403 | `token_operation_not_open` | A `write` token of an editor sends any action other than `keep` (for example `merge`) to the decision route. Nothing is written |
| 409 | `already_decided` | The pair was decided before this call: a repeated call, a lost race, or a person decided it first |
| 409 | none | The governor is deciding this pair right now; nothing was decided, so try again after its proposal |

Scope is checked before the owner's role, so a `read` token whose owner is a viewer gets `token_read_only`. A refusal changes nothing: the pair stays as it was and no ledger or audit row is written.

**Not open to tokens.** A token reads every Review section and decides nothing in them beyond the keep. Merge, manual merge, merge revert and batch decisions are not open: `merge` on the decision route answers `token_operation_not_open`, and the batch (`POST /kbs/{kb_id}/review/batch`, whatever its action — a batch of keeps included), revert (`POST /kbs/{kb_id}/merges/{merge_id}/revert`) and manual-merge (`POST /kbs/{kb_id}/entities/merge`) routes take a session only and answer a token with 401 until a later change opens them. Every other route that decides a Review item — confirming or rejecting facts, resolving a conflict, answering the governor, deciding an errata action, a violation or a defect, the nod on what `remember` recorded — is also session only. Reading "Awaiting your nod" gives a token no way to nod: the list shows what waits, and the nod route answers a token with 401. One Review write was open to `write` tokens before the Review tools existed and is unchanged: deciding an Alignment kind word (`POST /kbs/{kb_id}/review/alignment/kind-words/{kind_word}`), for a backend that sets up a base. A signed-in person's requests and responses on all of these routes are unchanged, apart from the added `total` on list responses.

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
