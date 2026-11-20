# Evidence — domain

<!-- Written by one researcher role. Consumed by the decider. -->

Dimension: domain
Questions assigned: The ruled table makes "answer a governor decision" propose only for a token, while the synthesis keeps the governor's own decision route session-only. Through which token route and MCP tool does an agent propose an answer to a governor decision, given that in code that answer is a second way to merge, keep, revert or manually merge a pair as a person (map F12)? The behaviour is ruled in edge E6; the route and tool are open.; Which MCP tool names and families satisfy "one write tool per operation family" under the name rules the M003 tests pin (map F20)?

<!-- Method: Read/Grep only, repository at revision d5d2356 (branch gsd-path/M004). No build, test or
     Git command was run. Web pages were opened with a live fetch on 2026-11-20; every URL below was
     opened. "map F<n>" is a finding of .project/research/evidence-codebase.md. Paths are relative to
     D:\utopia. Options are laid out; the choice belongs to decide. -->

## Finding: D1 — An answer to the governor is defined as a person's decision made through the person's own path

- **Claim**: ADR 0025 decision 6 defines an answer as: accepting or overriding a proposal "calls `decide_review` with the person's id, exactly as the Review page does"; reverting an applied merge calls `revert_merge`; merging over an applied keep calls `merge_entities` with the person as `merged_by`; "Each writes the usual audit row plus the decision id. So an answer is a person's decision in every table that records decisions, and by decision 1 it is precedent for the next look." Decision 5 names the answer states: `accepted`, `overridden` and `reverted` "are a person's answers". The design page repeats it: "Answers go through the person's own decide path, so an answer is a person's decision and a precedent".
- **Source**: `docs/decisions/0025-governance-reads-the-ledger-before-it-decides.md:54, 58-60`; `docs/design/governance.md:47-51`; handler doc comment `crates/utopia-server/src/api/review_routes.rs:437-438`
- **Confidence**: high
- **Why it matters here**: The ruled row "Answer a governor decision (accept, override, revert)" names an act whose definition includes "by a person"; a token can only ever hold a proposal of it, which is what edge E6 rules.

## Finding: D2 — The terms accept, override and revert are outcomes the server computes, not request values

- **Claim**: The answer body carries `action` = `merge`, `keep` or `revert` plus an optional `rationale`; no request says "accept" or "override". The handler has three shapes: a `proposed` decision answered `merge` or `keep` is settled `accepted` when the action equals the governor's and `overridden` when it differs; an `applied` merge answered `revert` is settled `reverted`; an `applied` keep answered `merge` is settled `overridden`. Any other pair of status and action is refused with code `agent_answer`. `governance::settle` only writes while the row is `proposed` or `applied`, and otherwise answers an uncoded conflict "this decision has already been answered".
- **Source**: `crates/utopia-server/src/api/review_routes.rs:427-435, 451-551`; `crates/utopia-store/src/governance.rs:1009-1035`; web buttons `web/src/pages/Review.tsx:647-666`; status labels `web/src/i18n/en.ts:2119-2126`
- **Confidence**: high
- **Why it matters here**: A proposable governor answer has the payload `{decision id, action merge|keep|revert, rationale}`; whether it is an accept or an override is known only from the governor's row, and intent edge E3 ("a governor decision already answered answers `already_decided`") maps onto the `settle` guard.

## Finding: D3 — An answer records four things beyond the merge or keep: the settled row, precedent with its "why", a govern round with superseded siblings, and (revert only) the fuse

- **Claim**: (1) The governor's `agent_decisions` row is settled with `decided_by` and `decided_at`. (2) The audit row is the person's (`review.merge`, `review.keep`, `merge.revert` or `merge.manual`) with `agent_decision`, and for accept and override `agent_action` and `why`; ADR 0025 decision 1 counts it as precedent ("A proposal a person accepted counts"), and ADR 0026 decisions 1 to 3 make the rationale ("What told you?") the quoted ground of that precedent. (3) `after_human_decision` supersedes open governor proposals on sibling pairs and queues a `govern` round. (4) Only the revert shape calls `governance::fuse`; ADR 0025 decision 9: two reverts of the governor's merges within seven days turn governance off for the base, and "A person merging over an applied keep, or overriding a proposal, does not count".
- **Source**: `crates/utopia-server/src/api/review_routes.rs:467-479, 489-502, 525-542, 552`; `crates/utopia-server/src/governance.rs:978-1013`; `docs/decisions/0025-governance-reads-the-ledger-before-it-decides.md:19, 68-72`; `docs/decisions/0026-a-decision-records-why.md:17-27`
- **Confidence**: high
- **Why it matters here**: Edge E6 forbids all four on the token's call and requires all four when a person approves ("the answer runs as that person's answer does today"); they are also what a person must be shown before approving (finding D7).

## Finding: D4 — In the product's own terms, deciding the pair is answering the governor's proposal: there are two entry points and one meaning

- **Claim**: A person's Merge or Keep on a duplicate card answers the governor's open proposal on that pair without going through the governor route: `after_human_decision(.., Some(action), ..)` calls `governance::answer_open`, which sets the open proposal `accepted` or `overridden`. ADR 0025's status history describes "the proposal chip on a duplicate card whose Merge / Keep answers the proposal", and its decision 9 says both revert paths (the Agent queue's Revert and the Merges page's Revert) mark the governor's row `reverted` and count for the fuse (`after_revert` → `settle_by_merge` → `fuse`).
- **Source**: `crates/utopia-server/src/api/review_routes.rs:415-422`; `crates/utopia-server/src/governance.rs:969-998`; `crates/utopia-store/src/governance.rs:1037-1059, 1117-1136`; `docs/decisions/0025-governance-reads-the-ledger-before-it-decides.md:72, 123`; `docs/design/governance.md:60-62`
- **Confidence**: high
- **Why it matters here**: This is the factual basis of option 2 of the governor question (no separate operation); it shows which existing paths already settle the governor's row and so which do not.

## Finding: D5 — The M003 proposals already settle the governor's row in two of the four answer shapes when a person approves them, and not in the other two

- **Claim**: At `d5d2356`: approving a token queue-merge proposal runs `decide_review` then `after_human_decision(.., Some("merge"), ..)`, so an open governor proposal on that pair becomes `accepted` or `overridden`; approving a token revert proposal runs `revert_merge` then `after_revert`, so a governor merge is settled `reverted` and the fuse counts it. Approving a manual-merge proposal calls no governance follow-up (its comment: "也没有治理的后续"), so a governor's applied keep is not settled `overridden`. A token keep finalizes (ruled row Keep) and by design does not answer the open proposal on its pair ("不替人回答这一对上开着的建议"); the same holds for a token merge the gate passes. No existing token call proposes a keep.
- **Source**: `crates/utopia-server/src/api/review_routes.rs:653-728` (queue merge, `:725`), `:744-793` (manual merge, comment `:744-746`), `:795-838` (revert, `:836`); `crates/utopia-server/src/api/review_ops.rs:355-359, 487`; `web/src/docs/mcp.md:334, 350` Confidence note: high for the code paths read; medium for end-to-end behaviour (no test was run).
- **Confidence**: medium
- **Why it matters here**: Under option 2 the row "answer a governor decision: propose only" would be met by other rows' outcomes: `applied` for a keep and for a gate-passing merge, `proposed` only for a held merge and for a revert of the governor's merge.

## Finding: D6 — A governor keep is not "a pair a person kept", so the token merge gate does not hold a merge over it for that reason

- **Claim**: The token-only hold `kept_by_person` requires the latest decided review row of the two entities to be `kept` with `decided_by IS NOT NULL AND decided_token IS NULL`. The governor closes a keep through `close_review_auto(.., "kept", "governed|<conf>")`, which names no person. So the fourth answer shape (merge over the governor's applied keep) is reachable today as a token manual merge that finalizes when the four `hold_for_merge` checks pass, with the governor's row left `applied`.
- **Source**: `crates/utopia-store/src/execution_gate.rs:270-302`; `crates/utopia-server/src/governance.rs:627-636`; `crates/utopia-server/src/api/review_token_merge_tests.rs:850-870` (a test seeds a review closed `kept` with `governed|0.99`) Confidence note: medium (static reading; `close_review_auto`'s body and the test's assertions after line 870 were not read).
- **Confidence**: medium
- **Why it matters here**: It bears on prohibition N9 (no outcome stronger than the row through another route) whichever option is chosen: the override-of-a-keep shape already has a finalize path under the Merge row.

## Finding: D7 — No record anticipates another agent proposing an answer to the governor; the ruled row and intent edge E6 are the only sources

- **Claim**: ADRs 0025, 0026, 0027, 0028 and 0043 and `docs/design/governance.md` describe only a person answering the governor ("A person answers from the Agent queue: accept or override a proposal, or revert an applied decision", 0043 decision 4). The only text about a token and a governor decision is ADR 0076: the table row ("The route is the human check on the server's own agent; a token answer would be one model approving another and would be stored as a person's answer") and decision 9 ("The governor's own decision route, `review/agent/{decision_id}`, stays session-only"). The research finding the row cites describes the three shapes and gives the same reason; it proposes no route or tool.
- **Source**: grep of `docs/decisions/` and `docs/design/` for `governor` (7 decision records, 3 design pages); `docs/decisions/0043-every-review-queue-is-governed.md:50`; `docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md:98, 199-204`; `.project/SYNTHESIS.md:73, 126`; `.project/archive/001-token-review-skeleton/research/evidence-pitfalls.md:61-66` (read only) Confidence note: high that no such text exists in the files searched.
- **Confidence**: high
- **Why it matters here**: Decide has no prior design to adopt for the route or tool; whatever it chooses is a new decision that ADR 0076 must record as a dated revision.

## Finding: D8 — The product's records disagree on whether the governor route may gain a token branch

- **Claim**: Three statements pull in different directions. (a) SYNTHESIS "Token access to Review routes": handlers change extractor "in place (same URLs, no parallel route tree)", and ADR 0076 dead end "A separate token-only route tree". (b) SYNTHESIS "Governor precedent and the governance fuse" and ADR 0076 decision 9: the governor's route "stays session-only"; decision 12's last revision lists "the governor's decision route" as closed to every token separately from "every propose-only row of the table"; `mcp.md` lists "answering the governor's own decisions" under routes that answer a token 401; a test pins 401 on it. (c) The ruled table makes the operation propose only, and the policy enum has `AnswerGovernor => ProposeOnly` with no caller. M003 already added three new routes where no session handler fitted (unit revert, two proposal answer routes).
- **Source**: `.project/SYNTHESIS.md:31-32, 73, 126`; `docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md:199-204, 303-316, 364-372, 536-539`; `web/src/docs/mcp.md:451`; `crates/utopia-server/src/api/review_token_reads_tests.rs:1099` (per map F19); `crates/utopia-store/src/execution_gate.rs:161, 216-218`
- **Confidence**: high
- **Why it matters here**: A token branch on `review/agent/{decision_id}` contradicts the letter of (b) and changes a pinned test and two documents; a new route contradicts the letter of (a). The conflict is reported, not resolved.

## Finding: D9 — What a person needs to see to approve a proposed governor answer

- **Claim**: The governor's own row shows: its action (merge, keep, unsure), both names, confidence, status, the question it asks, its reason, the precedents shown to it and its lookup trace, and an input for the person's rationale. An approval of a token proposal today is recorded with `why: null` because "the token's sentence stays on the proposal and does not pose as this person's words". From D2 and D3, a proposed answer adds: which governor decision it targets and that decision's current status; which of the shapes it is (same as the governor = accept, different = override, revert of an applied merge, merge over an applied keep); the token's rationale as the token's; the owner and token name; and the consequences that differ from other proposals: the approval is that person's precedent, starts a govern round, supersedes sibling proposals, and a revert counts toward the two-in-seven-days fuse that turns governance off.
- **Source**: `web/src/pages/Review.tsx:564-671`; `crates/utopia-server/src/api/review_routes.rs:699-707, 722`; findings D2, D3; intent prohibitions N2, N5, N10 in `.project/intent/INTENT.md:80-88` Confidence note: medium (the list is assembled from the row component and the records; no record states it for a token proposal).
- **Confidence**: medium
- **Why it matters here**: Success criterion 2 requires the proposal on the item's card and in Agent History; the item here is a governor row in Agent History itself, so both places are the same section.

## Finding: D10 — The six shipped Review tools: names, families, listing and annotations

- **Claim**: `review_tools.rs` defines `review_list`, `review_item` (reads) and `review_keep`, `review_merge`, `review_revert`, `review_batch` (writes; `writes()` names exactly these four). One tool covers one ruled row and may take two argument forms: `review_merge` covers the queue merge and the manual merge (one table row), `review_revert` covers one merge and a unit revert. `listed(can_write)` hides the write tools unless the scope is `write` and the owner is at least Editor. Annotations: reads `{readOnlyHint: true}`; `review_keep` `{readOnlyHint: false, destructiveHint: false, idempotentHint: false}`; the other three `{readOnlyHint: false, destructiveHint: true, idempotentHint: false}`. A write-scope Editor is listed 18 tools today (11 in `EXPOSED`, `remember`, 6 review tools).
- **Source**: `crates/utopia-server/src/api/review_tools.rs:31-46, 70-355, 358-399`; `crates/utopia-server/src/api/mcp.rs:43-69, 197-204`; `web/src/docs/mcp.md:52-57, 207-212, 274`; `docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md:68-74`
- **Confidence**: high
- **Why it matters here**: "One write tool per operation family" has one precedent only: four tools for four table rows, named `review_<one word>`, each calling one or two `review_ops` functions.

## Finding: D11 — The pinned name rules, quoted, and the reason each test states

- **Claim**: Three tests pin names.
  1. `review_proposal_answer_tests.rs:1122-1130`: for every tool in a write token's `tools/list` (all tools, not only review ones), the name must not contain any of `["proposal", "approve", "decline", "answer"]`; failure message "no tool answers a proposal: {name}". `:1131-1151`: calls to `review_approve`, `review_decline`, `review_answer`, `review_proposal_answer`, `agent_proposal_answer` must answer JSON-RPC `-32601` with no result ("{name} is an unknown tool"). Section comment `:1111`: "MCP：写令牌看到的工具里没有一个答提议的；编出来的名字是「没有这个工具」".
  2. `review_mcp_reads_tests.rs:1180-1199`: `review_decide`, `review_confirm`, `review_approve`, `review_pending`, called with a pending-fact id, must answer `-32601` ("{name} must not exist"). Comment `:1171`: reading "Awaiting your nod" gives a token no way to nod: "工具里也没有能点头的".
  3. `review_token_merge_tests.rs:1816-1830`: `review_decide`, `review_confirm`, `review_approve` must answer `-32601`. Comment `:1816`: "没有哪个工具能替人点头或答提议：这些名字不存在".
  ADR 0076 decision 14 gives the rule behind them: "no MCP tool answers a proposal (a call to an invented `review_` name is JSON-RPC -32601)". The eight exact names that must stay unknown are `review_confirm`, `review_decide`, `review_pending`, `review_approve`, `review_decline`, `review_answer`, `review_proposal_answer`, `agent_proposal_answer`.
- **Source**: lines in the claim; `docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md:431-436`
- **Confidence**: high
- **Why it matters here**: The intent constraint keeps these rules; a name that breaks one cannot be fixed by adding a list entry, only by changing the test, which the intent allows only if decide records a reason.

## Finding: D12 — Two further assertions break for any new tool whose name starts with `review`, and are fixed by list entries

- **Claim**: `review_mcp_reads_tests.rs:1050-1081` asserts the sorted names starting with `review` equal `["review_item", "review_list"]` for a read token (viewer or editor) and `["review_batch", "review_item", "review_keep", "review_list", "review_merge", "review_revert"]` for a write token of an editor. `:1083-1102` then requires every tool whose name starts with `review`, other than the four named write tools, to have annotations exactly equal to `{"readOnlyHint": true}`; the four must have `readOnlyHint == false`. A new write tool named `review_*` therefore fails both until it is added to the expected list (`:1064-1071`) and to the `writes` expression (`:1087-1090`); a new read tool named `review_*` fails the two read-token lists (`:1054, 1059`) as well. `review_mcp_tests.rs:1171-1175` (no chat tool starts with `review`) and `:881-894` (each tool listed once, `review_keep` listed only for a writer) keep passing.
- **Source**: lines in the claim
- **Confidence**: high
- **Why it matters here**: These are the "exact tool lists that gain entries" intent edge E22 permits; they sit in one test function, so the tasks that add tools all touch `review_mcp_reads_tests.rs`.

## Finding: D13 — MCP tool-name rules: every candidate below conforms; the rules are SHOULD and are absent from the revision the server declares

- **Claim**: The server declares protocol revision `2025-06-18`, whose tools page has no tool-name rules (name is only "Unique identifier for the tool"). Revision `2025-11-25` and the current revision `2026-07-28` add: names "SHOULD be between 1 and 128 characters", "SHOULD be considered case-sensitive", allowed characters "uppercase and lowercase ASCII letters (A-Z, a-z), digits (0-9), underscore (_), hyphen (-), and dot (.)", "SHOULD NOT contain spaces, commas, or other special characters", "SHOULD be unique within a server". The versioning page states "The current protocol version is 2026-07-28".
- **Source**: https://modelcontextprotocol.io/specification/2025-06-18/server/tools ; https://modelcontextprotocol.io/specification/2025-11-25/server/tools ; https://modelcontextprotocol.io/specification/2026-07-28/server/tools ; https://modelcontextprotocol.io/specification/versioning ; `crates/utopia-server/src/api/mcp.rs:33`
- **Confidence**: high
- **Why it matters here**: The standard does not constrain the choice between the naming schemes; only the product's own tests do. Lower-case `snake_case` names under 30 characters satisfy every revision.

## Finding: D14 — The MCP specification gives no number or grouping rule for tools; listing by the caller's scope is explicitly allowed in the current revision

- **Claim**: No revision opened states a maximum or recommended number of tools or how to group operations into tools. `tools/list` "supports pagination" in all three. Revision `2026-07-28` adds that the listed set "MAY vary by the authorization presented on the request — for example, returning only the tools the caller's granted scopes permit" and that servers "SHOULD return tools in a deterministic order". The only guidance found on count is secondary: Anthropic's "Writing tools for agents" (2025-09-11): "More tools don't always lead to better outcomes", "Too many tools or overlapping tools can also distract agents", tools "can consolidate functionality, handling potentially multiple discrete operations", and "Namespacing (grouping related tools under common prefixes) can help delineate boundaries between lots of tools".
- **Source**: the three specification tools pages in D13; https://www.anthropic.com/engineering/writing-tools-for-agents Confidence note: high for the specification text; medium for the secondary article (vendor guidance, not a standard).
- **Confidence**: medium
- **Why it matters here**: Between 11 and 16 new tools is not a conformance question; it is a usability trade-off, and the shared `review_` prefix the tests filter on is the namespacing the secondary source recommends.

## Finding: D15 — What the three annotation hints mean for a tool that only proposes

- **Claim**: Schema `2025-11-25`, `ToolAnnotations`: "all properties in ToolAnnotations are **hints**. They are not guaranteed to provide a faithful description of tool behavior"; `readOnlyHint`: "If true, the tool does not modify its environment. Default: false"; `destructiveHint`: "If true, the tool may perform destructive updates to its environment. If false, the tool performs only additive updates. (This property is meaningful only when `readOnlyHint == false`) Default: true"; `idempotentHint`: "If true, calling the tool repeatedly with the same arguments will have no additional effect on its environment. ... Default: false"; `openWorldHint` default true. Clients "MUST consider tool annotations to be untrusted unless they come from trusted servers". A propose-only call writes a ledger row and an audit row and changes no item, so: `readOnlyHint` false (it writes); `destructiveHint` false is the accurate value and must be set explicitly because the default is true; `idempotentHint` true is defensible while a proposal is open (edge E2: a repeat returns the same proposal and writes no second row) and not after it is answered, and M003 set false on every write tool. The finalize tools differ: a mapping decision and a kind-word decision overwrite a status (not additive), and a consistency run deletes stale open violations (map F11).
- **Source**: https://modelcontextprotocol.io/specification/2025-11-25/schema (section `ToolAnnotations`, read from offset 100000); https://modelcontextprotocol.io/specification/2025-11-25/server/tools ; `crates/utopia-server/src/api/review_tools.rs:357-373`; ADR 0076 decision 3 (`:76-80`) Confidence note: high for the quoted definitions; medium for the mapping onto each operation (an inference).
- **Confidence**: medium
- **Why it matters here**: Annotations are per tool, so a tool that mixes a propose-only and a finalize operation (for example phrase, rule and kind word in one tool) can carry only the stronger hint; the server reads none of them (ADR 0076 decision 3).

## Finding: D16 — How the shipped tools word a call that only proposed, against one that applied

- **Claim**: Applied: `review_keep` says "Applied at once and recorded under the token's owner and the token's name". Proposed: `review_merge` says "nothing is changed: the call waits as a proposal that only a signed-in person can answer in the Review tab, and the result is {ok, outcome: \"held\" or \"proposed\", action_id, reason, explanation}. Repeating a held call returns the same proposal with code already_proposed. No tool answers a proposal." The answer's `explanation` is built as "Not applied: {why}. Nothing was changed; the call waits as a proposal that a signed-in editor answers in the Review tab." The web labels a proposal as intent, not act: `tokenProposes` "wants to merge a duplicate pair", and the audit label `agent.proposal` "proposed a change that waits for a person". `review_list` ends "nothing listed here is decided by reading it".
- **Source**: `crates/utopia-server/src/api/review_tools.rs:88-90, 168-175, 197-219, 264-271`; `crates/utopia-server/src/api/review_ops.rs:432-457`; `web/src/i18n/en.ts:2140-2157`
- **Confidence**: high
- **Why it matters here**: Prohibition N11 is judged on the tool descriptions; the shipped wording gives the sentences a propose-only tool can reuse, and shows that a description opening with the decision verb ("Confirm a fact…") would read as the agent's own act.

## Finding: D17 — The specification's unknown-tool error code differs from the one the product's tests pin

- **Claim**: All three specification revisions show an unknown tool as a JSON-RPC protocol error with code `-32602` ("Unknown tool: invalid_tool_name"). The product answers an unknown or unexposed tool name with `-32601`, and the three name tests of D11 assert `-32601`.
- **Source**: the three specification tools pages in D13 (section "Error Handling"); `crates/utopia-server/src/api/mcp.rs:234-247, 286`; `crates/utopia-server/src/api/review_proposal_answer_tests.rs:1147-1150`
- **Confidence**: high
- **Why it matters here**: The names that "must stay unknown" are pinned with the product's code, not the specification's; changing the code is outside this milestone and would touch tests the intent says keep their expectation.

## Finding: D18 — Intent edge E19 and the shipped MCP behaviour disagree on what a refused caller gets from a write tool

- **Claim**: Intent edge E19 says "over MCP the write tools are not listed for those callers and a call to one is an unknown tool, as the M003 tools behave". The M003 tools do not behave so: a Review tool name is dispatched without asking `is_exposed`, and a read-scope token or a below-editor owner calling `review_keep` gets a tool error whose `structuredContent` carries `token_read_only` or `token_owner_below_editor`, equal to the REST code. Tests assert it, and `mcp.md` documents it ("calling it anyway returns a tool error with a code that says why"). Only `remember` answers `-32601` when not listed.
- **Source**: `.project/intent/INTENT.md:70`; `crates/utopia-server/src/api/mcp.rs:210-233`; `crates/utopia-server/src/api/review_tools.rs:482-486`; `crates/utopia-server/src/api/review_mcp_tests.rs:783-799, 909-922`; `web/src/docs/mcp.md:54`
- **Confidence**: high
- **Why it matters here**: Success criterion 6 asks for a refusal "with a stable code ... over MCP"; the shipped behaviour gives that and the edge's wording does not. The two sources conflict and the conflict is left for decide or the user.

## Finding: D19 — The specification requires a server to rate limit tool invocations; intent edge E5 exempts propose-only calls from the only limit the product has

- **Claim**: Every revision's tools page says "Servers **MUST**: Validate all tool inputs; Implement proper access controls; Rate limit tool invocations; Sanitize tool outputs". The product's limit counts `applied` ledger rows only (map F6), and intent edge E5 rules that "proposals and refused calls do not count toward the 200-per-hour limit and a propose-only call is never answered `agent_rate_limited`". The program synthesis recorded "No rate limit on MCP or the API" as a pitfall answered by the ledger-based limit, and that a cap on `remember` calls was not read.
- **Source**: the three specification tools pages in D13 (section "Security Considerations"); `.project/intent/INTENT.md:56`; `.project/SYNTHESIS.md:159, 192`; `crates/utopia-server/src/api/review_ops.rs:290-344` (per map F6, not re-read line by line) Confidence note: high for the quoted texts; medium that no other limit applies to the MCP endpoint (not searched beyond `mcp.rs:171-288`, which has none).
- **Confidence**: medium
- **Why it matters here**: Nine or more new propose-only tools would be callable without any count limit; one open proposal per target bounds stored rows, not calls. The specification and the edge ruling point different ways; it is recorded, not resolved.

## Assigned questions — answers

- The ruled table makes "answer a governor decision" propose only for a token, while the synthesis keeps the governor's own decision route session-only. Through which token route and MCP tool does an agent propose an answer to a governor decision, given that in code that answer is a second way to merge, keep, revert or manually merge a pair as a person (map F12)? The behaviour is ruled in edge E6; the route and tool are open. → No record names a route or a tool (D7; `docs/decisions/0076-a-token-agent-acts-on-review-under-a-ruled-table.md:98, 199-204`). The records define an answer as a person's decision through the person's own path that settles the governor's row, becomes precedent, starts a govern round and, for a revert, counts toward the fuse (D1, D2, D3; `docs/decisions/0025-governance-reads-the-ledger-before-it-decides.md:54-72`, `crates/utopia-server/src/api/review_routes.rs:439-555`). Options: (1) a distinct proposable operation targeting the governor's decision id (policy row `AnswerGovernor`, payload action merge, keep or revert) with its own MCP tool whose name does not contain `answer` (`review_governor` or `review_propose_governor`, D11), on (1a) a token branch of `review/agent/{decision_id}` or (1b) a new route; (2) no separate operation, the row met by existing merge, keep and revert proposals on the same pair (D4). The evidence favours option 1: intent Scope in and edges E1, E3 and E6 presuppose a proposal on the governor's decision and SC6 is then met directly, while option 2 settles the governor's row in two of four answer shapes only, yields finalize outcomes for keep and gate-passing merges, and fails SC6 as written (D5, D6; `crates/utopia-server/src/api/review_routes.rs:653-838`). Between 1a and 1b the records conflict and the domain evidence does not settle it (D8; `.project/SYNTHESIS.md:31-32, 73`). Detail is under "Option detail — governor answer".
- Which MCP tool names and families satisfy "one write tool per operation family" under the name rules the M003 tests pin (map F20)? → Options: scheme A, one noun per family (`review_fact`, `review_conflict`, `review_violation`, `review_defect`, `review_errata`, `review_alignment`, `review_ontology`, `review_mapping`, `review_check`, `review_pending_fact`, `review_governor`; 11 new tools); variant A′, the same with `review_kind_word` split from `review_alignment` so no tool mixes a finalize and a propose-only ruling (12); scheme B, `review_propose_<noun>` for propose-only tools plus plain finalize names (12); scheme C, one tool per ruled table row (16). Every listed name passes the pinned rules: none contains `proposal`, `approve`, `decline` or `answer`, and none is one of the eight names that must stay unknown (D11; `crates/utopia-server/src/api/review_proposal_answer_tests.rs:1122-1151`, `crates/utopia-server/src/api/review_mcp_reads_tests.rs:1180-1199`, `crates/utopia-server/src/api/review_token_merge_tests.rs:1816-1830`). Each scheme needs new entries in the exact tool list and the `writes` expression of `review_mcp_reads_tests.rs:1062-1071, 1087-1090` (D12); only a name with a forbidden substring or one of the eight exact names fails outright. The MCP specification constrains none of the schemes (D13, D14, D15; https://modelcontextprotocol.io/specification/2026-07-28/server/tools). The evidence favours A′: it continues the shipped `review_<word>` precedent (D10), has the fewest tools among schemes with one ruling per tool, and keeps one ruling per description for prohibition N11 (D16); scheme C is the alternative if per-tool annotations and N11 outweigh the tool count. Detail is under "Option detail — tool names".

## Option detail — governor answer

**What the records say an answer is (domain part).** An answer is a person's decision made through the person's own decide path, recorded as that person's in every table and read as precedent (D1). Its vocabulary is three request actions (`merge`, `keep`, `revert`) and three settled states the server computes (`accepted`, `overridden`, `reverted`) (D2). Beyond the merge or keep it settles the governor's row, becomes precedent with its rationale, supersedes sibling proposals and starts a govern round, and a revert counts toward the fuse (D3). Only a signed-in Editor may give it; the ruled row's reason is that a token answer "would be one model approving another and would be stored as a person's answer" (D7).

**Option 1 — a new proposable operation targeting the governor's decision.** A ledger operation of its own (a string distinct from `review.merge`, `merge.manual` and `merge.revert`, because the open-proposal index is on base, operation and target key; the target key is the `agent_decisions` id), payload `{action: merge|keep|revert, rationale}`, policy row `AgentOperation::AnswerGovernor` (already `ProposeOnly`, no caller; map F1). Approval runs the three shapes of `agent_answer` as the person; a decision that is no longer `proposed` or `applied` closes the proposal `superseded` (the `settle` guard, D2). The agent finds the decision id in `review_list` section `agent` (rows whose `source` is not `token`; ADR 0076 decision 13). A dedicated MCP tool; its name must not contain `answer` (D11), see the name lists below (`review_governor` or `review_propose_governor`).
  - Route form 1a, a token branch on `POST /kbs/{id}/review/agent/{decision_id}`: follows the synthesis rule "same URLs, no parallel route tree", and contradicts the letter of "stays session-only" in the synthesis, ADR 0076 decisions 9 and 12 and `mcp.md:451`; the 401 assertion at `review_token_reads_tests.rs:1099` changes (D8).
  - Route form 1b, a new route: keeps every statement that the governor route is session-only and its test; contradicts the letter of "no parallel route tree", with the M003 precedent of three added routes (D8).
  - SC6: met directly. A write token of an Editor gets `proposed` for this row and a read-scope or below-editor caller gets the stable refusals; edges E1, E3 and E6 are testable as written (the governor row, the pair and the fuse unchanged; `already_decided` on an answered decision).
  - To record: a dated revision of ADR 0076 decision 9 or 12 either way, and what the person sees (D9).

**Option 2 — no separate operation; the row is satisfied by merge, keep and revert on the same pair, and the documentation says so.** Basis: deciding the pair already is answering the proposal (D4). What it gives at `d5d2356` (D5, D6): a held or propose-only token merge on a pair with an open governor proposal, once approved, settles that proposal `accepted` or `overridden`; a token revert of a governor merge is always a proposal (`merge_by_governor`) and its approval settles `reverted` and counts for the fuse. Not covered: a keep (finalizes at once and leaves the governor's proposal unanswered), a merge the gate passes (applies at once, proposal unanswered), and a merge over the governor's applied keep (may finalize; an approved manual-merge proposal does not settle the row `overridden`).
  - SC6: not met as written. No call yields "proposed" for the row "answer a governor decision"; the outcomes are those of the Merge, Keep and Revert rows. The intent's Scope in ("a proposed answer to a governor decision") and edges E1, E3 and E6 presuppose a proposal whose target is the governor's decision, so this option needs an intent correction through define, which is not a research or decide act.
  - It changes no route, tool, test or policy row, and leaves `AnswerGovernor` without a caller.

**Which the evidence favours.** Option 1: the approved intent already rules the behaviour of a distinct proposal (E1, E3, E6), the policy table already has the row, and option 2 covers two of the four answer shapes only. Between 1a and 1b the product's records conflict (D8); the domain evidence does not settle it.

## Option detail — tool names

Three schemes below satisfy every pinned rule; each needs the same two list entries in `review_mcp_reads_tests.rs` (D12) and none needs a rule test changed.

**Operation families to cover** (intent Scope in; policy rows in `execution_gate.rs:149-171`): fact (confirm, reject, close: three rows); conflict; violation; defect; errata; alignment phrase; alignment rule; alignment kind word (finalize); ontology proposal (reject, adopt: one row); mapping (finalize); consistency check and inference run (one row, two routes, finalize); proposed answer to a pending fact; governor answer (if its own operation). Fifteen policy rows.

**Rules every name was checked against** (D11): contains none of `proposal`, `approve`, `decline`, `answer`; is none of `review_confirm`, `review_decide`, `review_pending`, `review_approve`, `review_decline`, `review_answer`, `review_proposal_answer`, `agent_proposal_answer`. Specification form (D13): `[A-Za-z0-9_.-]`, 1 to 128 characters.

**Scheme A — one noun per family, `review_<noun>`** (11 new tools; 29 listed for a write Editor). The M003 pattern: one word after the prefix, one tool per section-level family.

| Tool | Covers | Ruling | Rule check |
|---|---|---|---|
| `review_fact` | confirm, reject, close (an `action` argument) | propose | pass |
| `review_conflict` | close old, retime new, keep both, reject new | propose | pass |
| `review_violation` | violation decision | propose | pass |
| `review_defect` | defect decision | propose | pass |
| `review_errata` | approve or reject a held action | propose | pass (the body value `approve` is an argument, not a name) |
| `review_alignment` | phrase, rule, kind word | mixed: propose, propose, finalize | pass |
| `review_ontology` | reject or adopt an ontology proposal | propose | pass (`review_ontology_proposal` would fail: contains `proposal`) |
| `review_mapping` | mapping decision | finalize | pass |
| `review_check` | consistency check, inference run | finalize | pass |
| `review_pending_fact` | proposed answer to a pending fact | propose | pass literally; one suffix away from `review_pending`, which stays unknown because tool lookup is by exact name (`review_tools.rs:39-41`) |
| `review_governor` | proposed answer to a governor decision | propose | pass |

Variant A′ (12 new tools): `review_alignment` covers phrase and rule only, and the kind word gets `review_kind_word` (pass), so no tool mixes a finalize and a propose-only ruling.

**Scheme B — the ruling in the name: `review_propose_<noun>` for propose-only, plain noun for finalize** (12 new tools).

`review_propose_fact`, `review_propose_conflict`, `review_propose_violation`, `review_propose_defect`, `review_propose_errata`, `review_propose_alignment` (phrase, rule), `review_propose_ontology`, `review_propose_nod`, `review_propose_governor`; finalize: `review_kind_word`, `review_mapping`, `review_check`. Every name passes: `propose` does not contain `proposal`, and none contains `approve`, `decline` or `answer`. Notes: the names differ from the forbidden substring by a suffix, so a later rename to `…proposal…` fails `review_proposal_answer_tests.rs:1122-1130`; the shipped `review_merge` and `review_revert` also propose at times without saying so in their names, so the prefix would mark only some proposing tools.

**Scheme C — one tool per ruled table row** (16 new tools; 34 listed).

`review_fact_confirm`, `review_fact_reject`, `review_fact_close`, `review_conflict`, `review_violation`, `review_defect`, `review_errata`, `review_alignment_phrase`, `review_alignment_rule`, `review_alignment_kind_word`, `review_ontology`, `review_mapping`, `review_check`, `review_infer`, `review_pending_fact`, `review_governor`. Every name passes (`review_fact_confirm` is not `review_confirm`). Notes: each tool has exactly one ruling and one input schema, which is the simplest case for annotations (D15) and for prohibition N11; the cost is the largest tool list (D14), and `review_fact_confirm` reads as the agent confirming unless the description leads with "Propose".

**Names that fail outright** (no list entry fixes them): any name containing `answer` (`review_governor_answer`, `review_agent_answer`, `review_pending_answer`), `proposal` (`review_ontology_proposal`, `review_proposal`), `approve` or `decline` (`review_errata_approve`); and the eight exact names. One omnibus `review_propose` with an operation argument passes the name rules and is excluded by the synthesis ("one write tool per operation family rather than one omnibus write tool") and ADR 0076's dead ends (`:547-548`).

**Tests, by effect, for any of the three schemes.**
- Need new list entries (D12): `review_mcp_reads_tests.rs:1062-1071` (expected names for a write token of an Editor) and `:1087-1090` (the `writes` expression; without it each new write tool must have annotations exactly `{readOnlyHint: true}`). A new read tool named `review_*` would also need `:1054` and `:1059`.
- Keep passing unchanged if the rules are obeyed: `review_proposal_answer_tests.rs:1122-1151`; `review_mcp_reads_tests.rs:1180-1213`; `review_token_merge_tests.rs:1816-1830`; `review_mcp_tests.rs:881-894, 1171-1175`.
- Fail outright: only through a name in the list above.
- Not tests but closed lists that gain entries: `serves` and `writes` in `review_tools.rs:39-46`; the tool tables and the annotations sentence in `web/src/docs/mcp.md:52-57, 207-212, 274`.

**How a tool that only proposes should describe itself** (N11; D16). Lead with the proposal, not the decision ("Propose that a person …"); say nothing is changed; say who answers and where ("only a signed-in person can answer in the Review tab"); give the result shape with `outcome: "proposed"`, `action_id`, `reason`, `explanation`; say a repeat returns the same proposal with `already_proposed`; end with "No tool answers a proposal". For alignment phrase and rule add that an approval queues the change (N6). Annotations for such a tool: `readOnlyHint: false`, `destructiveHint: false` set explicitly, `idempotentHint` false as M003 did or true by edge E2 (D15).

**Which the evidence favours.** A′. It continues the one shipped precedent (D10: `review_<word>`, one tool per family, argument forms inside the tool), stays lowest in tool count among the schemes with no mixed-ruling tool (D14, D15), and keeps one ruling per description for N11. Scheme C is the stronger choice if decide weighs N11 and per-tool annotations above the tool count. Scheme B's only gain, the ruling in the name, is also available from the description, and it marks M003's proposing tools inconsistently.

## Dead ends

- MCP specification guidance on how many tools a server should expose or how to group operations — none in revisions 2025-06-18, 2025-11-25 or 2026-07-28; only the pagination and (2026-07-28) deterministic-order sentences touch the list (D14).
- Tool-name rules in the revision the server declares (2025-06-18) — there are none; the rules start in 2025-11-25 (D13).
- A product record that designs a token, agent or second model answering the governor — none in `docs/decisions/` (0025, 0026, 0027, 0028, 0043, 0044, 0076) or `docs/design/` (`governance.md`, `identity.md`, `README.md`, `chat-and-mcp.md`, `interface.md`, `access-and-audit.md`); ADR 0043's undo table and Agent-queue answers for facts and conflicts are accepted but not built (D7).
- A product record that sets a tool-naming convention — grep of `docs/decisions/` for tool naming, "one tool", "per tool" found only ADR 0076 decision 2 and its dead end, and ADR 0020's rule that a documented `structuredContent` field is not renamed without a decision record (`0020-…md:21-23`), which is about fields, not names.
- The first fetch of https://modelcontextprotocol.io/specification/2025-11-25/schema returned characters 0 to 100000, which end before `Tool` and `ToolAnnotations`; the definitions were read from offset 100000. The first fetch of the 2026-07-28 tools page timed out and the second succeeded.
- ADR 0046 and ADR 0014 were searched by heading and for "tool per", "write tool", "every tool" only; no naming or grouping rule was found, and their full text was not read.
- Conflicts left unresolved for decide or the user: the governor route's status (D8); intent edge E19 against the shipped MCP refusal (D18); the specification's rate-limit MUST against intent edge E5 (D19); the specification's `-32602` against the pinned `-32601` (D17).
