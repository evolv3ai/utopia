---
id: T002
title: Record ADR 0076 with the ruled authority table and check its numbers against upstream
wave: 1
deps: []
status: done
agent: build_T002
base: 5e101654fc91deff9eb3f5a8e7a8bc4983960cfc
worktree: null
task_branch: null
files:
  - docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md
  - docs/decisions/README.md
  - scripts/check-upstream-numbers.sh
---

# T002 — Record ADR 0076 with the ruled authority table and check its numbers against upstream

## Context

Upstream convention, binding here, is that a decision record precedes any public-API or data-model change. This milestone opens Review routes and MCP tools to personal tokens and adds a ledger table, so ADR 0076 must be in the tree before the first route or schema task. The owner ruled the agent authority on 2026-11-18: "Option 1: the recommended per-operation table"; the table with its reasons is in `.project/SYNTHESIS.md` under `### Agent-authority policy`, and the record must carry it without change (INTENT prohibition N11). One reason in that table, "ontology adoption has no undo", rests on low-confidence evidence and must be confirmed or corrected against the store before the record is final. New numbers start at ADR 0076 and migration 0128, each the lowest number above every merged number and every number claimed by an open upstream pull request (0127 is claimed by upstream pull request #1496); they are re-checked before each landing.

## Approach

- Write the record in English, in the form the directory uses: read `docs/decisions/README.md` (Conventions: one-line status line starting with one of the five words, then the date, then `open:`; revisions stay in place; function, table and constant names instead of line numbers) and a recent record such as `docs/decisions/0075-a-ticket-source-says-where-its-reading-stopped.md`. Add the record to the README index under the fitting domain; the index repeats only the first word of the status line.
- Content the record must settle, all taken from `.project/SYNTHESIS.md` (Decisions and User rulings) and `.project/intent/INTENT.md` (Scope in); do not invent or reopen a decision:
  - The opened Review routes: `GET /api/v1/kbs/{kb_id}/review?queue=duplicates`, `GET /api/v1/kbs/{kb_id}/review/summary`, and `POST /api/v1/kbs/{kb_id}/review/{review_id}` with action `keep`, by changing the extractor of the existing handlers from `AuthUser` to `SessionOrToken`; a session caller's requests and responses are unchanged. The MCP tools `review_list` (a `section` parameter, the existing 1 to 200 page clamp), `review_item` and `review_keep`.
  - The owner's ruled per-operation authority table with its reasons, enforced by the server and never by the MCP client, tool annotations or elicitation; a compiled-in table, not a per-base setting.
  - One implementation: each Review operation is one server function taking the resolved caller (owner, token, base), called by the REST token branch and by the MCP tool; the policy function lives with the `execution_gate` module (ADR 0027 decision 3).
  - Checks and codes: reads need an owner at least Viewer and a token that reaches the base (404 otherwise); writes check scope first, then the owner's role. Codes `token_read_only`, `token_kb_out_of_scope`, the new `token_owner_below_editor`, the new `already_decided` (a lost race or a repeated call), and `token_operation_not_open` for a token call to a Review write this build does not open (this last name is fixed by the plan; no source named one).
  - The ledger `token_agent_actions`: what it records per token-agent action, and that this milestone creates the columns it writes (base, operation, target kind and id, owner, token, token name snapshot, outcome, payload, time); the further columns of the settled design (later state, gate reason, batch id, undo reference, answering person) arrive with the milestones that write them.
  - Attribution: `audit_events.actor_id` stays the owner; every token Review write adds `detail.token_id` and a `detail.token_name` snapshot; shown as "owner via token name"; the stored HTTP `User-Agent` is returned marked as declared and never read for authorization; MCP `clientInfo` is not parsed. A failed snapshot or audit write fails the action.
  - Precedent isolation: token decisions are never human precedent; `resolution_reviews.decided_token`; `after_human_decision` is not called for a token decision; `review/agent/{decision_id}` stays session-only.
  - The named constants for later milestones: batch cap 50, 200 finalized writes per rolling hour. The statement that a merge revert restores graph state only (derivations, violations, exports and answers already given are not recalled).
  - What this milestone leaves closed, and the dead ends (the runner-ups of the synthesis decisions).
- Confirm or correct "ontology adoption has no undo" by reading the store and its routes: `crates/utopia-store/src/ontology.rs` (`save_agent_proposals`, `open_proposals`, `reject_proposal`, `adopt_iri_onto_key`), the adopt and reject routes in `crates/utopia-server/src/api/ontology_routes.rs`, and `unadopt_types` in `crates/utopia-store/src/resolution.rs` (an undo of a type-adoption batch; decide from the code whether it undoes the adoption of an ontology proposal). Record what you read and the finding under its own heading. The ruling of that row stays the owner's (propose only); only the reason text may be corrected, as a dated note beside the original reason.
- Record the owner's ruling of 2026-11-18 on the Decisions section and the Overview's decided counts, where a token keep, stored as a `review.keep` audit row under its owner, appears and is counted under that person: "leave Decisions and Overview unchanged for now" (owner, driver D056). This milestone does not change those two reads. Keep the point under `open:` as ruled for now, not as settled for the program.
- `scripts/check-upstream-numbers.sh` is read-only towards `deeplethe/utopia`: it lists the files of `docs/decisions/` and `migrations/` on upstream `dev` and the files added by open upstream pull requests. It must work without credentials (public GitHub API or `git`), may use `gh` when it is signed in, and fails closed when upstream cannot be read. Nothing is pushed, opened or commented upstream.
- If 0076 or 0128 is taken when you check, do not rename on your own: stop with `NEEDS-ORCHESTRATOR` naming the next lowest free number above every merged and claimed number, because this task's `files` and Verify, and T003's, name the numbers (INTENT edge E21).
- Conventions (bind this task): explanatory code comments in Chinese; tests and migrations named as sentences; user-facing error strings in English with a stable `code`; an ADR precedes any public-API or data-model change — this task is that ADR and lands before any route or schema task; new migration numbers start at 0128 and ADR numbers at 0076, re-checked against upstream `deeplethe/utopia` `dev` and its open pull requests before landing; migration SQL idempotent; one behaviour-sentence commit subject with a DCO `Signed-off-by`; nothing is sent to `deeplethe/utopia` or a deployed instance.

## Interface contract

- `scripts/check-upstream-numbers.sh <adr-number> <migration-number>` — run with bash from the repository root. It only reads `deeplethe/utopia` (branch `dev` and its open pull requests) and sends nothing. Exit 0 when no file `docs/decisions/<adr-number>-*.md` and no file `migrations/<migration-number>_*.sql` exists on upstream `dev` or is added by an open upstream pull request; exit 1, naming the colliding file, on a collision; exit 2 when upstream cannot be read. Numbers are four digits (`0076`, `0128`).

## Intent coverage

- SC11

## Acceptance criteria

1. `docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md` exists, follows the directory's conventions, and holds the authority table as one Markdown table whose first two columns are `Operation` and `Ruling`, with exactly the 19 operations of `.project/SYNTHESIS.md` `### Agent-authority policy` and their rulings unchanged: 12 rows `propose only`, 3 rows `finalize behind the server gate`, 4 rows `finalize`, each with its reason. The ruling cell is written exactly as `| propose only |`, `| finalize behind the server gate |` or `| finalize |`, and those three cell texts appear nowhere else in the file.
2. The record covers every item listed in the Approach: opened routes and tools, enforcement point, one implementation, checks and codes, the ledger, attribution, precedent isolation, the two named constants, the merge-revert statement, what stays closed, dead ends, and the `open:` item.
3. The record has a heading for the ontology adoption finding that names the functions read and states whether an adoption can be undone; if the low-confidence reason was wrong, the corrected reason stands beside the original as a dated note and the ruling is unchanged.
4. `docs/decisions/README.md` lists the record in its index with the first word of its status line.
5. `bash scripts/check-upstream-numbers.sh 0076 0128` exits 0 at landing, exits 1 for a number that upstream `dev` already uses (for example `0075 0126`), and exits 2 when upstream cannot be read.
6. No file outside `docs/decisions/` and `scripts/` changes.

## Verify

```bash
f=docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md && test -s "$f" && [ "$(grep -Fc '| propose only |' "$f")" -eq 12 ] && [ "$(grep -Fc '| finalize behind the server gate |' "$f")" -eq 3 ] && [ "$(grep -Fc '| finalize |' "$f")" -eq 4 ] && grep -Fq 'token_agent_actions' "$f" && grep -Fq 'token_owner_below_editor' "$f" && grep -Fq 'already_decided' "$f" && grep -Fqi 'ontology adoption' "$f" && grep -Fq '0076-a-token-agent-acts-on-review-under-a-ruled-table.md' docs/decisions/README.md && bash scripts/check-upstream-numbers.sh 0076 0128 && ! bash scripts/check-upstream-numbers.sh 0075 0126
```

Heavy: no

## Log

- 2026-11-18 — created by planner
- 2026-11-18 — build_T002 preflight: SC11 present in INTENT.md; Interface contract text identical in T003; all Verify paths declared. One map note, not blocking: the adopt route is `adopt` in `crates/utopia-server/src/api/question_routes.rs` (calls `ontology_agent::adopt`), not in `ontology_routes.rs`, which holds only the reject route `decide_proposal`; both were read.
- 2026-11-18 — build_T002 implementation: wrote `docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md` (status `Accepted`; the 19-row table is byte-identical to SYNTHESIS.md lines 115-135 by `diff`; D056 ruling under Open questions and in `open:`), added its row to the governance index of `docs/decisions/README.md`, wrote `scripts/check-upstream-numbers.sh` (one open-PR listing through `gh api` or the unauthenticated public API, one `git fetch --depth 1 --filter=blob:none` into a temporary bare repository; exit 0/1/2 per the Interface contract; sends nothing). Ontology adoption finding: confirmed, no undo — only three statements write `ontology_proposals` (`save_agent_proposals` insert/refresh of `open`, `reject_proposal` open→rejected, `finish_adoption` open→adopted); `unadopt_types` reverts an `entity_retypes` batch only; `adopt_iri_onto_key` is the OWL import's IRI claim. Reason text left unchanged.
- 2026-11-18 — build_T002 upstream numbers, read by hand with read-only commands (`git ls-remote`, `gh api` GET): upstream `dev` is 3d307f6 (the commit under this task's base, highest ADR 0075 and migration 0126); 22 open pull requests; the only numbered file an open pull request adds under `docs/decisions/` or `migrations/` is `migrations/0127_document_purge_finds_chunk_dependents.sql` (#1496). 0076 and 0128 are free.
- 2026-11-18 — build_T002 Verify: NOT RUN as written. The grep half ran directly and passed (exit 0: counts 12 / 3 / 4, the three names, `ontology adoption`, README entry). Both `bash scripts/check-upstream-numbers.sh …` calls were refused before execution by the project's PreToolUse guard (`.gsd-path/guard_hook.py`: "bash reads commands from a file or stdin the guard cannot inspect"); `bash -n` and `bash -c 'bash scripts/…'` were refused the same way, and the script body pasted inline was refused because the guard cannot parse `case`. The guard was not worked around. The script has therefore never been executed: its exit codes 0, 1 and 2 are unproven.
- 2026-11-18 — build_T002 blocked: verification cannot run from the coder's tool channel. Needed: the orchestrator's isolated Verify rerun of this worktree as it stands (no product change is pending), or a guard allowance for `bash scripts/check-upstream-numbers.sh`.
- 2026-11-19 — orchestrator Verify (isolate gsd-path-task/T002): pass, exit 0; output tail:
  ```
  free: decision record 0076 and migration 0128 (upstream dev and 22 open pull requests checked)
  collision: docs/decisions/0075-a-ticket-source-says-where-its-reading-stopped.md (upstream dev)
  collision: migrations/0126_a_superseded_chunk_is_found_by_its_age.sql (upstream dev)
  ```
- 2026-11-19 — orchestrator Verify (isolate gsd-path-task/T002): pass, exit 0; output tail:
  ```
  free: decision record 0076 and migration 0128 (upstream dev and 22 open pull requests checked)
  collision: docs/decisions/0075-a-ticket-source-says-where-its-reading-stopped.md (upstream dev)
  collision: migrations/0126_a_superseded_chunk_is_found_by_its_age.sql (upstream dev)
  ```
