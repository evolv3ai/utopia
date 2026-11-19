---
id: T004
title: Document the full Review read surface for tokens and amend ADR 0076 with it and with the three docs-audit corrections
wave: 2
deps: [T002, T003]
status: pending
agent: null
base: null
worktree: null
task_branch: null
files:
  - web/src/docs/mcp.md
  - docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md
---

# T004 — Document the full Review read surface for tokens and amend ADR 0076 with it and with the three docs-audit corrections

## Context

`web/src/docs/mcp.md` is the product's API and MCP documentation (sections "Review over MCP" and "Review over the token API"); it still says `review_list` serves two sections, `review_item` reads duplicate pairs only, and the other eleven Review sections cannot be listed with a token. T002 and T003 changed that: a token lists all thirteen sections, every list response carries `total`, both proof routes are open at Viewer, and `review_item` reads one listed item of any section. ADR 0076 (`docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md`) records the M001 cut and must be amended where a read shape differs from what it records (INTENT Scope in; upstream convention: the change that implements a record updates it). The plan also absorbs three docs-audit corrections on that record: queue 35 (fix-doc: the status line says every row of the table except Keep stays closed to tokens, while the alignment kind-word decision is already open to write tokens), queue 36 (ruling fix-doc, driver D074: decision 6 says of every read that a token whose `kb_ids` exclude the base gets 404, while the MCP endpoint answers HTTP 403) and queue 37 (ruling fix-doc, driver D077: decision 12 says manual merge, revert and batch answer `token_operation_not_open`, while those three routes are session-only and answer a token with 401). The server embeds `web/src/docs/*.md` at compile time, so this task's Verify rebuilds the server. No behaviour changes here.

## Approach

- Write from the code as landed in your base (`review_ops.rs`, `review_tools.rs`, `review_routes.rs`, `graph_routes.rs`) and from the Interface contract below; where the two disagree, stop with `NEEDS-ORCHESTRATOR` instead of documenting either.
- `web/src/docs/mcp.md`, English, in the file's existing style (tables, one paragraph per line). It must cover: the thirteen sections by name and key (Overview `overview`, Awaiting your nod `pending`, Duplicates `duplicates`, Conflicts `conflicts`, Unconfirmed `unconfirmed`, Low confidence `lowconf`, Axioms `violations`, Ontology `defects`, Alignment `alignment`, Errata `errata`, Agent History `agent`, Decisions `decisions`, Merges `merges`) and which route and which `review_list` arguments read each; the `total` field (the number of rows the section's list returns over all pages; `counts` keeps its badge values, so for Alignment, Low confidence and Agent History page by `total`); the page clamp and the offset past the end; Decisions' own `page`/`per` and `{events, total}`; the two proof routes; `review_item` with `section` and `id` (default `duplicates`, a kind word by its exact word, an Agent History row by its row id, the `proof` key on an Axioms item, one `not_found` answer for an id not listed); Agent History rows of both sources and how a token row names its owner and token; the refusals (401 dead token; 404 on the token API and the MCP endpoint's HTTP 403 for a token whose `kb_ids` exclude the base; `unknown_queue` for `mappings` and unknown names). Correct every sentence that says only two sections are served or that the other eleven cannot be listed. Keep the "Not open to tokens" paragraph true: every Review write other than keep stays closed, and reading "Awaiting your nod" gives a token no way to nod. Remove nothing about `remember` and its nod.
- ADR 0076: revisions stay in place (`docs/decisions/README.md`, Conventions) — keep a dated revision note where the original claim stood, saying what it said and what changed it; do not silently rewrite. Required amendments:
  - Queue 35: the status line (one line, starting `In progress`) names the alignment kind-word decision as already open to write tokens under ADR 0014 decision 6, as the table's own row does, and is brought up to date for this cut: the read slice built on its date; `open:` no longer lists token reads of the other eleven sections and of Agent History, and still lists what remains closed.
  - Queue 36: decision 6 states that the 404 is the token API's answer and that the MCP endpoint answers such a token with HTTP 403 before any tool runs, with no item, count or evidence; the endpoint's answer is unchanged (driver D074).
  - Queue 37: decision 12 says manual merge, revert and batch are session-only routes that answer a token with 401 until M003 opens them; only `merge` on the decision route answers `token_operation_not_open`. Decision 12 is also revised for what this cut opened (the other sections, item evidence, the proof routes, section totals).
  - Decisions 1 and 2 are revised for the reads: the extractor change now also covers the Decisions history and the two proof routes; `review_list` serves all thirteen sections; `review_item` takes `section` and `id`.
  - A new decision (next number after 12) records the read shapes ruled for this cut: `total` as a new field with `counts` unchanged; the Low confidence list unchanged and its total counting listed rows; Decisions keeping `page`/`per`; the Alignment identifier tie-break; `review_item` identity and the single not-found answer; the `proof` key (steps for `derived_contradiction`, null for other kinds); `mappings` refused for a token; the MCP 403 unchanged; no Review write added.
- The authority table of decision 4 is the owner's ruling and is not edited: its 19 rows, their rulings and reasons stay byte-identical, and the three ruling cell texts (`| propose only |`, `| finalize behind the server gate |`, `| finalize |`) appear nowhere else in the file.
- Do not edit `.project/research/DOCS-AUDIT.md`; the orchestrator marks queue rows 35, 36 and 37 with this task's id. No other queued doc ruling is in scope (driver D086). `docs/decisions/README.md` needs no change: the index repeats only the first word of the status line, which stays `In progress`.
- N4 (judgment prohibition, SC1). Must not: make any Review write other than the M001 keep reachable by a token. Detail: the reviewer checks that this milestone adds no token branch to a write handler and no MCP write tool, and that reading "Awaiting your nod" gives a token no way to nod. The documentation must not describe any write as open that is not.
- Conventions (bind this task): documentation in English; no new ADR number (the record is amended); one behaviour-sentence commit subject with a DCO `Signed-off-by`; nothing is sent to `deeplethe/utopia` or a deployed instance.

## Interface contract

- Token-open Review reads, all under `/api/v1` with `Authorization: Bearer utp_pat_…`, any scope, owner at least Viewer: `GET /kbs/{kb_id}/review?queue=<one of the eleven sections>` (optional `types` for `duplicates`, `limit`, `offset`), `GET /kbs/{kb_id}/review/summary`, `GET /kbs/{kb_id}/review/history?page=&per=`, `GET /kbs/{kb_id}/violations/{violation_id}/proof`, `GET /kbs/{kb_id}/derived/{derived_id}/proof`. Refusals: 401 for a dead or revoked token; 404 when the owner is below Viewer or the token's `kb_ids` exclude the base; 422 with code `unknown_queue` on the list route for `mappings` or any name that is not one of the eleven.
- MCP Review read tools (file `crates/utopia-server/src/api/review_tools.rs`), listed for every token that reaches the base with an owner at least Viewer. `review_list` takes `section` (required string: `overview`, `decisions` or one of the eleven queue sections), `limit` and `offset` (optional integers, queue sections only) and `page` and `per` (optional integers, `decisions` only); its `structuredContent` is the token API's body for that section — `{"counts","queue","items","total"}`, the Overview summary object, or `{"events","total"}`. `review_item` takes `section` (optional string, default `duplicates`; `decisions` or one of the eleven queue sections) and `id` (required string: the item's `id`, or the word of an Alignment kind word, compared exactly as stored); its `structuredContent` is that item exactly as the list returns it, and for section `violations` the same object plus `proof` (the proof steps for kind `derived_contradiction`, `null` for any other kind). Tool errors (`isError: true`, `structuredContent` `{"code","message"}`): `unknown_queue` for a section outside the thirteen, `mappings` included; `not_found` for an id not listed in that section of this base, and for any id with section `overview`. No Review tool is added; `review_keep` is unchanged.

## Intent coverage

- None

## Acceptance criteria

1. `web/src/docs/mcp.md` documents every item listed in the Approach for both surfaces, names all thirteen section keys, and contains no sentence saying that `review_list` serves two sections, that `review_item` reads duplicate pairs only, or that the other eleven Review sections cannot be listed with a token. What it says about each route, argument, key and code matches the code in this task's base.
2. Docs-audit queue 35: the ADR's status line is one line starting `In progress`, names the alignment kind-word decision as already open to write tokens, records the read slice as built, and its `open:` part no longer lists token reads of the other eleven sections or of Agent History.
3. Docs-audit queue 36: ADR decision 6 states the MCP endpoint's HTTP 403 for a token whose `kb_ids` exclude the base, beside the token API's 404, as a dated revision in place.
4. Docs-audit queue 37: ADR decision 12 says manual merge, revert and batch are session-only and answer a token with 401 until M003, as a dated revision in place; the text "batch (answered" is gone.
5. ADR decisions 1, 2 and 12 are revised for the reads, and a new numbered decision records the read shapes listed in the Approach, including the literal names `total`, `section` and `proof`.
6. The authority table is unchanged: the file still holds exactly 12 `| propose only |`, 3 `| finalize behind the server gate |` and 4 `| finalize |` cells.
7. Only the two files of this task change; the `docs_corpus` test `every_corpus_md_file_is_indexed` passes on a server built with the new `mcp.md`.

## Verify

```bash
set -o pipefail && test -s web/src/docs/mcp.md && ! grep -Fq 'serves two sections' web/src/docs/mcp.md && ! grep -Fq 'other eleven Review sections cannot be listed' web/src/docs/mcp.md && ! grep -Fq 'reads duplicate pairs only' web/src/docs/mcp.md && grep -Fq '`pending`' web/src/docs/mcp.md && grep -Fq '`conflicts`' web/src/docs/mcp.md && grep -Fq '`unconfirmed`' web/src/docs/mcp.md && grep -Fq '`lowconf`' web/src/docs/mcp.md && grep -Fq '`violations`' web/src/docs/mcp.md && grep -Fq '`defects`' web/src/docs/mcp.md && grep -Fq '`alignment`' web/src/docs/mcp.md && grep -Fq '`errata`' web/src/docs/mcp.md && grep -Fq '`agent`' web/src/docs/mcp.md && grep -Fq '`merges`' web/src/docs/mcp.md && grep -Fq '`decisions`' web/src/docs/mcp.md && grep -Fq '`total`' web/src/docs/mcp.md && grep -Fq '`proof`' web/src/docs/mcp.md && grep -Fq 'review/history' web/src/docs/mcp.md && grep -Fq '/proof' web/src/docs/mcp.md && grep -E '^- \*\*Status\*\*: In progress' docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md | grep -Fq 'kind-word' && ! grep -Fq 'batch (answered' docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md && tr '\n' ' ' < docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md | tr -s ' ' | grep -Fq 'HTTP 403' && tr '\n' ' ' < docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md | tr -s ' ' | grep -Fq 'answer a token with 401' && grep -Fq '`total`' docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md && grep -Fq '`proof`' docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md && grep -Fq '`section`' docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md && grep -Fc '| propose only |' docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md | grep -qx 12 && grep -Fc '| finalize behind the server gate |' docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md | grep -qx 3 && grep -Fc '| finalize |' docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md | grep -qx 4 && docker exec utopia-db-1 psql -U utopia -d utopia -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS m002_t004 WITH (FORCE)" -c "CREATE DATABASE m002_t004" && export UTOPIA_DATABASE_URL=postgres://utopia:utopia@127.0.0.1:1517/m002_t004 && cargo run -p utopia-store --example migrate && UTOPIA_TEST_REQUIRE_DB=1 cargo test -p utopia-server -- every_corpus_md_file_is_indexed 2>&1 | tee /dev/stderr | grep -c -E "every_corpus_md_file_is_indexed \.\.\. ok" | grep -qx 1
```
<!-- Runs in a fresh sidecar worktree at the landed commit with nothing
     installed. It creates and migrates its own database `m002_t004` on the
     compose Postgres and rebuilds the server, which embeds
     `web/src/docs/mcp.md` at compile time. The orchestrator's environment
     supplies an empty `CARGO_TARGET_DIR` outside the worktree and PostgreSQL
     16 client tools on PATH. The ADR is hard-wrapped, so the two phrase checks
     read it with line breaks folded into spaces; write "HTTP 403" in decision
     6 and "answer a token with 401" in decision 12 in those words. -->

Heavy: yes

## Log

- 2026-11-19 — created by planner
