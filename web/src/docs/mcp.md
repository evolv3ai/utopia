# Agents over MCP

Utopia serves every knowledge base as a **Model Context Protocol** server. Any MCP client — Claude Desktop, Cursor, an agent framework, a script — can search a base, read a document, look up an entity and ask what changed, with the same permissions as the person whose token it carries. Reading needs nothing but a token. One tool records: `remember` asks a `write` token held by an editor, and what it records **waits for a person's nod** before it reaches the graph. Twenty tools reach the Review tab and the two lists beside it, the open ontology proposals and the data mappings. Four read: `review_list` and `review_item` read all thirteen sections of the Review tab, `review_ontology_list` the open ontology proposals and `review_mapping_list` the mappings. Sixteen write, for a `write` token held by an editor, each as the owner's ruled table allows its operation. Four act on duplicate pairs and merges: `review_keep` keeps one duplicate pair apart, `review_merge` merges a pair or two entities **behind the server's merge gate**, `review_revert` reverts a merge, and `review_batch` decides up to 50 duplicate pairs in one call. Nine **only propose**: `review_fact`, `review_conflict`, `review_violation`, `review_defect`, `review_errata`, `review_alignment`, `review_ontology`, `review_pending_fact` and `review_governor`. Their call changes nothing; it waits as a proposal, and the decision is made only if a signed-in editor approves it, as that person's own decision. Three **finalize** at once: `review_mapping` decides a mapping, `review_check` runs the consistency check or inference, and `review_kind_word` binds a kind word to a class. A merge the gate holds is a proposal too. No token and no tool answers a proposal.

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
| `review_fact` | **Proposes** that a person confirm, reject or close one fact of the Unconfirmed or Low confidence section. The call changes nothing. Listed and refused as `review_keep` is, as are all the write tools below |
| `review_conflict` | **Proposes** that a person resolve one temporal conflict: close the old fact, retime the new one, keep both or reject the new one. The call changes nothing |
| `review_violation` | **Proposes** that a person decide one axiom violation of the Axioms section. The call changes nothing |
| `review_defect` | **Proposes** that a person mark one ontology defect `fixed` or `accepted`. The call changes nothing |
| `review_errata` | **Proposes** that a person carry out or reject one errata action the server's gate held. The call changes nothing |
| `review_alignment` | **Proposes** that a person decide one Alignment phrase signature or implication rule. The call changes nothing and queues no job |
| `review_ontology` | **Proposes** that a person reject or adopt one open ontology proposal. The call changes nothing |
| `review_pending_fact` | **Proposes** that a person confirm or reject one remembered sentence of "Awaiting your nod". The call changes nothing: **a token never gives the nod** |
| `review_governor` | **Proposes** how a person should answer one decision of the governor. The call changes nothing and starts no governor round |
| `review_mapping` | Decide one data mapping, `confirmed` or `rejected`. **Finalizes** at once, and counts toward the limit on finalized writes |
| `review_check` | Run the consistency check or inference over the whole base. **Finalizes** at once, and counts toward the limit on finalized writes |
| `review_kind_word` | Bind one Alignment kind word to a class, or to none. **Finalizes** at once, as the kind-word route always did for a `write` token |
| `review_ontology_list` | The open ontology proposals, the list a signed-in person sees on the Ontology page. Listed for every token that reaches the base |
| `review_mapping_list` | One page of the data mappings, the list a signed-in person sees on the Data mapping page. Listed for every token that reaches the base |

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
No extra argument or protocol upgrade is needed. The twenty Review tools also return
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

Twenty tools serve the Review tab and the two lists beside it: four reads and sixteen writes. Of the writes, four act on duplicate pairs and merges, nine only propose and three finalize. They call the same server functions as the token routes described in the next section, so for one token a tool and its route return the same data and refuse with the same code.

| Tool | Arguments | Listed in `tools/list` for | Successful `structuredContent` |
|---|---|---|---|
| `review_list` | `section` (required string); `limit` and `offset` (optional integers, queue sections only); `page` and `per` (optional integers, `decisions` only) | Every token that reaches the base, `read` or `write`, with an owner who is at least a viewer | The body the token route of that section returns: `{"counts", "queue", "items", "total"}` for a queue section, the summary object for `overview`, `{"events", "total"}` for `decisions` |
| `review_item` | `section` (optional string, default `duplicates`); `id` (required string) | The same | That item exactly as `review_list` returns it. For section `violations`, the same object plus `proof` |
| `review_keep` | `id` (required), `rationale` (optional string) | Only a `write` token whose owner is at least an editor of the base | `{"ok": true, "outcome": "applied", "action_id": "<uuid>"}`, the body of the keep route |
| `review_merge` | Either `id` (the review id of a pending duplicate pair) or both `source` and `target` (two entity ids; `source` is merged into `target`); `rationale` (optional string). No argument is marked required in the schema, because "one form or the other" cannot be expressed there; the server checks it | The same as `review_keep` | The body of the matching merge route: `{"ok": true, "outcome": "applied", "action_id", "merge_id"}`, or `{"ok": true, "outcome": "held" or "proposed", "action_id", "reason", "explanation"}` when the call became a proposal |
| `review_revert` | Exactly one of `merge_id` (one merge) and `batch_id` (every merge of one batch); `rationale` (optional string). No argument is marked required | The same as `review_keep` | With `merge_id`, the body of the revert route: `{"ok": true, "outcome": "applied", "action_id", "merge_id"}` or `{"ok": true, "outcome": "proposed", "action_id", "reason", "explanation"}`. With `batch_id`, the body of the unit revert: `{"batch_id", "reverted", "outcomes"}` |
| `review_batch` | `ids` (required list of review ids), `action` (required, `merge` or `keep`), `rationale` (optional string) | The same as `review_keep` | The body of the token batch: `{"batch_id", "decided", "outcomes"}` |
| `review_fact` | `id` (required, a fact id), `action` (required: `confirm`, `reject` or `close`); for `close`, `valid_to` (required, an RFC 3339 date-time) and `valid_to_precision` (optional: `year`, `month`, `day`, `hour`, `minute` or `second`; default `day`) | The same as `review_keep` | The proposal answer: `{"ok": true, "outcome": "proposed", "action_id", "reason": "propose_only", "explanation"}`, plus `"code": "already_proposed"` when an open proposal was met. The same for every propose-only tool below |
| `review_conflict` | `id` (required, a conflict id), `action` (required: `close`, `retime_new`, `keep` or `reject_new`); optional `close_at`, `close_at_precision`, `new_from`, `new_from_precision` (the fields of the conflict route's body) | The same | The proposal answer |
| `review_violation` | `id` (required, a violation id), `resolution` (required: `fact_retracted`, `fact_closed`, `axiom_relaxed` or `accepted`); optional `fact_id`, `close_at`, `close_at_precision` | The same | The proposal answer |
| `review_defect` | `id` (required, a defect id), `resolution` (required: `fixed` or `accepted`) | The same | The proposal answer |
| `review_errata` | `id` (required, an errata action id), `approve` (required boolean: `true` proposes carrying the held action out, `false` rejecting it) | The same | The proposal answer |
| `review_alignment` | `kind` (required: `phrase` or `rule`) and `id` (required). For a phrase: `property` (optional key; left out, it proposes that no property fits), `direction` (`forward` or `reverse`, needed with `property`), `marks` (`start`, `end` or `none`, under a state property). For a rule: `approve` (required boolean) | The same | The proposal answer |
| `review_ontology` | `section` (required: `entity_types`, `relation_types`, `attribute_types` or `map_to`), `key` (required), `action` (required: `reject` or `adopt`); `reason` (optional, `reject` only); `label`, `description`, `datatype`, `domains`, `ranges`, `parents` (optional, `adopt` only) | The same | The proposal answer |
| `review_pending_fact` | `id` (required, a pending fact id from section `pending`), `action` (required: `confirm` or `reject`) | The same | The proposal answer |
| `review_governor` | `id` (required, the id of a governor row of section `agent`), `action` (required: `merge`, `keep` or `revert`), `rationale` (optional string) | The same | The proposal answer |
| `review_mapping` | `id` (required, a mapping id from `review_mapping_list`), `status` (required: `confirmed` or `rejected`) | The same | `{"ok": true}`, the body a signed-in person gets |
| `review_check` | `run` (required: `consistency` or `inference`) | The same | The report a signed-in person gets: for `consistency` `{"edges", "predicates_with_axioms", "found", "inserted", "cleared", "cycles_capped", "classes", "defects_found", "defects_new"}`, for `inference` `{"rules", "edges", "derived", "inserted", "invalidated", "capped"}` |
| `review_kind_word` | `kind_word` (required string, as the Alignment item gives it), `class` (optional class key; left out, it decides that no class fits) | The same | `{"ok": true}`, the body a signed-in person gets |
| `review_ontology_list` | none | Every token that reaches the base, `read` or `write`, with an owner who is at least a viewer | The body of `GET /kbs/{kb_id}/ontology/proposals`: `{"entity_types", "relation_types", "attribute_types", "map_to"}`, one list per section of the open proposals the ontology agent made, each item named by its section and its `key`. An item carries `agent_proposals` while a token's proposal on it is open |
| `review_mapping_list` | `status` (optional: `proposed`, `confirmed` or `rejected`), `q` (optional text the concept, source or table name must contain), `limit` (optional, default 25, served as 1 to 200), `offset` (optional, default 0) | The same as `review_ontology_list` | The body of `GET /kbs/{kb_id}/mappings`: `{"items", "total", "counts", "last_run"}` |

What each write does, every key of its answer and every reason a call becomes a proposal are described once, under [Review over the token API](#review-over-the-token-api): a tool and its route run the same server function, so the answers are the same object. `review_fact` stands for the three fact routes, `review_alignment` for the phrase and rule routes, `review_ontology` for the reject and adopt routes, and `review_check` for the consistency and inference routes.

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

A paging argument the section does not take is ignored: `overview` reads none, `decisions` does not read `limit` or `offset`, and a queue section does not read `page` or `per`. The route's `types` filter for Duplicates is not a tool argument; `review_list` reads the Duplicates section unfiltered. Any other section name, `mappings` included, is refused with `unknown_queue` and returns no items; read the mappings with `review_mapping_list` and the open ontology proposals with `review_ontology_list`. Reading is all these sections offer: nothing listed is decided by reading it.

**Totals and paging.** A queue section's result carries `total`: the number of rows that section's list returns over all pages, so the last item is at `offset = total - 1`. `counts` holds the Review tab's badges, with the values they always had, and for three sections the badge named after the section is not the number of listed rows: `counts.alignment` leaves out proposed implication rules, `counts.lowconf` leaves out derived facts that the list includes, and `counts.agent` counts the governor's open proposals only. For Alignment, Low confidence and Agent History, page by `total`, not by `counts`. An empty section is `"items": []` with `"total": 0`, not an error. `counts` has one more key, `agent_token_proposals`: the number of open token proposals of the base (outcome `held` or `proposed`, not yet answered). It is present only while that number is above zero, so the counts of a base with no open token proposal have the keys they always had. The Agent rail badge of the Review tab shows `counts.agent` plus `counts.agent_token_proposals`.

**Page size.** For a queue section, `limit` defaults to 10 and is served as 1 to 200: a value below 1 returns one item and a value above 200 returns 200. An out-of-range `limit` is not an error. `offset` defaults to 0, and a negative value is served as 0. An `offset` at or past `total` returns no items and the true `total`. The Alignment list is ordered by each item's `decided_at` and then by the item's own identifier (the `id` of a phrase or rule, the word of a kind word), so paging by `limit` and `offset` over an unchanged queue visits each waiting item exactly once.

**Decisions.** `section: "decisions"` pages with the history route's own `page` and `per`, not with `limit` and `offset`. `page` is zero-based and defaults to 0 (a negative value is served as 0); `per` defaults to 20 and is served as 1 to 100. The result is `{"events", "total"}`: one page of decision events, newest first, each with `id`, `action`, `target_kind`, `target_id`, `detail`, `actor_id`, `actor_name` and `created_at`, and the number of events in the history. A page past the end has no events and the same `total`.

**Agent History.** `section: "agent"` returns rows from two sources in one sequence, newest first, and `limit`, `offset` and `total` apply to the merged sequence. A governor row is the row the Review tab has always shown and has no `source` key. A row for an action made with a token has `"source": "token"` and names both the person and the token: `owner_id` and `owner_name` for the token's owner, `token_id` and `token_name` for the token (the name is the snapshot stored with the action, so a renamed or revoked token still reads as it was), beside `id`, `operation`, `target_kind`, `target_id`, `outcome`, `left`, `right`, `rationale` and `created_at`. `operation` is one of:

- on duplicate pairs and merges: `review.keep`, `review.merge` (a queue merge), `merge.manual` (a merge of two entities named by id) and `merge.revert`;
- propose only: `fact.confirm`, `fact.reject` and `fact.close` (target kind `fact`), `conflict.resolve` (`conflict`), `violation.decide` (`axiom_violation`), `defect.decide` (`ontology_defect`), `errata.decide` (`errata_action`), `alignment.phrase` (`phrase_binding`), `alignment.rule` (`implication_rule`), `ontology.proposal` (`ontology_proposal`), `pending.nod` (`pending_fact`) and `governor.answer` (`agent_decision`);
- finalized: `mapping.decide` (`concept_mapping`), `consistency.check` and `inference.run` (`knowledge_base`, the base itself) and `alignment.kind_word` (`type_binding`).

`outcome` is `applied`, `held` (the merge gate held the call) or `proposed` (the call may only propose). `left` and `right` name the two sides of a pair and are `null` on a row whose target is not a pair. Eight more keys are present only when they have a value, so the row of a single keep has none of them: `reason` (why a `held` or `proposed` call was not applied; `propose_only` for every propose-only operation; the codes are listed in the next section), `batch_id` (the call was a member of that batch, or a revert made by the unit revert of that batch), `merge_id` (on an applied merge the merge it made, on a `merge.revert` row the merge it targets, on an approved merge proposal the merge the person's approval made), `state` with `state_at`, `target_label` (words naming a target that is not a pair: the fact, the conflict, the word, the base), `action` (the action the call asked for, such as `confirm`, `keep` or `bind`, when the operation has one) and `queued` (`true` on an approved `alignment.phrase` or `alignment.rule` row: the person's decision is recorded and its change to the typed graph is **queued** as a job, not yet applied). A row with outcome `held` or `proposed` and no `state` is an open proposal. `state` is `approved`, `declined` or `superseded` once a person answered the proposal, and `undone` on an applied merge row whose merge has since been reverted, by anyone; `state_at` is the time of the answer or of the revert. `rationale` is the token's own rationale (the `rationale` of a governor proposal, the `reason` of an ontology rejection), kept on the proposal and never recorded as the reason of the person who answers it. On a base where no token has acted, the section holds the governor's rows only.

**One item.** `review_item` takes the `section` an item is listed in and the item's `id`, and returns that item with the fields `review_list` returns for it. `section` may be any of the eleven queue sections or `decisions`; left out, it is `duplicates`.

- `id` is the item's `id` from the list: a uuid. For `decisions` it is the event's `id`; for `agent` it is the row's `id`, whichever source the row has.
- An Alignment item of kind `kind_word` has no `id`. Pass its `kind_word` as `id`; it is compared exactly as stored, with the same letter case and spacing.
- A Duplicates item is the pair: `id`, `score`, `reason`, `stage`, `created_at`, `left` and `right` (each with `id`, `name`, `type_label`, `color`, `disambiguator`, `degree`, `top_facts`), and `proposal` (the governor's open proposal, or `null`). When a token agent's proposal is open on the pair, the item also has `agent_proposal`: `{"id", "operation", "reason", "owner_name", "token_name", "created_at"}`. That is an open queue-merge proposal for this review id or, when there is none, an open manual-merge proposal naming the pair's two entities in either direction. The key is absent when no such proposal is open; it is never `null`.
- A Merges item has `id` (the merge id that `review_revert` takes), `source_name`, `target_name`, `merged_by_name`, `reason`, `created_at` and `reverted_at`. A merge that a token agent made also has `agent`: `{"action_id", "owner_name", "token_name"}`, where `action_id` is the ledger row of the merge; `merged_by_name` on such a row is the token's owner, so read `agent` to tell an agent's merge from a merge the owner made by hand. A merge made by a person approving a proposal is that person's merge and has no `agent`. A merge with an open revert proposal has `agent_proposal`, the same object as on a Duplicates item. Both keys are absent when they have no value.
- An item of Awaiting your nod, Conflicts, Unconfirmed, Low confidence, Axioms, Ontology (`defects`) or Errata, a phrase or rule item of Alignment, and a governor row of Agent History (a row with no `source` key) carry `agent_proposals` while at least one token proposal on them is open: a list, oldest first, of `{"id", "operation", "reason", "owner_name", "token_name", "created_at"}`, plus `"action"` when the proposal has one. `id` is the proposal, the id the answer routes take. The key is absent when no proposal is open, so such an item has exactly the keys it had before; it is never `null` or `[]`. A kind-word item, a token row of Agent History and a mapping carry no proposal. The same key is on the proposals `review_ontology_list` returns. All readers see it alike: a signed-in person, a token and `review_list`.
- An Axioms item (section `violations`) carries one more key, `proof`: the proof steps when the item's `kind` is `derived_contradiction`, and `null` for any other kind, which is not an error. These are the `steps` that `GET /kbs/{kb_id}/violations/{violation_id}/proof` returns. `review_item` adds no key to an item of any other section.
- An id that is not listed in that section of this base gets one answer, `not_found`, with no evidence in it: the same for an unknown id, an id of another base and an item that is no longer waiting there. `section: "overview"` lists no items, so any id with it is `not_found`.

**Who may call.** The four read tools (`review_list`, `review_item`, `review_ontology_list`, `review_mapping_list`) need a token that reaches the base and an owner who is at least a viewer. The sixteen write tools need the `write` scope and an owner who is at least an editor; the scope is checked before the owner's role. A caller without that right does not see any of them in `tools/list`. If it calls one anyway, the answer is a tool error with a code (`token_read_only` or `token_owner_below_editor`), not a JSON-RPC error. That holds for `review_kind_word` too, although its REST route answers a below-editor owner with a plain 403 and no code. No tool answers a proposal, the token's own included: no listed tool name contains `proposal`, `approve`, `decline` or `answer`, a call to any other name beginning `review_` (such as `review_approve` or `review_confirm`) is JSON-RPC error -32601, and the proposal stays open. A token that is unknown, revoked or expired gets HTTP 401 from the endpoint. A token whose knowledge-base list excludes the base is turned away by the endpoint itself with HTTP 403 before any tool runs, and gets no tool list, item, count or evidence. The token API answers that same token with 404 (next section); the two answers differ, and neither carries anything from the base.

**A keep made by an agent is recorded under the token's owner and the token's name**: the audit row has the owner as its actor and carries the token's id and the name the token had at that moment, and the same keep is one row in the token-agent ledger (`action_id`). The keep is applied at once, changes no entity and no fact, and is not treated as a person's decision; the paragraph on keeps in the next section applies unchanged. Merges, reverts, proposals and finalized decisions made through the tools are recorded the same way, as the next section describes for the routes. Each call to any of the twenty tools, a refused one included, also writes the `mcp.tool_called` audit row that every MCP tool call writes, with the owner as actor and the tool's name in its detail; a call that changes nothing therefore still leaves that row in Activity.

**Errors.** A refused or failed call is a normal `tools/call` result with `isError: true`. Its `structuredContent` is `{"code": "<code>", "message": "<text>"}`, where `code` and `message` are the `code` and `error` of the response the matching token route gives. `content` holds the same message as text.

| Code | Tool | When |
|---|---|---|
| `unknown_queue` | `review_list`, `review_item` | `section` is not one of the thirteen keys above. `mappings`, a list queue the browser uses that is not a Review section, is refused the same way |
| `not_found` | `review_item` and every write tool except `review_check` | The id is not listed in the named section of this base, or the section is `overview` (`review_item`); no pair with that id is in this base (`review_keep`, `review_merge` with `id`); an entity id is unknown or of another base (`review_merge` with `source` and `target`); the merge id is unknown or of another base, or no token action of this base carries the batch id (`review_revert`); the target of a propose-only tool or of `review_mapping` is unknown or of another base, or is gone as that tool's route describes it (next section). The routes answer these with 404 and no code |
| `token_read_only` | every write tool | The token has the `read` scope |
| `token_owner_below_editor` | every write tool | The token's owner is below editor in the base |
| `already_decided` | every write tool except `review_check` and `review_kind_word` | The pair was decided before this call, or one of the two entities was already merged into another (`review_keep`, `review_merge`); the merge was already reverted, by anyone (`review_revert`); the target of a propose-only tool was already decided (next section); the mapping already has the status asked for (`review_mapping`) |
| `self_merge` | `review_merge` | `source` and `target` are the same entity |
| `batch_size` | `review_batch` | `ids` is empty or has more than 50 entries. The whole batch is refused and nothing is changed |
| `agent_rate_limited` | `review_keep`, `review_merge`, `review_revert`, `review_batch`, `review_mapping`, `review_check` | The token has finalized 200 writes on this base in the last hour. Nothing is changed. A propose-only tool and `review_kind_word` never get it |
| `unknown_action` | `review_fact`, `review_ontology`, `review_pending_fact` | `action` is not one the tool takes |
| `bad_resolution` | `review_defect`, `review_violation` | `resolution` is not one the tool takes, or `fact_closed` names a violation that is not a `derived_contradiction` |
| `bad_precision`, `close_at_required`, `new_from_required`, `not_a_state`, `interval_inverted`, `fact_required` | `review_fact`, `review_conflict`, `review_violation` | The body would be refused by the person's own route with that code (next section) |
| `unknown_property`, `direction_required`, `unknown_marks`, `marks_needs_state` | `review_alignment` | The phrase body would be refused by the person's own route with that code |
| `unknown_class` | `review_ontology`, `review_kind_word` | A class key the ontology does not have |
| `agent_answer`, `not_a_merge` | `review_governor` | The action does not fit the governor decision's status; a revert of a decision that made no merge |
| `bad_status` | `review_mapping`, `review_mapping_list` | A status other than `confirmed` or `rejected` (`review_mapping`), or other than `proposed`, `confirmed` or `rejected` (`review_mapping_list`) |
| `inference_off` | `review_check` | `run` is `inference` and the base's materialized-inference setting is off. Nothing runs |
| `unknown_run` | `review_check` | `run` is neither `consistency` nor `inference`. This code has no REST counterpart: each run has its own route |
| `alignment_busy` | `review_kind_word` | Another operation is updating the kind word's binding; nothing was decided, and the call can be repeated shortly |

In the result of `review_batch`, and of `review_revert` with `batch_id`, a refusal of one member is not a tool error: it is that member's `code` and `error` in `outcomes`, and the call itself succeeds. `already_proposed` is not an error either: it is the `code` key of a successful `held` or `proposed` answer that met an open proposal.

An error for which the route has no code has `structuredContent` `{"message": "<text>"}` with no `code` key: the governor is deciding the pair right now (nothing was decided; try again after its proposal), `review_batch` was called with an `action` other than `merge` or `keep` (the whole batch is refused), `review_conflict` was called with an action other than the four (the conflict route answers it with an uncoded 422), or the server failed. A refused call writes no ledger row and changes nothing. A missing or wrongly typed argument of the newer write tools is malformed in the same way: an `id` that is not a uuid, `review_fact` with `close` and no `valid_to`, `review_errata` or a rule of `review_alignment` without a boolean `approve`, a `kind` of `review_alignment` other than `phrase` or `rule`. A call with a missing or malformed argument is not run; it returns `isError: true` with a text message and no `structuredContent`. That covers: no `section` for `review_list`, a `limit` that is not an integer, a `section` that is not a string, an id argument of a write tool that is not a uuid (one entry of `ids` is enough to stop the whole batch), `review_merge` called with both forms, half of the `source` and `target` form or neither form, and `review_revert` called with both `merge_id` and `batch_id` or with neither. For `review_item`, an `id` that is not a uuid is not malformed, because a kind word is not a uuid: outside the Alignment section it is answered `not_found`.

The annotations on these tools (`readOnlyHint: true` on the four reads; `readOnlyHint`, `destructiveHint` and `idempotentHint`, all `false`, on `review_keep` and on the nine propose-only tools; `readOnlyHint: false`, `destructiveHint: true` and `idempotentHint: false` on `review_merge`, `review_revert`, `review_batch`, `review_mapping`, `review_check` and `review_kind_word`) are hints for the client. The server does not read them, and it does not use elicitation: what a token may do is decided by the checks above.

## Review over the token API

A token can also call the HTTP routes behind the Review tab, the entity panel's "Merge into…", the Ontology page's proposals and the Data mapping page: seven reads and twenty-two writes, each listed below with its ruling. All but three are the routes the browser uses — same URLs, same handlers — so an agent reads what a signed-in editor sees. The three others are new and have no button: the unit revert of a batch, and the two routes through which a token proposes an answer to a fact awaiting a nod or to a governor decision. On every route the browser uses, a signed-in person's request runs the code it ran before and gets the answer it got before: the gate, the proposals, the limit and the breaker described below apply to token callers only. All are under `/api/v1` and take `Authorization: Bearer utp_pat_…`.

Each write is ruled one of three ways, by a table compiled into the server:

- **Finalizes**: the call is applied at once and recorded as the token's action (keep, mapping decision, consistency check, inference run, kind-word decision).
- **Finalizes behind the gate**: the server asks its merge gate first; what the gate passes is applied, what it holds becomes a proposal (merge, manual merge, a revert of a token agent's merge, the batch).
- **Proposes**: the call changes nothing — no fact, entity, evidence, review row, ontology row or mapping is touched, no job is queued, and retrieval and reasoning see what they saw before. It is stored as a proposal, and the decision is made only when a signed-in editor approves it, by that person's own code and as that person's decision (every other Review decision; see [Propose-only operations](#propose-only-operations)).

| Route | What it does | Who may call it |
|---|---|---|
| `GET /kbs/{kb_id}/review?queue=<key>` | One page of a queue section, as `{"counts", "queue", "items", "total"}`. `queue` is one of the eleven queue keys in the table under [Review over MCP](#review-over-mcp) (`pending`, `duplicates`, `conflicts`, `unconfirmed`, `lowconf`, `violations`, `defects`, `alignment`, `errata`, `agent`, `merges`); left out, it is `duplicates`. Optional `limit` (default 10, served as 1 to 200) and `offset` (default 0; a negative value is served as 0), and for `duplicates` only, `types` (`any`, `same`, `conflict`). `counts`, `total` and the order of `items` are the ones a session gets for the same request; an empty section is `"items": []` with `"total": 0`, not an error | Any token (`read` or `write`) that reaches the base, with an owner who is at least a viewer |
| `GET /kbs/{kb_id}/review/summary` | The Overview summary: what is waiting, what was decided, the base's health, the governor's recent rows | The same |
| `GET /kbs/{kb_id}/review/history?page=&per=` | One page of the Decisions history, as `{"events", "total"}`. `page` is zero-based (default 0); `per` defaults to 20 and is served as 1 to 100 | The same |
| `GET /kbs/{kb_id}/violations/{violation_id}/proof` | The proof behind one Axioms item, as `{"steps"}`: the proof steps for a violation of kind `derived_contradiction`, and `"steps": null` for any other kind or an id that is not a violation of this base | The same |
| `GET /kbs/{kb_id}/derived/{derived_id}/proof` | The proof of one derived fact, as `{"proof"}`: its premises in order, each with its evidence. `"proof": null` when the derivation is withdrawn or does not exist | The same |
| `GET /kbs/{kb_id}/ontology/proposals` | The open ontology proposals, as `{"entity_types", "relation_types", "attribute_types", "map_to"}`: the body a signed-in person gets, plus `agent_proposals` on a proposal while a token's proposal on it is open. These are not a Review section; an item is named by its section and its `key` | The same |
| `GET /kbs/{kb_id}/mappings?status=&q=&limit=&offset=` | One page of the data mappings, as `{"items", "total", "counts", "last_run"}`, the body a signed-in person gets. `status` keeps `proposed`, `confirmed` or `rejected` mappings (another value is 422 `bad_status`); `limit` defaults to 25 and is served as 1 to 200. The Review list still refuses `queue=mappings` with `unknown_queue`; this is the route a token reads mappings on | The same |
| `POST /kbs/{kb_id}/review/{review_id}` with `{"action": "keep", "rationale": "…"}` | **Finalizes.** Keeps one pending duplicate pair apart. `rationale` is optional (a string or `null`). Answers `{"ok": true, "outcome": "applied", "action_id": "<uuid>"}` | A `write` token that reaches the base, with an owner who is at least an editor |
| `POST /kbs/{kb_id}/review/{review_id}` with `{"action": "merge", "rationale": "…"}` | **Finalizes behind the gate.** The queue merge: merges one pending duplicate pair behind the merge gate, in the direction a person's queue merge takes for that pair. See [Merging](#merging) | The same |
| `POST /kbs/{kb_id}/entities/merge` with `{"source": "<entity id>", "target": "<entity id>", "rationale": "…"}` | **Finalizes behind the gate.** The manual merge: merges `source` into `target` behind the same gate | The same |
| `POST /kbs/{kb_id}/merges/{merge_id}/revert` with no body or `{"rationale": "…"}` | **Finalizes behind the gate.** Reverts one merge a token agent made, or proposes the revert of any other merge. See [Reverting a merge](#reverting-a-merge) | The same |
| `POST /kbs/{kb_id}/review/batch` with `{"ids": ["<review id>", …], "action": "merge" or "keep", "rationale": "…"}` | **Finalizes behind the gate**, member by member. Decides 1 to 50 pending duplicate pairs under one `batch_id`. See [Batches](#batches) | The same |
| `POST /kbs/{kb_id}/review/agent-batches/{batch_id}/revert` with no body or `{"rationale": "…"}` | The unit revert: reverts every merge that batch made | The same, and also a signed-in editor of the base |
| `POST /kbs/{kb_id}/facts/{fact_id}/confirm`, `…/reject` (no body) and `…/close` with `{"valid_to": "<RFC 3339>", "valid_to_precision": "day"}` | **Proposes** that a person confirm, reject or close one fact. See [Propose-only operations](#propose-only-operations) | A `write` token that reaches the base, with an owner who is at least an editor |
| `POST /kbs/{kb_id}/conflicts/{conflict_id}` with `{"action": "close" or "retime_new" or "keep" or "reject_new", "close_at", "close_at_precision", "new_from", "new_from_precision"}` | **Proposes** that a person resolve one temporal conflict | The same |
| `POST /kbs/{kb_id}/review/violations/{violation_id}` with `{"resolution": "fact_retracted" or "fact_closed" or "axiom_relaxed" or "accepted", "fact_id", "close_at", "close_at_precision"}` | **Proposes** that a person decide one axiom violation | The same |
| `POST /kbs/{kb_id}/review/defects/{defect_id}` with `{"resolution": "fixed" or "accepted"}` | **Proposes** that a person resolve one ontology defect | The same |
| `POST /kbs/{kb_id}/review/errata/{action_id}` with `{"approve": true or false}` | **Proposes** that a person carry out (`true`) or reject (`false`) one held errata action | The same |
| `POST /kbs/{kb_id}/review/alignment/phrases/{binding_id}` with `{"property", "direction", "marks"}` | **Proposes** that a person bind one phrase signature to a property, or to none. A token gets 200 with the proposal; a signed-in person still gets 202 with a `job_id` | The same |
| `POST /kbs/{kb_id}/review/alignment/rules/{rule_id}` with `{"approve": true or false}` | **Proposes** that a person approve or reject one implication rule. 200 for a token, 202 for a signed-in person, as above | The same |
| `POST /kbs/{kb_id}/ontology/proposals` with `{"section", "key", "status": "rejected", "reason"}` | **Proposes** that a person reject one open ontology proposal | The same |
| `POST /kbs/{kb_id}/ontology/proposals/adopt` with `{"section", "key", "label", "description", "datatype", "domains", "ranges", "parents"}` | **Proposes** that a person adopt one open ontology proposal, with the fields given replacing the proposed ones | The same |
| `POST /kbs/{kb_id}/review/pending/{pending_id}/propose` with `{"action": "confirm" or "reject"}` | **Proposes** that a person confirm or reject one remembered sentence awaiting a nod. Token only; see [The two token-only routes](#the-two-token-only-routes) | Only such a token: a signed-in person gets 403 `token_required` |
| `POST /kbs/{kb_id}/review/agent/{decision_id}/propose` with `{"action": "merge" or "keep" or "revert", "rationale": "…"}` | **Proposes** how a person should answer one governor decision. Token only, as above | The same as the row above |
| `POST /kbs/{kb_id}/review/mappings/{mapping_id}` with `{"status": "confirmed" or "rejected"}` | **Finalizes.** Decides one data mapping. See [Finalized operations](#finalized-operations) | A `write` token that reaches the base, with an owner who is at least an editor |
| `POST /kbs/{kb_id}/consistency/check` (no body) | **Finalizes.** Runs the consistency check over the base | The same |
| `POST /kbs/{kb_id}/inference/run` (no body) | **Finalizes.** Runs inference over the base | The same |
| `POST /kbs/{kb_id}/review/alignment/kind-words/{kind_word}` with `{"class": "<class key>"}` (no class, or `null`: no class fits) | **Finalizes.** Binds one kind word to a class. Open to `write` tokens before the Review tools existed, and not narrowed | The same, except that a below-editor owner gets a plain 403 with no code, as before |

`rationale` is optional on the duplicate and merge writes and on the governor proposal (a string or `null`).

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

### Propose-only operations

The owner's ruled table makes every other Review decision **propose only** for a token: confirming, rejecting or closing a fact, resolving a conflict, deciding an axiom violation or an ontology defect, answering a held errata action, deciding an Alignment phrase or rule, rejecting or adopting an ontology proposal, answering a fact that awaits a nod, and answering the governor. A token's call to one of these routes **changes nothing**: no fact, entity, evidence row, review row, ontology row or the item itself is touched, no job is queued, and retrieval and reasoning see what they saw before. The call is stored as a proposal, and the decision is made only if a signed-in editor approves it. The approval runs that person's own code for the decision, and the decision is that person's. The answer is HTTP 200:

```json
{"ok": true, "outcome": "proposed", "action_id": "<uuid>", "reason": "propose_only", "explanation": "Not applied: a token only proposes this decision; a person makes it. Nothing was changed; the call waits as a proposal that a signed-in editor answers in the Review tab."}
```

`action_id` is the proposal: a row of the token-agent ledger with outcome `proposed`, reason `propose_only` and the full request, written with an `agent.proposal` audit row under the token's owner and the token's name. `explanation` may be reworded. A repeat, or a second call arriving at the same moment, writes nothing and returns the open proposal with `"code": "already_proposed"` (see [Proposals](#proposals)).

| Route | Ledger operation; action codes | Refused at propose time with |
|---|---|---|
| `…/facts/{fact_id}/confirm` | `fact.confirm`; `confirm` | 404 for a fact that is unknown, of another base or retracted |
| `…/facts/{fact_id}/reject` | `fact.reject`; `reject` | 404 for an unknown fact; 409 `already_decided` for a retracted one |
| `…/facts/{fact_id}/close` | `fact.close`; `close` | 422 `bad_precision`; 404 for an unknown fact or a live fact that already has an end; 409 `already_decided` for a retracted one; 422 `interval_inverted` when `valid_to` is before the fact's start |
| `…/conflicts/{conflict_id}` | `conflict.resolve`; `close`, `retime_new`, `keep`, `reject_new` | 422 `bad_precision`; an unknown action, 422 with no code; 422 `new_from_required`; 404; 409 `already_decided` when the conflict is not open or one of its facts was retracted; 422 `close_at_required` (close, no `close_at`, and the new fact has no start), `interval_inverted`, `not_a_state` (retime of a fact that is not a state) |
| `…/review/violations/{violation_id}` | `violation.decide`; `fact_retracted`, `fact_closed`, `axiom_relaxed`, `accepted` | 422 `bad_resolution`; 404; 409 `already_decided` when the violation is decided or the fact the request names was retracted or closed; 422 `fact_required`, `close_at_required`, `bad_precision`, `interval_inverted`, and `bad_resolution` for `fact_closed` on a violation that is not a `derived_contradiction` |
| `…/review/defects/{defect_id}` | `defect.decide`; `fixed`, `accepted` | 422 `bad_resolution`; 404; 409 `already_decided` when the defect is not open |
| `…/review/errata/{action_id}` | `errata.decide`; `approve`, `reject` | 404; 409 `already_decided` when the action is no longer held |
| `…/review/alignment/phrases/{binding_id}` | `alignment.phrase`; `bind`, `none` | 404; then 422 `unknown_property`, `direction_required`, `unknown_marks`, `marks_needs_state` |
| `…/review/alignment/rules/{rule_id}` | `alignment.rule`; `approve`, `reject` | 404 |
| `…/ontology/proposals` | `ontology.proposal`; `reject` | 422 `adoption_endpoint` (status `adopted`) or `bad_status` (any other status but `rejected`); 404 when no open proposal of the ontology agent has that section and key; 409 `already_decided` when it was adopted or rejected |
| `…/ontology/proposals/adopt` | `ontology.proposal`; `adopt` | 404 and 409 as above; 422 `unknown_class` for a class named in `parents`, `domains` or `ranges` that the ontology does not have |
| `…/review/pending/{pending_id}/propose` | `pending.nod`; `confirm`, `reject` | See [The two token-only routes](#the-two-token-only-routes) |
| `…/review/agent/{decision_id}/propose` | `governor.answer`; `merge`, `keep`, `revert` | The same |

- **The body is checked when it is proposed**, with the codes a signed-in person's route gives for the same body, so a proposal that lands is one the person's code would take. An invalid body lands nothing. A body that does not have the route's JSON shape at all is answered by the request reader before any check, for every caller, with no code (the two token-only routes answer it with 422 `invalid_body`, after the scope checks).
- **The target.** An id that is unknown, or of another base, is 404 and lands nothing; a target that is already decided is 409 `already_decided` and lands nothing. An Alignment phrase or rule has no decided state: it can be proposed on, and is applied on approval, while it exists. A confirm of a live fact has no decided state either.
- **The target key.** There is one open proposal per base, operation and target, so confirm, reject and close of one fact are three proposals that may all be open, while an ontology proposal carries one open proposal whichever action it asks for. The target of an ontology proposal is its stored row, named in the ledger by section and key joined by `:`; the target of every other propose-only operation is the item's own id.
- **Not counted.** A propose-only call does not count toward the limit on finalized writes and is never answered `agent_rate_limited`; the breaker does not apply to it.
- **The token's words stay the token's.** The governor proposal's `rationale` and an ontology rejection's `reason` are kept on the proposal and shown with it; they are never recorded as the reason of the person who approves it.
- **Ontology proposals are not a Review section.** Read them with `GET /kbs/{kb_id}/ontology/proposals` (or `review_ontology_list`) and name one by its section and `key`. Only an open proposal the ontology agent made can be proposed on.

### The two token-only routes

A token proposes an answer to a fact awaiting a nod, and an answer to a governor decision, through two routes of its own, not through the person's routes. The person's nod route (`POST /kbs/{kb_id}/review/pending/{pending_id}`), the read of one sentence's pending facts (`GET /kbs/{kb_id}/review/pending`) and the governor's own decision route (`POST /kbs/{kb_id}/review/agent/{decision_id}`) are unchanged and answer every token with 401. The nod stays with people: no route, tool or parameter lets a token confirm or reject a pending fact, and the governor's decision route stays a person's answer.

- `POST /kbs/{kb_id}/review/pending/{pending_id}/propose` with `{"action": "confirm"}` or `{"action": "reject"}` proposes that a person confirm or reject one item of "Awaiting your nod". The `pending_facts` row stays byte for byte as it was; no fact is written and none is rejected. An action other than `confirm` or `reject` is 422 `unknown_action`; an id that is unknown, of another base, or of a sentence a person already nodded or rejected is 404. A pending fact carries one open proposal, whichever action.
- `POST /kbs/{kb_id}/review/agent/{decision_id}/propose` with `{"action": "merge" or "keep" or "revert", "rationale": "…"}` proposes how a person should answer one governor row of Agent History: `merge` or `keep` on a suggestion that is still `proposed`, `revert` on a merge the governor applied, or `merge` on a pair the governor kept apart. The governor's decision row, the pair and the governance fuse stay as they are, and no governor round starts. A decision a person already settled, or a `proposed` suggestion whose pair no longer waits, is 409 `already_decided`; then an action that does not fit the decision's status is 422 `agent_answer`, and a revert of a decision that made no merge is 422 `not_a_merge`. An unknown or foreign id is 404.
- Both take a personal token only: a signed-in session gets 403 `token_required` and nothing happens. The body is read after the scope, base-list and role checks, so a refused token hears why it cannot write before it hears about its body: a body that is not a JSON object of the route's shape is then 422 `invalid_body`.
- Both are MCP tools: `review_pending_fact` and `review_governor`.

### Proposals

There is one open proposal per base, operation and target. The target of a queue merge is its review id; the target of a manual merge is the unordered pair of entity ids, so `A` into `B` and `B` into `A` are the same proposal and it keeps the direction of the first call; the target of a revert is the merge id; the targets of the propose-only operations are given above. A repeated call, or a second call arriving at the same moment, writes nothing and returns the open proposal: the same 200 answer with the existing `action_id` and one more key, `"code": "already_proposed"`.

An open proposal is read as a row of Agent History (outcome `held` or `proposed`, no `state`), as `agent_proposal` on the Duplicates item or Merges item it targets, and as an entry of `agent_proposals` on the item of any other section it targets (on an ontology proposal: in the body of `GET /kbs/{kb_id}/ontology/proposals`); all are described under [Review over MCP](#review-over-mcp). `counts.agent_token_proposals` counts the open ones.

**Only a signed-in person answers a proposal.** The two answer routes take a browser session and nothing else. Every personal token gets 401 from them, the token that made the proposal included, and no MCP tool answers one. In the browser, a signed-in editor sees a token's proposal on its item's card in its own section (an ontology proposal on its card on the Ontology page) and in the Agent History section of the Review tab, where they approve or decline it, one at a time or by selecting several for one bulk answer.

| Route | Body | Answer |
|---|---|---|
| `POST /kbs/{kb_id}/review/agent-proposals/{proposal_id}` | `{"action": "approve"}` or `{"action": "decline"}` | `{"ok": true, "state": "approved"}`, with `state` one of `approved`, `declined`, `superseded`, plus, each only when set: `"merge_id"` when a merge was made or reverted, `"job_id"` with `"queued": true` for an approved phrase or rule proposal, `"fact_id"` for an approved confirm of a pending fact |
| `POST /kbs/{kb_id}/review/agent-proposals` | `{"ids": ["<proposal id>", …], "action": "approve" or "decline"}`, 1 to 500 ids, any mix of operations | `{"answered": n, "outcomes": [{"id", "state", "merge_id", "code", "error"}]}`, one outcome per id in request order, all five keys always present, plus `job_id`, `queued` and `fact_id` only when set |

- The caller must be an editor of the base; a signed-in person below editor gets 403.
- **Approve** runs the person's own code for the operation — merge, revert, confirm, reject, close, conflict resolution, violation, defect, errata answer, phrase or rule decision, ontology rejection or adoption, nod, answer to the governor — with that person as actor (and, for a merge, no gate), exactly as if they had done it themselves. The audit row of that decision carries `agent_proposal`, the proposal's id, and no `token_id`; the token's rationale is not passed as the person's reason. It counts as that person's decision everywhere: the governor reads an approved merge as human precedent, and the governance fuse counts an approved revert as that person's revert.
- Particular approvals. A pending-fact proposal is recorded as the person's own nod: audit row `fact.nod_confirmed` (the answer carries `fact_id`, the fact written) or `fact.nod_rejected`. A governor proposal runs the person's answer to the governor with no rationale. An ontology rejection writes one audit row `ontology_proposal.rejected` with the person as actor and `agent_proposal` in its detail (a person's own rejection on that route still writes no audit row). An ontology adoption runs the adoption as the person, and its `ontology_proposal.adopted` row carries `agent_proposal`.
- **An approved phrase or rule is queued, not applied.** The person's decision is recorded and its change to the typed graph is queued as a job: the answer carries `job_id` and `"queued": true`, and the proposal's Agent History row reads `queued`.
- Approving a proposal whose target can no longer be decided closes it as `superseded` and applies nothing. The approval reads the target first: the pair was decided meanwhile, one of the entities was merged into another, the pair now names other entities, the merge was already reverted, the item was decided or is gone, the conflict now names other facts, or the ontology agent rewrote the ontology proposal. A close that turns out to change nothing, and a lost race inside a confirm, reject, defect, conflict or adoption, also close it `superseded`.
- If the person's own code refuses the approval — a date its rules refuse, an adoption that answers `alignment_busy` — the answer is that refusal with the person's code, and the proposal stays open. A violation decision and an errata answer are not one transaction: an error after a step has committed is returned, the proposal stays open, and it is never reported as `superseded`.
- **Decline** closes the proposal and changes nothing else; a pending fact keeps waiting for a nod. A declined or superseded proposal writes an `agent.proposal.declined` or `agent.proposal.superseded` audit row with the person as actor, the proposal's operation and the token's name, and `left` and `right` only when the proposal names a pair.
- A proposal that was already answered gets 409 `already_decided`; an id that is unknown, of another base or not a proposal gets 404. In the bulk answer these are that item's `code` (`already_decided`, `not_found`) with `state` null, and the other items are still answered.
- An `action` other than `approve` or `decline` is 422 `unknown_action`; an empty list or more than 500 ids is 422 `batch_size`. Both refuse the whole request and answer nothing.
- While a running governor round holds the pair, approving the queue-merge proposal on it answers 409 with no code (in the bulk answer: `code` null with the message in `error`). Nothing was decided and the proposal stays open; answer it again later.
- Approving a manual-merge proposal leaves the two entities' own pending duplicate pair open, as a person's manual merge does. (A token's own applied manual merge closes it.)

Known limits: an approval is not one transaction with the merge or revert it runs. If the server stops between the two, the merge exists as the person's merge and the proposal is still open; answering it later closes it `superseded` with no `merge_id`. Nothing closes a proposal without a person: there is no sweep of stale proposals, so a proposal whose approval the person's own code refuses stays open, and keeps counting in `counts.agent_token_proposals`, until a person declines it.

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

### Finalized operations

Three more rows of the owner's table **finalize**: a token's call is applied at once, with no person approving it. Each answers exactly the body a signed-in person gets (no `outcome` or `action_id`), and each writes one `applied` row in the token-agent ledger and one audit row with the owner as actor and the token's id, the token's name at that moment and the ledger row's `action_id` in its detail. Agent History reads that row with `target_label` and, where there is one, `action`.

- **Mapping decision.** `POST /kbs/{kb_id}/review/mappings/{mapping_id}` with `{"status": "confirmed"}` or `{"status": "rejected"}` answers `{"ok": true}`. Any other status is 422 `bad_status`; an unknown or foreign mapping is 404; a status equal to the mapping's own is 409 `already_decided` and writes no row. A different status is applied, so a confirmed mapping can be rejected and a rejected one confirmed: deciding again is the undo. The new status, the `mapping.decide` ledger row and the `mapping.decided` audit row are written in one transaction. Read the mappings with `GET /kbs/{kb_id}/mappings` or `review_mapping_list`.
- **Consistency check and inference.** `POST /kbs/{kb_id}/consistency/check` and `POST /kbs/{kb_id}/inference/run` take no body and answer the report a signed-in person gets (the keys are listed under [Review over MCP](#review-over-mcp)). Inference on a base whose materialized-inference setting is off is 422 `inference_off`; nothing runs and no row is written. The run is the person's run, with no lock of its own. Right after it succeeds, the ledger row (`consistency.check` or `inference.run`, target the base) and the audit row (`consistency.checked` or `inference.materialized`) are written in one transaction; if they cannot be written, the call fails.
- **Kind word.** `POST /kbs/{kb_id}/review/alignment/kind-words/{kind_word}` with `{"class": "<class key>"}` (no `class`, or `null`, decides that no class fits) answers `{"ok": true}` and keeps every answer it gave before the Review tools existed: `unknown_class` (422), `alignment_busy` (409, try again shortly), `token_read_only`, `token_kb_out_of_scope`, and a plain 403 with no code for an owner below editor. The decision and its `alignment.kind_word` ledger row commit in one transaction; the audit row `alignment.kind_word_decided`, which now also carries the token's name and the `action_id`, is written after the commit, as for a person. A word the Alignment section does not list is recorded as a new binding, as it is for a person.
- **The limit.** A mapping decision and a run count toward the 200 finalized writes per hour and are refused with 429 `agent_rate_limited` past it, changing nothing and writing no row. A kind-word decision is never refused by the limit, and its rows count toward it.

Known limits: a consistency check or an inference run commits its own changes before its ledger and audit rows are written. If that write fails, the call answers an error while the run's changes stay, with no ledger row and no audit row for it. The kind-word audit row is best effort after the commit: if it cannot be written, the decision and its ledger row stand without their Activity row.

### The limit and the breaker

Both are checked for every token write that would finalize, in the one place all entries share, so they hold alike for the decision route, the manual merge, the revert, each batch member, the mapping decision, the two runs and the MCP tools. The kind-word decision is the one finalized write that does not ask them (see [Finalized operations](#finalized-operations)). Neither is a per-base setting.

**The limit.** A token finalizes at most 200 writes per base in a rolling hour. Keeps, queue merges, manual merges, reverts, mapping decisions, consistency checks, inference runs and kind-word decisions count, each when its ledger row is `applied`; proposals (held merges and every propose-only call) and refused calls do not. The 200th write applies; the next call answers **HTTP 429 with code `agent_rate_limited`**, changes nothing and writes no ledger row, until earlier writes are more than an hour old. Two things to know: the limit is checked before the gate, so a merge or revert past the limit answers 429 even if it would only have become a proposal (a propose-only call is never limited); and the count is read before the write, not in its transaction, so calls of one token that are in flight at the same moment can overshoot the limit by the number in flight.

**The breaker.** When people have reverted two merges made by one token on one base within seven days, that token's merges and reverts on that base stop being applied: each lands as a proposal with reason `breaker`, batch merge members and unit revert items included. Its keeps, mapping decisions, runs and kind-word decisions still finalize. The breaker closes by itself when fewer than two such reverts lie within the last seven days, counted from the time of each revert.

- Counted: a revert by a person of a merge this token made — from the Merges section, by approving a revert proposal, or by a signed-in unit revert.
- Not counted: a revert made by a token (this one or another), and a revert of a merge another token made. One revert by a person does not trip it.
- A person's revert is recognised from the ledger (the merge is reverted and no applied token revert names it) and also from the person's own `merge.revert` audit row, so it still counts when a token reverted the same merge at the same moment.

The breaker uses the numbers of the governance fuse (two reverts, seven days) but is separate from it: it is per token and base, and a token's own reverts never move the fuse.

### Refusal codes

Refusals carry a stable `code` beside the `error` message where one is listed:

| Status | Code | When |
|---|---|---|
| 401 | none | The token is unknown, revoked or expired; or the route is one that takes a signed-in session only (see below): the two proposal answer routes, the nod route, the pending-fact read by sentence, the governor's own decision route, Activity |
| 404 | none | On the seven reads: the token's knowledge-base list excludes the base, or its owner cannot open it. The answer is the same as for a base that does not exist, and carries no item, count, total, summary, proof, proposal or mapping. (The MCP endpoint answers a token whose list excludes the base with HTTP 403 instead.) On a write: no pair with that id in this base (keep, queue merge); an entity id that is unknown or of another base (manual merge); a merge id that is unknown or of another base (revert); a batch id that no token action of this base carries (unit revert); a target of a propose-only route or of the mapping decision that is unknown, of another base or gone, as [Propose-only operations](#propose-only-operations) and [The two token-only routes](#the-two-token-only-routes) describe for each route |
| 422 | `unknown_queue` | A token asks the list for `mappings` or for any name that is not one of the eleven queue keys. No items are returned |
| 422 | `unknown_type_filter` | `types` on the Duplicates section is not `any`, `same` or `conflict` |
| 422 | `self_merge` | The manual merge names one entity as both `source` and `target` |
| 422 | `batch_size` | A token batch has no ids or more than 50 (a signed-in person's batch: more than 500). The whole batch is refused |
| 422 | none | A batch names an action other than `merge` or `keep`. The whole batch is refused. Also: a conflict resolution with an action other than `close`, `retime_new`, `keep` or `reject_new` |
| 422 | `invalid_body` | A token sends the revert or the unit revert a body that is neither empty nor a JSON object such as `{"rationale": "why"}`; or sends one of the two token-only propose routes a body that is not a JSON object of that route's shape |
| 422 | the person's code | A propose-only route or a finalized operation gets a body the person's route refuses, with the same code: `bad_precision`, `close_at_required`, `new_from_required`, `not_a_state`, `interval_inverted`, `bad_resolution`, `fact_required`, `unknown_property`, `direction_required`, `unknown_marks`, `marks_needs_state`, `adoption_endpoint`, `bad_status`, `unknown_class`, `unknown_action`, `agent_answer`, `not_a_merge`, `inference_off`. Which route gives which is in the sections above. Nothing is written |
| 403 | `token_read_only` | A `read` token calls any write route |
| 403 | `token_kb_out_of_scope` | A `write` token whose knowledge-base list excludes the base calls any write route |
| 403 | `token_owner_below_editor` | The token's owner is below editor in the base (on the kind-word route: a plain 403 with no code, as before) |
| 403 | `token_operation_not_open` | A `write` token of an editor sends the decision route an action other than `keep` or `merge`. Nothing is written |
| 403 | `token_required` | A signed-in session calls one of the two token-only propose routes. A person nods and answers the governor on their own routes |
| 409 | `already_decided` | The item was decided before this call: a repeated call, a lost race, or a person decided it first. For a keep or a queue merge, the pair is no longer pending; for either merge, one of the entities was already merged into another; for a revert, the merge was already reverted; for a propose-only route, the target is already decided as its section says; for a mapping decision, the mapping already has that status |
| 409 | `alignment_busy` | The kind word is being updated by another operation; nothing was decided |
| 409 | none | The governor is deciding this pair right now (keep, queue merge, or a manual merge of two entities whose own pending pair it holds); nothing was decided, so try again after its proposal |
| 429 | `agent_rate_limited` | The token has finalized 200 writes on this base in the last hour (keep, merges, reverts, mapping decision, runs). Nothing is changed and no ledger row is written |

`already_proposed` is not a refusal: it is the `code` key of a 200 answer whose call met an open proposal. `unknown_action` and the 500-id `batch_size` of the proposal answer routes are listed under [Proposals](#proposals); a token never sees them, because those routes answer it with 401.

The checks run in a fixed order: the token's scope, then its knowledge-base list, then the owner's role, and only then the request itself — the body (batch size, batch action, the revert body, an action that is not open, the person's body codes), the target (404, then 409 `already_decided`), the checks that need the target's own state — and, for a write that would finalize, the limit. So a `read` token whose owner is a viewer gets `token_read_only`, whatever it sent. A refusal changes nothing: the item stays as it was and no ledger or audit row is written (over MCP, apart from the `mcp.tool_called` row every tool call writes).

**What stays closed to tokens.** A token reads every Review section, the open ontology proposals and the mappings; it keeps, merges, reverts and batches duplicate pairs as described above; it decides mappings, runs the consistency check and inference, and decides kind words; and it proposes every other Review decision. These stay closed to every token and answer it with 401:

- answering a proposal — approving or declining it at the two `agent-proposals` routes — for every token, the proposal's own included;
- the nod: `POST /kbs/{kb_id}/review/pending/{pending_id}`, and the read of one sentence's pending facts, `GET /kbs/{kb_id}/review/pending`. A token reads "Awaiting your nod" through the Review list and can only propose a nod;
- the governor's own decision route, `POST /kbs/{kb_id}/review/agent/{decision_id}`. A token can only propose an answer;
- Activity, `GET /kbs/{kb_id}/audit`.

On the decision route itself, an action other than `keep` or `merge` answers `token_operation_not_open`. Batches exist for duplicate pairs only. A signed-in person's requests and responses on all of these routes are unchanged, apart from the added `total` on list responses and the optional keys described above (`agent_proposals`, `counts.agent_token_proposals`, and on a proposal answer `job_id`, `queued` and `fact_id`).

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
